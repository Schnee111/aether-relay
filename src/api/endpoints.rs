use crate::api::middleware::auth::verify_api_key;
use crate::error::AppError;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateEndpointRequest {
    pub name: String,
    pub provider: String,
    pub secret: String,
    pub target_url: String,
}

#[derive(Debug, Serialize)]
pub struct EndpointResponse {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub target_url: String,
    pub created_at: i64,
}

pub async fn create_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateEndpointRequest>,
) -> Result<impl IntoResponse, AppError> {
    verify_api_key(&headers, &state.config.auth.api_keys)?;

    let pool = state.pool.clone();
    let id = tokio::task::spawn_blocking(move || -> Result<EndpointResponse, AppError> {
        let conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;
        let id = Uuid::now_v7().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO endpoints (id, name, provider, secret, target_url, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, req.name, req.provider, req.secret, req.target_url, now],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(EndpointResponse {
            id,
            name: req.name,
            provider: req.provider,
            target_url: req.target_url,
            created_at: now,
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok((StatusCode::CREATED, Json(json!(id))))
}

pub async fn list_endpoints(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    verify_api_key(&headers, &state.config.auth.api_keys)?;

    let pool = state.pool.clone();
    let items = tokio::task::spawn_blocking(move || -> Result<Vec<EndpointResponse>, AppError> {
        let conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, provider, target_url, created_at
                 FROM endpoints ORDER BY created_at DESC",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(EndpointResponse {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    provider: row.get(2)?,
                    target_url: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(rows.flatten().collect())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok((StatusCode::OK, Json(items)))
}

pub async fn delete_endpoint(
    State(state): State<AppState>,
    Path(endpoint_id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    verify_api_key(&headers, &state.config.auth.api_keys)?;

    let pool = state.pool.clone();
    let id_clone = endpoint_id.clone();
    let deleted = tokio::task::spawn_blocking(move || -> Result<bool, AppError> {
        let conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;
        let rows = conn
            .execute("DELETE FROM endpoints WHERE id = ?1", params![id_clone])
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(rows > 0)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!(
            "Endpoint {endpoint_id} not found"
        )))
    }
}
