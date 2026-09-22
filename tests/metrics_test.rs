//! /metrics endpoint: exposition format parses and counters increment.

use aether_relay::api::create_router;
use aether_relay::config::AppConfig;
use aether_relay::db::create_pool;
use aether_relay::state::AppState;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use hmac::{Hmac, KeyInit, Mac};
use http_body_util::BodyExt;
use serde_json::json;
use sha2::Sha256;
use tower::ServiceExt;

type HmacSha256 = Hmac<Sha256>;

fn metrics_enabled_config(db_path: &str) -> AppConfig {
    let mut cfg: AppConfig = serde_json::from_str(
        &json!({
            "server": {"host": "127.0.0.1", "port": 0, "body_limit_bytes": 65536},
            "database": {"path": db_path, "busy_timeout_ms": 5000, "mmap_size": 0,
                         "cache_size": -2000, "pool_size": 2},
            "auth": {"api_keys": ["test-key"]},
            "worker": {"poll_interval_ms": 50, "max_attempts": 3, "backoff_base_ms": 10,
                       "backoff_cap_ms": 100, "circuit_failure_threshold": 5,
                       "circuit_recovery_timeout_secs": 1},
            "logging": {"level": "error", "format": "pretty"},
            "metrics": {"enabled": true}
        })
        .to_string(),
    )
    .expect("valid config");
    cfg.database.path = db_path.to_string();
    cfg
}

fn sign(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

#[tokio::test]
async fn metrics_endpoint_counts_ingest_and_parses() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("m.db").to_string_lossy().to_string();
    let cfg = metrics_enabled_config(&db_path);

    let pool = create_pool(
        &cfg.database.path,
        cfg.database.pool_size,
        cfg.database.busy_timeout_ms,
        cfg.database.mmap_size,
        cfg.database.cache_size,
    )
    .unwrap();

    // Seed one endpoint with a known secret and target.
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO endpoints (id, name, provider, secret, target_url, created_at)
             VALUES ('ep-metrics', 'm', 'generic', 'sekrit', 'http://127.0.0.1:9/hook', 1)",
            [],
        )
        .unwrap();
    }

    let state = AppState {
        pool,
        config: cfg.clone(),
    };

    // The exporter install lives in main.rs (process-wide, once). Integration
    // tests build the router directly, so install it here for the enabled case.
    aether_relay::api::middleware::metrics::install_prometheus_exporter();

    let app = create_router(state);

    let body = serde_json::to_vec(&json!({"event": "metrics_test"})).unwrap();
    let req = Request::builder()
        .method("POST")
        .uri("/v1/ingest/ep-metrics")
        .header("content-type", "application/json")
        .header("x-api-key", "test-key")
        .header("idempotency-key", "metrics-test-1")
        .header("x-signature", sign("sekrit", &body))
        .body(Body::from(body))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let req = Request::builder()
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let text = String::from_utf8(
        resp.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();

    assert!(
        text.contains("aether_webhook_ingest_total"),
        "exposition must carry the ingest counter"
    );
    // One sample line per label set plus the HELP/TYPE header lines.
    assert!(
        text.contains("endpoint=\"ep-metrics\""),
        "endpoint label missing"
    );
    assert!(
        text.contains("provider=\"generic\""),
        "provider label missing"
    );
    // The path label must carry the matched route TEMPLATE, never the raw URI
    // (that would let client input inflate Prometheus series cardinality).
    assert!(
        text.contains("path=\"/v1/ingest/{endpoint_id}\""),
        "path label must be the route template, got no template label in:\n{text}"
    );
    assert!(
        !text.contains("path=\"/v1/ingest/ep-metrics\""),
        "raw URI leaked into the path label"
    );
}
