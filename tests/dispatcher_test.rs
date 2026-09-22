use aether_relay::config::AppConfig;
use aether_relay::db::create_pool;
use aether_relay::db::models::EventStatus;
use aether_relay::state::AppState;
use aether_relay::worker::circuit_breaker::{CircuitBreaker, CircuitState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rusqlite::params;
use std::time::Duration;
use tower::ServiceExt;

#[test]
fn test_circuit_breaker_lifecycle() {
    let mut cb = CircuitBreaker::new(3, Duration::from_millis(50));
    assert_eq!(cb.state(), CircuitState::Closed);
    assert!(cb.can_attempt());

    // 2 failures -> still Closed
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Closed);

    // 3rd failure -> trips Open
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
    assert!(!cb.can_attempt());

    // Wait for recovery timeout
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(cb.state(), CircuitState::HalfOpen);

    // HalfOpen admits exactly ONE probe. Every other queued event must wait,
    // otherwise the whole backlog floods a downstream that has just been given
    // a chance to recover.
    assert!(cb.can_attempt(), "first caller becomes the probe");
    assert!(
        !cb.can_attempt(),
        "second caller must be refused while the probe is in flight"
    );
    assert!(!cb.can_attempt(), "still refused while probe is in flight");

    // A successful probe closes the circuit and clears the probe slot.
    cb.record_success();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert!(cb.can_attempt());
    assert!(cb.can_attempt(), "closed circuit admits everything again");
}

#[test]
fn test_half_open_failed_probe_reopens_circuit() {
    let mut cb = CircuitBreaker::new(1, Duration::from_millis(30));
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);

    std::thread::sleep(Duration::from_millis(40));
    assert_eq!(cb.state(), CircuitState::HalfOpen);
    assert!(cb.can_attempt());

    // The probe fails -> circuit reopens and the probe slot is freed.
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
    assert!(!cb.can_attempt());
}

#[tokio::test]
async fn test_dlq_list_and_replay_api() {
    let pool = create_pool(":memory:", 1, 5000, 0, -2000).unwrap();
    let config = AppConfig::load().unwrap();

    // Insert an endpoint, a failed event, and a DLQ row
    let event_id = "ev_failed_123";
    let endpoint_id = "ep_1";
    let dlq_id = "dlq_rec_456";

    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO endpoints (id, name, provider, secret, target_url, created_at)
             VALUES (?1, 'Test', 'generic', 'sec', 'https://mock.local', 1700000000)",
            params![endpoint_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO incoming_events (id, endpoint_id, idempotency_key, raw_body, headers, status, created_at)
             VALUES (?1, ?2, 'idem_1', 'body', '{}', 'FAILED', 1700000000)",
            params![event_id, endpoint_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO dead_letter_queue (id, event_id, endpoint_id, error_reason, last_attempt_status, created_at)
             VALUES (?1, ?2, ?3, 'Max retries 5 reached', 500, 1700000000)",
            params![dlq_id, event_id, endpoint_id],
        )
        .unwrap();
    }

    let state = AppState {
        pool: pool.clone(),
        config,
    };
    let app = aether_relay::api::create_router(state);

    // 1. List DLQ
    let list_req = Request::builder()
        .uri("/v1/dlq")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let list_resp = app.clone().oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);

    let list_bytes = list_resp.into_body().collect().await.unwrap().to_bytes();
    let list_json: serde_json::Value = serde_json::from_slice(&list_bytes).unwrap();
    assert_eq!(list_json.as_array().unwrap().len(), 1);
    assert_eq!(list_json[0]["id"], dlq_id);

    // 2. Replay DLQ
    let replay_req = Request::builder()
        .uri(format!("/v1/dlq/{}/replay", dlq_id))
        .method("POST")
        .body(Body::empty())
        .unwrap();

    let replay_resp = app.oneshot(replay_req).await.unwrap();
    assert_eq!(replay_resp.status(), StatusCode::OK);

    // 3. Verify event is back to RECEIVED and DLQ is empty
    let conn = pool.get().unwrap();
    let status: String = conn
        .query_row(
            "SELECT status FROM incoming_events WHERE id = ?1",
            params![event_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, EventStatus::Received.as_str());

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM dead_letter_queue", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}
