use crate::db::DbPool;
use crate::db::models::EventStatus;
use crate::error::AppError;
use crate::worker::circuit_breaker::{CircuitBreaker, SharedCircuitBreakers};
use reqwest::Client;
use rusqlite::params;
use std::time::Duration;
use uuid::Uuid;

pub struct DispatchResult {
    pub success: bool,
    pub status_code: Option<u16>,
    pub error: Option<String>,
    /// True when the event was withheld because the downstream circuit is open.
    /// The event has been returned to the queue and must not consume an attempt.
    pub deferred: bool,
}

/// Return a claimed event to the queue so the worker picks it up again.
///
/// A row left in PROCESSING is never re-claimed by anything, so every path that
/// abandons a delivery without settling it has to call this.
pub(crate) fn requeue_event(pool: &DbPool, event_id: &str) -> Result<(), AppError> {
    let conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;
    conn.execute(
        "UPDATE incoming_events SET status = ?1 WHERE id = ?2",
        params![EventStatus::Received.as_str(), event_id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub async fn dispatch_single_event(
    client: &Client,
    pool: &DbPool,
    breakers: &SharedCircuitBreakers,
    failure_threshold: u32,
    recovery: Duration,
    max_attempts: u32,
    event_id: &str,
) -> Result<DispatchResult, AppError> {
    // 1. Fetch event and endpoint
    let pool_clone = pool.clone();
    let ev_id = event_id.to_string();
    let (target_url, raw_body, endpoint_id, attempt_count) = tokio::task::spawn_blocking(
        move || -> Result<(String, Vec<u8>, String, u32), AppError> {
            let conn = pool_clone
                .get()
                .map_err(|e| AppError::Database(e.to_string()))?;

            let (endpoint_id, raw_body): (String, Vec<u8>) = conn
                .query_row(
                    "SELECT endpoint_id, raw_body FROM incoming_events WHERE id = ?1",
                    params![ev_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(|e| AppError::Database(e.to_string()))?;

            let target_url: String = conn
                .query_row(
                    "SELECT target_url FROM endpoints WHERE id = ?1",
                    params![endpoint_id],
                    |row| row.get(0),
                )
                .map_err(|e| AppError::Database(e.to_string()))?;

            let attempt_count: u32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM delivery_attempts WHERE event_id = ?1",
                    params![ev_id],
                    |row| row.get(0),
                )
                .map_err(|e| AppError::Database(e.to_string()))?;

            Ok((target_url, raw_body, endpoint_id, attempt_count))
        },
    )
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    // 2. Check Circuit Breaker. The guard is scoped so it is released before any
    // await below.
    let circuit_open = {
        let mut map = breakers.write().unwrap();
        let breaker = map
            .entry(endpoint_id.clone())
            .or_insert_with(|| CircuitBreaker::new(failure_threshold, recovery));
        !breaker.can_attempt()
    };

    if circuit_open {
        // Hand the event straight back to the queue. Leaving it in PROCESSING
        // would strand it forever, and charging it an attempt would burn the
        // retry budget on a delivery that was never actually tried.
        let requeue_pool = pool.clone();
        let id = event_id.to_string();
        tokio::task::spawn_blocking(move || requeue_event(&requeue_pool, &id))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??;

        return Ok(DispatchResult {
            success: false,
            status_code: None,
            error: Some("Circuit breaker OPEN".into()),
            deferred: true,
        });
    }

    // 3. Perform HTTP Dispatch
    let attempt_num = attempt_count + 1;
    let res = client
        .post(&target_url)
        .header("Content-Type", "application/json")
        .body(raw_body)
        .send()
        .await;

    let (success, status_code, error_msg) = match res {
        Ok(resp) => {
            let status = resp.status().as_u16();
            if resp.status().is_success() {
                (true, Some(status), None)
            } else {
                (
                    false,
                    Some(status),
                    Some(format!("Downstream HTTP {status}")),
                )
            }
        }
        Err(e) => (false, None, Some(e.to_string())),
    };

    // 4. Update Circuit Breaker
    {
        let mut map = breakers.write().unwrap();
        if let Some(breaker) = map.get_mut(&endpoint_id) {
            if success {
                breaker.record_success();
            } else {
                breaker.record_failure();
            }
        }
    }

    // 5. Record attempt and update event status in DB
    let pool_clone = pool.clone();
    let ev_id = event_id.to_string();
    let err_clone = error_msg.clone();
    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let conn = pool_clone
            .get()
            .map_err(|e| AppError::Database(e.to_string()))?;

        let attempt_id = Uuid::now_v7().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO delivery_attempts (id, event_id, attempt_number, response_status, error_message, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![attempt_id, ev_id, attempt_num, status_code, err_clone, now],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        if success {
            conn.execute(
                "UPDATE incoming_events SET status = ?1 WHERE id = ?2",
                params![EventStatus::Delivered.as_str(), ev_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        } else if attempt_num >= max_attempts {
            // Budget exhausted: park the event and record it for forensics.
            conn.execute(
                "UPDATE incoming_events SET status = ?1 WHERE id = ?2",
                params![EventStatus::Failed.as_str(), ev_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

            let dlq_id = Uuid::now_v7().to_string();
            let reason = err_clone.unwrap_or_else(|| "Max attempts exceeded".into());
            conn.execute(
                "INSERT OR REPLACE INTO dead_letter_queue (id, event_id, endpoint_id, error_reason, last_attempt_status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![dlq_id, ev_id, endpoint_id, reason, status_code, now],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        } else {
            // Budget remains: hand the event back to the queue so the worker can
            // retry it. Returning it to RECEIVED here is what makes retries
            // actually happen; leaving it in PROCESSING would strand it.
            conn.execute(
                "UPDATE incoming_events SET status = ?1 WHERE id = ?2",
                params![EventStatus::Received.as_str(), ev_id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        }

        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(DispatchResult {
        success,
        status_code,
        error: error_msg,
        deferred: false,
    })
}
