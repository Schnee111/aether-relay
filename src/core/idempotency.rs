use crate::db::models::EventStatus;
use crate::error::AppError;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use uuid::Uuid;

pub fn accept_event(
    conn: &mut Connection,
    endpoint_id: &str,
    idempotency_key: &str,
    raw_body: &[u8],
    headers: &str,
) -> Result<String, AppError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| AppError::Database(e.to_string()))?;

    let existing: Option<String> = tx
        .query_row(
            "SELECT status FROM incoming_events WHERE endpoint_id = ?1 AND idempotency_key = ?2",
            params![endpoint_id, idempotency_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| AppError::Database(e.to_string()))?;

    if let Some(status) = existing {
        return Err(AppError::DuplicateIdempotencyKey { status });
    }

    let event_id = Uuid::now_v7().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    tx.execute(
        "INSERT INTO incoming_events (id, endpoint_id, idempotency_key, raw_body, headers, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'RECEIVED', ?6)",
        params![event_id, endpoint_id, idempotency_key, raw_body, headers, now],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
    Ok(event_id)
}

pub fn transition_status(
    conn: &mut Connection,
    event_id: &str,
    from: EventStatus,
    to: EventStatus,
) -> Result<bool, AppError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| AppError::Database(e.to_string()))?;

    let rows_affected = tx
        .execute(
            "UPDATE incoming_events SET status = ?1 WHERE id = ?2 AND status = ?3",
            params![to.as_str(), event_id, from.as_str()],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

    tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
    Ok(rows_affected > 0)
}
