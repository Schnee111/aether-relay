//! End-to-end coverage for the dispatch worker.
//!
//! Before these tests existed the gateway had no caller of
//! `dispatch_single_event` at all: events were accepted and stored, and then
//! nothing ever delivered them. These tests stand up a real HTTP downstream and
//! assert on what actually reaches the wire.

use aether_relay::db::create_pool;
use aether_relay::db::models::EventStatus;
use aether_relay::worker::circuit_breaker::create_shared_breakers;
use aether_relay::worker::runner::{DispatchLoopConfig, DispatchOutcome, run_dispatch_once};
use rusqlite::params;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

/// Minimal HTTP downstream that records how many requests it received and
/// replies with `status`. Returns the URL to point an endpoint at.
async fn spawn_downstream(status: u16, hits: Arc<AtomicU32>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let hits = hits.clone();
            tokio::spawn(async move {
                let mut stream = stream;
                // Drain the request; we only care that it arrived.
                let mut buf = [0u8; 4096];
                use tokio::io::AsyncReadExt;
                let _ = stream.read(&mut buf).await;

                hits.fetch_add(1, Ordering::SeqCst);

                let body = format!("{{\"downstream\":{status}}}");
                let resp = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.flush().await;
            });
        }
    });

    format!("http://{addr}/hook")
}

fn seed_endpoint_and_event(pool: &aether_relay::db::DbPool, target_url: &str, event_id: &str) {
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO endpoints (id, name, provider, secret, target_url, created_at)
         VALUES ('ep_e2e', 'E2E', 'generic', 'sec', ?1, 1700000000)",
        params![target_url],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO incoming_events (id, endpoint_id, idempotency_key, raw_body, headers, status, created_at)
         VALUES (?1, 'ep_e2e', ?2, ?3, '{}', 'RECEIVED', 1700000000)",
        params![event_id, format!("idem_{event_id}"), b"{\"ping\":true}".to_vec()],
    )
    .unwrap();
}

// The pool holds a single connection, so every helper below must let its guard
// drop before returning. Holding one across a call that also wants a connection
// deadlocks the pool.
fn event_status(pool: &aether_relay::db::DbPool, event_id: &str) -> String {
    pool.get()
        .unwrap()
        .query_row(
            "SELECT status FROM incoming_events WHERE id = ?1",
            params![event_id],
            |row| row.get(0),
        )
        .unwrap()
}

fn attempt_rows(pool: &aether_relay::db::DbPool, event_id: &str) -> u32 {
    pool.get()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM delivery_attempts WHERE event_id = ?1",
            params![event_id],
            |row| row.get(0),
        )
        .unwrap()
}

fn dlq_reason(pool: &aether_relay::db::DbPool, event_id: &str) -> String {
    pool.get()
        .unwrap()
        .query_row(
            "SELECT error_reason FROM dead_letter_queue WHERE event_id = ?1",
            params![event_id],
            |row| row.get(0),
        )
        .unwrap()
}

fn loop_config(max_attempts: u32) -> DispatchLoopConfig {
    DispatchLoopConfig {
        poll_interval: Duration::from_millis(5),
        max_attempts,
        backoff_base: Duration::from_millis(1),
        backoff_cap: Duration::from_millis(10),
        circuit_failure_threshold: 3,
        circuit_recovery: Duration::from_millis(50),
    }
}

#[tokio::test]
async fn dispatch_once_delivers_event_to_downstream() {
    let pool = create_pool(":memory:", 1, 5000, 0, -2000).unwrap();
    let hits = Arc::new(AtomicU32::new(0));
    let url = spawn_downstream(200, hits.clone()).await;

    seed_endpoint_and_event(&pool, &url, "ev_ok");

    let breakers = create_shared_breakers();
    let client = reqwest::Client::new();
    let cfg = loop_config(3);

    let outcome = run_dispatch_once(&pool, &breakers, &client, &cfg)
        .await
        .unwrap();
    assert_eq!(outcome, DispatchOutcome::Settled);

    // The event actually left the process.
    assert_eq!(hits.load(Ordering::SeqCst), 1, "downstream got one request");
    assert_eq!(
        event_status(&pool, "ev_ok"),
        EventStatus::Delivered.as_str()
    );
    assert_eq!(attempt_rows(&pool, "ev_ok"), 1);

    // Nothing left to do.
    let idle = run_dispatch_once(&pool, &breakers, &client, &cfg)
        .await
        .unwrap();
    assert_eq!(idle, DispatchOutcome::Idle, "queue is empty");
}

