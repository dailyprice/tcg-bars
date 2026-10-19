use std::{convert::Infallible, time::Duration};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, patch, post},
};
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_stream::wrappers::BroadcastStream;

use crate::{
    AppState,
    db::{self, BarFeature, NewBar, NewRating, Rating},
    events::BarEvent,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/bars", get(list_bars).post(create_bar))
        .route("/bars/{id}", get(get_bar))
        .route("/bars/{id}/location", patch(move_bar))
        .route("/bars/{id}/ratings", post(rate_bar))
        .route("/events", get(events))
}

pub enum ApiError {
    NotFound,
    Invalid(String),
    Internal(sqlx::Error),
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        ApiError::Internal(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "bar not found".to_string()),
            ApiError::Invalid(message) => (StatusCode::UNPROCESSABLE_ENTITY, message),
            ApiError::Internal(err) => {
                tracing::error!("database error: {err}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal error".to_string(),
                )
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

#[derive(Serialize)]
struct FeatureCollection {
    #[serde(rename = "type")]
    kind: &'static str,
    features: Vec<BarFeature>,
}

async fn list_bars(State(state): State<AppState>) -> ApiResult<Json<FeatureCollection>> {
    Ok(Json(FeatureCollection {
        kind: "FeatureCollection",
        features: db::list_bars(&state.pool).await?,
    }))
}

#[derive(Serialize)]
struct BarDetail {
    bar: BarFeature,
    ratings: Vec<Rating>,
}

async fn get_bar(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<BarDetail>> {
    let bar = db::get_bar(&state.pool, &id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let ratings = db::recent_ratings(&state.pool, &id).await?;
    Ok(Json(BarDetail { bar, ratings }))
}

fn check_coordinates(lat: f64, lng: f64) -> ApiResult<()> {
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lng) {
        return Err(ApiError::Invalid("coordinates out of range".into()));
    }
    Ok(())
}

/// Trims optional text and drops it if empty; rejects anything too long.
fn clean_text(field: &str, value: Option<String>, max: usize) -> ApiResult<Option<String>> {
    let value = value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    if let Some(v) = &value
        && v.chars().count() > max
    {
        return Err(ApiError::Invalid(format!(
            "{field} is too long (max {max})"
        )));
    }
    Ok(value)
}

async fn create_bar(
    State(state): State<AppState>,
    Json(input): Json<NewBar>,
) -> ApiResult<(StatusCode, Json<BarFeature>)> {
    let name = clean_text("name", Some(input.name), 120)?
        .ok_or_else(|| ApiError::Invalid("name is required".into()))?;
    check_coordinates(input.lat, input.lng)?;
    let bar = NewBar {
        name,
        address: clean_text("address", input.address, 200)?,
        city: clean_text("city", input.city, 100)?,
        country: clean_text("country", input.country, 100)?,
        player_offers: clean_text("player_offers", input.player_offers, 500)?,
        website: clean_text("website", input.website, 300)?,
        ..input
    };

    // Whoever adds a bar places the pin themselves, so it counts as verified.
    let id = db::insert_bar(&state.pool, &bar, true).await?;
    let feature = db::get_bar(&state.pool, &id)
        .await?
        .ok_or(ApiError::NotFound)?;
    state.events.publish(BarEvent::BarCreated {
        bar: feature.clone(),
    });
    Ok((StatusCode::CREATED, Json(feature)))
}

#[derive(Deserialize)]
struct Location {
    lat: f64,
    lng: f64,
}

async fn move_bar(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(location): Json<Location>,
) -> ApiResult<Json<BarFeature>> {
    check_coordinates(location.lat, location.lng)?;
    if !db::move_bar(&state.pool, &id, location.lat, location.lng).await? {
        return Err(ApiError::NotFound);
    }
    let feature = db::get_bar(&state.pool, &id)
        .await?
        .ok_or(ApiError::NotFound)?;
    state.events.publish(BarEvent::BarMoved {
        bar: feature.clone(),
    });
    Ok(Json(feature))
}

async fn rate_bar(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<NewRating>,
) -> ApiResult<(StatusCode, Json<BarFeature>)> {
    for (field, value) in [
        ("lighting", input.lighting),
        ("table_space", input.table_space),
        ("offers", input.offers),
    ] {
        if !(1..=5).contains(&value) {
            return Err(ApiError::Invalid(format!(
                "{field} must be between 1 and 5"
            )));
        }
    }
    let rating = NewRating {
        comment: clean_text("comment", input.comment, 1000)?,
        ..input
    };
    if db::get_bar(&state.pool, &id).await?.is_none() {
        return Err(ApiError::NotFound);
    }
    db::insert_rating(&state.pool, &id, &rating).await?;
    let feature = db::get_bar(&state.pool, &id)
        .await?
        .ok_or(ApiError::NotFound)?;
    state.events.publish(BarEvent::BarRated {
        bar: feature.clone(),
    });
    Ok((StatusCode::CREATED, Json(feature)))
}

async fn events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(state.events.subscribe()).filter_map(|msg| async move {
        // A lagged receiver just skips what it missed; the client refetches on reconnect.
        let event = msg.ok()?;
        Event::default()
            .event(event.name())
            .json_data(&event)
            .ok()
            .map(Ok)
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(20)))
}
