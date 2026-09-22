use crate::api::middleware::auth::verify_api_key;
use crate::db::models::EventStatus;
use crate::error::AppError;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use rusqlite::params;
use serde_json::json;

pub async fn list_dlq(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    verify_api_key(&headers, &state.config.auth.api_keys)?;

    let pool = state.pool.clone();
    let items = tokio::task::spawn_blocking(move || -> Result<Vec<serde_json::Value>, AppError> {
        let conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, event_id, endpoint_id, error_reason, last_attempt_status, created_at
                 FROM dead_letter_queue ORDER BY created_at DESC LIMIT 100",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(json!({
                    "id": row.get::<_, String>(0)?,
                    "event_id": row.get::<_, String>(1)?,
                    "endpoint_id": row.get::<_, String>(2)?,
                    "error_reason": row.get::<_, String>(3)?,
                    "last_attempt_status": row.get::<_, Option<u16>>(4)?,
                    "created_at": row.get::<_, i64>(5)?,
                }))
            })
            .map_err(|e| AppError::Database(e.to_string()))?;

        let mut list = Vec::new();
        for val in rows.flatten() {
            list.push(val);
        }
        Ok(list)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok((StatusCode::OK, Json(items)))
}

pub async fn replay_dlq(
    State(state): State<AppState>,
    Path(dlq_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    verify_api_key(&headers, &state.config.auth.api_keys)?;

    let pool = state.pool.clone();
    let id_clone = dlq_id.clone();
    let event_id = tokio::task::spawn_blocking(move || -> Result<String, AppError> {
        let conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;

        let event_id: String = conn
            .query_row(
                "SELECT event_id FROM dead_letter_queue WHERE id = ?1",
                params![id_clone],
                |row| row.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::NotFound(format!("DLQ record {id_clone} not found"))
                }
                other => AppError::Database(other.to_string()),
            })?;

        conn.execute(
            "UPDATE incoming_events SET status = ?1 WHERE id = ?2",
            params![EventStatus::Received.as_str(), event_id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        conn.execute(
            "DELETE FROM dead_letter_queue WHERE id = ?1",
            params![id_clone],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(event_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok((
        StatusCode::OK,
        Json(json!({
            "status": "replayed",
            "event_id": event_id
        })),
    ))
}
