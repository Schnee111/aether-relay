pub mod health;
pub mod ingest;
pub mod middleware;

use crate::state::AppState;
use axum::Router;
use axum::routing::{get, post};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health_check))
        .route("/v1/ingest/{endpoint_id}", post(ingest::handle_ingest))
        .with_state(state)
}
