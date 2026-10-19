pub mod api;
pub mod db;
pub mod events;

use std::path::PathBuf;

use axum::Router;
use sqlx::SqlitePool;
use tower_http::{services::ServeDir, trace::TraceLayer};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub events: events::EventBus,
}

/// Builds the HTTP app. `static_dir` is the built frontend (web/dist); pass
/// `None` to serve the API only.
pub fn app(pool: SqlitePool, static_dir: Option<PathBuf>) -> Router {
    let state = AppState {
        pool,
        events: events::EventBus::new(),
    };
    let router = Router::new()
        .nest("/api", api::routes())
        .with_state(state)
        .layer(TraceLayer::new_for_http());
    match static_dir {
        Some(dir) => router.fallback_service(ServeDir::new(dir)),
        None => router,
    }
}
