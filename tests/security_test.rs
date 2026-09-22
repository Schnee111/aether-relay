//! Security-relevant behaviour of the management and ingestion APIs.
//!
//! Each test here corresponds to a defect found in review: a missing client
//! header answered 500, an unknown provider was accepted and then failed every
//! later delivery, the relay would happily POST to loopback and cloud metadata
//! addresses, and the create response could not be chained into the documented
//! flow.

use aether_relay::config::AppConfig;
use aether_relay::db::create_pool;
use aether_relay::state::AppState;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rusqlite::params;
use serde_json::json;
use tower::ServiceExt;

fn setup_app() -> (axum::Router, aether_relay::db::DbPool) {
    let pool = create_pool(":memory:", 1, 5000, 0, -2000).unwrap();
    let config = AppConfig::load().unwrap();
    let state = AppState {
        pool: pool.clone(),
        config,
    };
    (aether_relay::api::create_router(state), pool)
}

async fn post_json(
    app: &axum::Router,
    uri: &str,
    payload: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .uri(uri)
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, body)
}

fn endpoint_payload(provider: &str, target_url: &str) -> serde_json::Value {
    json!({
        "name": "Test Endpoint",
        "provider": provider,
        "secret": "s3cret",
        "target_url": target_url
    })
}

#[tokio::test]
async fn create_endpoint_returns_an_object_with_the_id() {
    let (app, _pool) = setup_app();

    let (status, body) = post_json(
        &app,
        "/v1/endpoints",
        endpoint_payload("github", "https://downstream.example.com/hook"),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(
        body["id"].is_string(),
        "response must be an object exposing `id`, got: {body}"
    );
    assert_eq!(body["provider"], "github");

    // The signing secret must never be echoed back.
    assert!(
        body.get("secret").is_none(),
        "response must not leak the endpoint secret: {body}"
    );
}

#[tokio::test]
async fn create_endpoint_rejects_an_unknown_provider() {
    let (app, _pool) = setup_app();

    let (status, body) = post_json(
        &app,
        "/v1/endpoints",
        endpoint_payload("paypal", "https://downstream.example.com/hook"),
    )
    .await;

    // Accepting it here would create an endpoint that can never verify a
    // signature, turning a configuration typo into a stream of 500s.
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "UNSUPPORTED_PROVIDER");
}

#[tokio::test]
async fn create_endpoint_refuses_targets_inside_the_network() {
    let (app, _pool) = setup_app();

    let hostile = [
        "http://127.0.0.1/hook",
        "http://localhost/hook",
        "http://10.0.0.5/hook",
        "http://192.168.1.10/hook",
        "http://172.16.4.4/hook",
        // Cloud metadata service.
        "http://169.254.169.254/latest/meta-data/",
        "http://[::1]/hook",
        "http://[fd00::1]/hook",
        "http://0.0.0.0/hook",
    ];

    for target in hostile {
        let (status, body) =
            post_json(&app, "/v1/endpoints", endpoint_payload("generic", target)).await;

        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "target {target} must be refused, got {status} {body}"
        );
        assert_eq!(body["code"], "BAD_REQUEST");
    }
}

#[tokio::test]
async fn create_endpoint_refuses_non_http_schemes() {
    let (app, _pool) = setup_app();

    for target in [
        "file:///etc/passwd",
        "gopher://example.com/",
        "ftp://example.com/",
    ] {
        let (status, _) =
            post_json(&app, "/v1/endpoints", endpoint_payload("generic", target)).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "scheme of {target} must be refused"
        );
    }
}

#[tokio::test]
async fn create_endpoint_accepts_a_public_destination() {
    let (app, _pool) = setup_app();

    let (status, _) = post_json(
        &app,
        "/v1/endpoints",
        endpoint_payload("stripe", "https://api.example.com/webhooks"),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
}

async fn post_ingest(
    app: &axum::Router,
    endpoint_id: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder()
        .uri(format!("/v1/ingest/{endpoint_id}"))
        .method("POST")
        .header("Content-Type", "application/json");

    for (k, v) in headers {
        builder = builder.header(*k, *v);
    }

    let resp = app
        .clone()
        .oneshot(builder.body(Body::from("{}")).unwrap())
        .await
        .unwrap();

    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, body)
}

#[tokio::test]
async fn ingest_without_idempotency_key_is_a_client_error() {
    let (app, pool) = setup_app();

    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO endpoints (id, name, provider, secret, target_url, created_at)
             VALUES ('ep_x', 'Test', 'generic', 'sec', 'https://example.com/hook', 1700000000)",
            [],
        )
        .unwrap();
    }

    let (status, body) = post_ingest(&app, "ep_x", &[]).await;

    // A missing request header is the caller's fault. This used to answer 500,
    // which told the caller the server had broken.
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "BAD_REQUEST");
}

#[tokio::test]
async fn ingest_with_empty_idempotency_key_is_a_client_error() {
    let (app, pool) = setup_app();

    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO endpoints (id, name, provider, secret, target_url, created_at)
             VALUES ('ep_y', 'Test', 'generic', 'sec', 'https://example.com/hook', 1700000000)",
            [],
        )
        .unwrap();
    }

    let (status, _) = post_ingest(&app, "ep_y", &[("Idempotency-Key", "")]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn deleting_an_endpoint_with_undelivered_events_is_refused() {
    let (app, pool) = setup_app();

    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO endpoints (id, name, provider, secret, target_url, created_at)
             VALUES ('ep_busy', 'Busy', 'generic', 'sec', 'https://example.com/hook', 1700000000)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO incoming_events (id, endpoint_id, idempotency_key, raw_body, headers, status, created_at)
             VALUES ('ev_busy', 'ep_busy', 'idem_busy', 'body', '{}', 'RECEIVED', 1700000000)",
            [],
        )
        .unwrap();
    }

    let req = Request::builder()
        .uri("/v1/endpoints/ep_busy")
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CONFLICT,
        "an endpoint with in-flight events must not be deleted silently"
    );

    // And it is still there.
    let count: i64 = pool
        .get()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM endpoints WHERE id = 'ep_busy'",
            params![],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}
