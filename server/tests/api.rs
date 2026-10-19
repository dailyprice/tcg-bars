use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn test_app() -> Router {
    let path = std::env::temp_dir().join(format!("tcgbars-test-{}.db", uuid::Uuid::new_v4()));
    let pool = tcgbars_server::db::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    tcgbars_server::db::seed_if_empty(&pool).await.unwrap();
    tcgbars_server::app(pool, None)
}

async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body.map_or(Body::empty(), |b| Body::from(b.to_string())))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn lists_seeded_bars_as_geojson() {
    let app = test_app().await;
    let (status, body) = call(&app, "GET", "/api/bars", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["type"], "FeatureCollection");
    let features = body["features"].as_array().unwrap();
    assert_eq!(features.len(), 4);
    assert_eq!(features[0]["geometry"]["type"], "Point");
    assert_eq!(features[0]["properties"]["location_verified"], false);
}

#[tokio::test]
async fn creates_rates_and_moves_a_bar() {
    let app = test_app().await;
    let (status, bar) = call(
        &app,
        "POST",
        "/api/bars",
        Some(json!({ "name": "  Test Bar ", "lat": 52.52, "lng": 13.405, "city": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(bar["properties"]["name"], "Test Bar");
    assert_eq!(bar["properties"]["city"], Value::Null);
    let id = bar["id"].as_str().unwrap().to_string();

    let rating = json!({ "lighting": 5, "table_space": 3, "offers": 4, "comment": "big tables" });
    let (status, rated) = call(
        &app,
        "POST",
        &format!("/api/bars/{id}/ratings"),
        Some(rating),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(rated["properties"]["rating_count"], 1);
    assert_eq!(rated["properties"]["score"], 4.0);

    let (status, moved) = call(
        &app,
        "PATCH",
        &format!("/api/bars/{id}/location"),
        Some(json!({ "lat": 52.0, "lng": 13.0 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(moved["geometry"]["coordinates"], json!([13.0, 52.0]));

    let (status, detail) = call(&app, "GET", &format!("/api/bars/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["ratings"][0]["comment"], "big tables");
}

#[tokio::test]
async fn rejects_bad_input() {
    let app = test_app().await;
    let (status, _) = call(
        &app,
        "POST",
        "/api/bars",
        Some(json!({ "name": " ", "lat": 1.0, "lng": 1.0 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = call(
        &app,
        "POST",
        "/api/bars",
        Some(json!({ "name": "X", "lat": 91.0, "lng": 1.0 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (_, bars) = call(&app, "GET", "/api/bars", None).await;
    let id = bars["features"][0]["id"].as_str().unwrap().to_string();
    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/bars/{id}/ratings"),
        Some(json!({ "lighting": 6, "table_space": 3, "offers": 3 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _) = call(
        &app,
        "POST",
        "/api/bars/missing/ratings",
        Some(json!({ "lighting": 3, "table_space": 3, "offers": 3 })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
