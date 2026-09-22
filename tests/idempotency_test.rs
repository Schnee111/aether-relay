use aether_relay::core::idempotency::{accept_event, transition_status};
use aether_relay::db::migrations::run_migrations;
use aether_relay::db::models::EventStatus;
use aether_relay::error::AppError;
use rusqlite::Connection;

#[test]
fn test_idempotency_accept_and_duplicate() {
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    let endpoint_id = "ep_test_1";
    let key = "idem_key_123";
    let body = b"{\"event\":\"payment.succeeded\"}";
    let headers = "{\"content-type\":\"application/json\"}";

    // First attempt must succeed
    let event_id = accept_event(&mut conn, endpoint_id, key, body, headers).unwrap();
    assert!(!event_id.is_empty());

    // Second attempt with same endpoint and key must fail with DuplicateIdempotencyKey
    let err = accept_event(&mut conn, endpoint_id, key, body, headers).unwrap_err();
    match err {
        AppError::DuplicateIdempotencyKey { status } => {
            assert_eq!(status, "RECEIVED");
        }
        other => panic!("Expected DuplicateIdempotencyKey, got {:?}", other),
    }

    // Different key must succeed
    let event_id_2 = accept_event(&mut conn, endpoint_id, "idem_key_456", body, headers).unwrap();
    assert_ne!(event_id, event_id_2);
}

#[test]
fn test_status_transitions() {
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();

    let event_id = accept_event(&mut conn, "ep_1", "key_1", b"test", "{}").unwrap();

    // Transition from RECEIVED to PROCESSING
    let ok = transition_status(
        &mut conn,
        &event_id,
        EventStatus::Received,
        EventStatus::Processing,
    )
    .unwrap();
    assert!(ok);

    // Invalid transition: from RECEIVED (now PROCESSING) to DELIVERED must return false
    let failed = transition_status(
        &mut conn,
        &event_id,
        EventStatus::Received,
        EventStatus::Delivered,
    )
    .unwrap();
    assert!(!failed);

    // Transition from PROCESSING to DELIVERED
    let ok2 = transition_status(
        &mut conn,
        &event_id,
        EventStatus::Processing,
        EventStatus::Delivered,
    )
    .unwrap();
    assert!(ok2);
}
