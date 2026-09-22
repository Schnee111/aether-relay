use aether_relay::config::AppConfig;
use aether_relay::db::create_pool;
use aether_relay::state::AppState;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn test_health_endpoint() {
    let pool = create_pool(":memory:", 1, 5000, 0, -2000).unwrap();
    let config = AppConfig::load().unwrap();
    let state = AppState { pool, config };
    let app = aether_relay::api::create_router(state);

    let request = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert_eq!(json["version"], "0.2.0");
    assert!(json["timestamp"].is_number());
}
