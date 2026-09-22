use crate::api::middleware::auth::verify_api_key;
use crate::core::idempotency::accept_event;
use crate::crypto::{Provider, verify_signature};
use crate::db::models::EndpointRecord;
use crate::error::AppError;
use crate::state::AppState;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use rusqlite::params;
use serde_json::json;
use std::str::FromStr;

/// Bounded so an oversized key cannot be persisted into the idempotency index.
const MAX_IDEMPOTENCY_KEY_LEN: usize = 255;

pub async fn handle_ingest(
    State(state): State<AppState>,
    Path(endpoint_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AppError> {
    if body.len() > state.config.server.body_limit_bytes {
        return Err(AppError::PayloadTooLarge);
    }

    verify_api_key(&headers, &state.config.auth.api_keys)?;

    // This header is a required part of the contract, not optional metadata.
    // Answering 500 for a missing client header told the caller the server had
    // broken when in fact their request was malformed.
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty() && v.len() <= MAX_IDEMPOTENCY_KEY_LEN)
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "Missing or invalid Idempotency-Key header (must be 1..={MAX_IDEMPOTENCY_KEY_LEN} characters)"
            ))
        })?
        .to_string();

    let pool = state.pool.clone();
    let ep_id = endpoint_id.clone();
    let endpoint = tokio::task::spawn_blocking(move || -> Result<EndpointRecord, AppError> {
        let conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;
        conn.query_row(
            "SELECT id, name, provider, secret, target_url, created_at FROM endpoints WHERE id = ?1",
            params![ep_id],
            |row| {
                Ok(EndpointRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    provider: row.get(2)?,
                    secret: row.get(3)?,
                    target_url: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound(format!("Endpoint {ep_id} not found"))
            }
            other => AppError::Database(other.to_string()),
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let provider = Provider::from_str(&endpoint.provider)?;
    verify_signature(provider, &endpoint.secret, &body, &headers)?;

    let mut header_map = serde_json::Map::new();
    for (name, val) in headers.iter() {
        if let Ok(str_val) = val.to_str() {
            header_map.insert(name.as_str().to_string(), json!(str_val));
        }
    }
    let headers_json = serde_json::Value::Object(header_map).to_string();

    let pool = state.pool.clone();
    let body_vec = body.to_vec();
    let event_id = tokio::task::spawn_blocking(move || -> Result<String, AppError> {
        let mut conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;
        accept_event(
            &mut conn,
            &endpoint_id,
            &idempotency_key,
            &body_vec,
            &headers_json,
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "id": event_id,
            "status": "RECEIVED"
        })),
    ))
}
