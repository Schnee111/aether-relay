pub mod dlq;
pub mod endpoints;
pub mod health;
pub mod ingest;
pub mod middleware;

use crate::state::AppState;
use axum::Router;
use axum::routing::{get, post};

pub fn create_router(state: AppState) -> Router {
    let metrics_enabled = state.config.metrics.enabled;

    let mut router = Router::new()
        .route("/health", get(health::health_check))
        .route("/v1/ingest/{endpoint_id}", post(ingest::handle_ingest))
        .route("/v1/dlq", get(dlq::list_dlq))
        .route("/v1/dlq/{id}/replay", post(dlq::replay_dlq))
        .route("/v1/endpoints", post(endpoints::create_endpoint))
        .route("/v1/endpoints", get(endpoints::list_endpoints))
        .route(
            "/v1/endpoints/{id}",
            axum::routing::delete(endpoints::delete_endpoint),
        );

    if metrics_enabled {
        router = router.route("/metrics", get(middleware::metrics::metrics_endpoint));
    }

    router
        .layer(axum::middleware::from_fn(
            middleware::metrics::track_metrics,
        ))
        .with_state(state)
}
