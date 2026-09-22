use aether_relay::config::AppConfig;
use aether_relay::db::create_pool;
use aether_relay::state::AppState;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use hmac::{Hmac, KeyInit, Mac};
use http_body_util::BodyExt;
use rusqlite::params;
use sha2::Sha256;
use tower::ServiceExt;

type HmacSha256 = Hmac<Sha256>;

fn setup_test_app() -> (axum::Router, String, String) {
    let pool = create_pool(":memory:", 1, 5000, 0, -2000).unwrap();
    let config = AppConfig::load().unwrap();

    let endpoint_id = "ep_github_test";
    let secret = "github_webhook_secret_key";

    // Seed endpoint
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO endpoints (id, name, provider, secret, target_url, created_at)
             VALUES (?1, 'GitHub Test', 'github', ?2, 'https://downstream.test/webhook', 1700000000)",
            params![endpoint_id, secret],
        )
        .unwrap();
    }

    let state = AppState {
        pool,
        config: config.clone(),
    };
    (
        aether_relay::api::create_router(state),
        endpoint_id.to_string(),
        secret.to_string(),
    )
}

fn compute_github_sig(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

#[tokio::test]
async fn test_ingest_valid_and_duplicate() {
    let (app, ep_id, secret) = setup_test_app();
    let body = b"{\"event\":\"push\",\"ref\":\"refs/heads/main\"}";
    let sig = compute_github_sig(&secret, body);

    // 1. Valid ingest -> 202 Accepted
    let req = Request::builder()
        .uri(format!("/v1/ingest/{}", ep_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", "req_id_1001")
        .header("X-Hub-Signature-256", &sig)
        .body(Body::from(body.to_vec()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let resp_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&resp_bytes).unwrap();
    assert_eq!(json["status"], "RECEIVED");
    assert!(json["id"].is_string());

    // 2. Duplicate idempotency key -> 409 Conflict
    let dup_req = Request::builder()
        .uri(format!("/v1/ingest/{}", ep_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", "req_id_1001")
        .header("X-Hub-Signature-256", &sig)
        .body(Body::from(body.to_vec()))
        .unwrap();

    let dup_resp = app.oneshot(dup_req).await.unwrap();
    assert_eq!(dup_resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_ingest_corrupted_signature() {
    let (app, ep_id, _secret) = setup_test_app();
    let body = b"{\"event\":\"ping\"}";

    let req = Request::builder()
        .uri(format!("/v1/ingest/{}", ep_id))
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", "req_id_corrupt")
        .header(
            "X-Hub-Signature-256",
            "sha256=0000000000000000000000000000000000000000000000000000000000000000",
        )
        .body(Body::from(body.to_vec()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_ingest_unknown_endpoint() {
    let (app, _ep_id, _secret) = setup_test_app();
    let body = b"{\"event\":\"ping\"}";

    let req = Request::builder()
        .uri("/v1/ingest/non_existent_ep")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", "req_id_404")
        .header("X-Hub-Signature-256", "sha256=abcdef")
        .body(Body::from(body.to_vec()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
