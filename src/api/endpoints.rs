use crate::api::middleware::auth::verify_api_key;
use crate::crypto::Provider;
use crate::error::AppError;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
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

/// Validate an endpoint at creation time so a bad configuration fails loudly on
/// the operator's request instead of silently poisoning every later ingest.
fn validate_new_endpoint(req: &CreateEndpointRequest) -> Result<(), AppError> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }

    // Reject an unknown provider here. Storing it would create a webhook that
    // can never verify a signature: ingestion would fail at dispatch time with
    // a server error rather than at setup with a clear message.
    Provider::from_str(&req.provider)?;

    if req.secret.is_empty() {
        return Err(AppError::BadRequest("secret must not be empty".into()));
    }

    validate_target_url(&req.target_url)
}

/// Gate outbound destinations that would turn the relay into an SSRF pivot.
///
/// The relay fetches `target_url` on every delivery, so an attacker who can
/// create an endpoint could otherwise aim it at loopback, private ranges, or
/// the cloud metadata service and have the relay fetch internal resources on
/// their behalf.
fn validate_target_url(raw: &str) -> Result<(), AppError> {
    let url = reqwest::Url::parse(raw)
        .map_err(|e| AppError::BadRequest(format!("target_url is not a valid URL: {e}")))?;

    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(AppError::BadRequest(format!(
                "target_url scheme must be http or https, got {other}"
            )));
        }
    }

    let host = url
        .host_str()
        .ok_or_else(|| AppError::BadRequest("target_url must have a host".into()))?;

    if is_blocked_host(host) {
        return Err(AppError::BadRequest(format!(
            "target_url host {host} is not routable from the relay (loopback, private, or link-local address)"
        )));
    }

    Ok(())
}

fn is_blocked_host(host: &str) -> bool {
    let bare = host.trim_start_matches('[').trim_end_matches(']');

    // Hostnames that resolve to the machine itself.
    if bare.eq_ignore_ascii_case("localhost") || bare.ends_with(".localhost") {
        return true;
    }

    if let Ok(ip) = bare.parse::<std::net::IpAddr>() {
        return is_blocked_ip(ip);
    }

    // An unresolvable name is not our problem here; the delivery will fail and
    // be retried/dead-lettered like any other unreachable downstream.
    false
}

fn is_blocked_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local() // 169.254.0.0/16, incl. 169.254.169.254
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_documentation()
                // 100.64.0.0/10 carrier-grade NAT, handled via octet check
                // because the std helper for it is not stable.
                || (v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                // Unique local addresses fc00::/7
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                // Link-local fe80::/10
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                // IPv4-mapped addresses, unwrapped and re-checked
                || v6
                    .to_ipv4_mapped()
                    .map(|v4| is_blocked_ip(std::net::IpAddr::V4(v4)))
                    .unwrap_or(false)
        }
    }
}

pub async fn create_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateEndpointRequest>,
) -> Result<impl IntoResponse, AppError> {
    verify_api_key(&headers, &state.config.auth.api_keys)?;
    validate_new_endpoint(&req)?;

    let pool = state.pool.clone();
    let response = tokio::task::spawn_blocking(move || -> Result<EndpointResponse, AppError> {
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

    // Return the same shape as the list route. This previously returned a bare
    // JSON string, so the documented create-then-ingest flow could not be
    // followed by a client reading `id` out of the response.
    Ok((StatusCode::CREATED, Json(response)))
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

        let mut list = Vec::new();
        for val in rows {
            list.push(val.map_err(|e| AppError::Database(e.to_string()))?);
        }
        Ok(list)
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
        let mut conn = pool.get().map_err(|e| AppError::Database(e.to_string()))?;
        let tx = conn
            .transaction()
            .map_err(|e| AppError::Database(e.to_string()))?;

        // If any event still exists for this endpoint, refuse. Leaving events
        // behind with a dangling endpoint_id would produce deliveries that can
        // never resolve a target, and (before the cascade rename below) would
        // strand them permanently.
        let pending: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM incoming_events
                 WHERE endpoint_id = ?1 AND status IN ('RECEIVED', 'PROCESSING')",
                params![id_clone],
                |row| row.get(0),
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        if pending > 0 {
            return Err(AppError::Conflict(format!(
                "Endpoint {id_clone} still has {pending} undelivered event(s); \
                 drain or replay them before deleting"
            )));
        }

        let rows = tx
            .execute("DELETE FROM endpoints WHERE id = ?1", params![id_clone])
            .map_err(|e| AppError::Database(e.to_string()))?;

        tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
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