#[tokio::test]
async fn failing_event_is_retried_up_to_max_attempts_then_dead_lettered() {
    let pool = create_pool(":memory:", 1, 5000, 0, -2000).unwrap();
    let hits = Arc::new(AtomicU32::new(0));
    let url = spawn_downstream(500, hits.clone()).await;

    seed_endpoint_and_event(&pool, &url, "ev_fail");

    let breakers = create_shared_breakers();
    let client = reqwest::Client::new();
    let max_attempts = 3;

    // Circuit threshold high so the breaker does not mask the retry logic.
    let mut cfg = loop_config(max_attempts);
    cfg.circuit_failure_threshold = 99;

    for _ in 0..max_attempts {
        let outcome = run_dispatch_once(&pool, &breakers, &client, &cfg)
            .await
            .unwrap();
        assert_eq!(
            outcome,
            DispatchOutcome::Settled,
            "each pass must claim the still-undelivered event"
        );
    }

    assert_eq!(
        attempt_rows(&pool, "ev_fail"),
        max_attempts,
        "each failed delivery must be recorded as an attempt"
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        max_attempts,
        "downstream must actually be contacted on every attempt"
    );

    // Budget exhausted -> parked, not left spinning in the queue.
    assert_eq!(event_status(&pool, "ev_fail"), EventStatus::Failed.as_str());

    let reason = dlq_reason(&pool, "ev_fail");
    assert!(
        reason.contains("500"),
        "DLQ should preserve the downstream failure, got: {reason}"
    );

    // A settled event must not be re-claimed.
    let idle = run_dispatch_once(&pool, &breakers, &client, &cfg)
        .await
        .unwrap();
    assert_eq!(
        idle,
        DispatchOutcome::Idle,
        "dead-lettered event must not be retried"
    );
}

#[tokio::test]
async fn open_circuit_defers_delivery_without_burning_attempts() {
    let pool = create_pool(":memory:", 1, 5000, 0, -2000).unwrap();
    let hits = Arc::new(AtomicU32::new(0));
    let url = spawn_downstream(500, hits.clone()).await;

    seed_endpoint_and_event(&pool, &url, "ev_cb");

    let breakers = create_shared_breakers();
    let client = reqwest::Client::new();
    let mut cfg = loop_config(5);
    cfg.circuit_failure_threshold = 1;

    // First pass fails and trips the breaker.
    let first = run_dispatch_once(&pool, &breakers, &client, &cfg)
        .await
        .unwrap();
    assert_eq!(first, DispatchOutcome::Settled);
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // Second pass: the breaker is OPEN, so the downstream is not contacted and
    // no delivery attempt is recorded.
    let second = run_dispatch_once(&pool, &breakers, &client, &cfg)
        .await
        .unwrap();
    assert_eq!(second, DispatchOutcome::Deferred, "circuit must defer");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "open circuit must not touch the downstream"
    );
    assert_eq!(
        attempt_rows(&pool, "ev_cb"),
        1,
        "a deferred delivery must not consume retry budget"
    );
    assert_eq!(
        event_status(&pool, "ev_cb"),
        EventStatus::Received.as_str(),
        "deferred event goes back to the queue, not to the DLQ"
    );

    // The event is not stranded: it is re-claimable once the circuit allows it.
    let claimed_again = run_dispatch_once(&pool, &breakers, &client, &cfg)
        .await
        .unwrap();
    assert_ne!(
        claimed_again,
        DispatchOutcome::Idle,
        "a deferred event must remain claimable, not stranded in PROCESSING"
    );
}

#[tokio::test]
async fn replay_resets_attempt_budget() {
    let pool = create_pool(":memory:", 1, 5000, 0, -2000).unwrap();
    let hits = Arc::new(AtomicU32::new(0));
    let url = spawn_downstream(500, hits.clone()).await;

    seed_endpoint_and_event(&pool, &url, "ev_replay");

    let breakers = create_shared_breakers();
    let client = reqwest::Client::new();
    let mut cfg = loop_config(2);
    cfg.circuit_failure_threshold = 99;

    // Exhaust the budget -> dead-lettered.
    for _ in 0..2 {
        run_dispatch_once(&pool, &breakers, &client, &cfg)
            .await
            .unwrap();
    }
    assert_eq!(
        event_status(&pool, "ev_replay"),
        EventStatus::Failed.as_str()
    );

    let dlq_id = dlq_reason(&pool, "ev_replay");
    assert!(dlq_id.contains("500"));

    // Operator replays it via the API, which clears delivery_attempts.
    let app = {
        let config = aether_relay::config::AppConfig::load().unwrap();
        aether_relay::api::create_router(aether_relay::state::AppState {
            pool: pool.clone(),
            config,
        })
    };

    let dlq_row_id: String = pool
        .get()
        .unwrap()
        .query_row(
            "SELECT id FROM dead_letter_queue WHERE event_id = 'ev_replay'",
            [],
            |row| row.get(0),
        )
        .unwrap();

    let req = axum::http::Request::builder()
        .uri(format!("/v1/dlq/{dlq_row_id}/replay"))
        .method("POST")
        .body(axum::body::Body::empty())
        .unwrap();

    use tower::ServiceExt;
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    assert_eq!(
        event_status(&pool, "ev_replay"),
        EventStatus::Received.as_str()
    );
    assert_eq!(
        attempt_rows(&pool, "ev_replay"),
        0,
        "replay must reset the retry budget, otherwise it is one-shot"
    );

    // The replayed event gets a full budget again.
    for _ in 0..2 {
        run_dispatch_once(&pool, &breakers, &client, &cfg)
            .await
            .unwrap();
    }
    assert_eq!(attempt_rows(&pool, "ev_replay"), 2);
    assert_eq!(
        event_status(&pool, "ev_replay"),
        EventStatus::Failed.as_str()
    );
}
