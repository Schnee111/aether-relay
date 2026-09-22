pub mod health;

use axum::Router;
use axum::routing::get;

pub fn create_router() -> Router {
    Router::new().route("/health", get(health::health_check))
}
