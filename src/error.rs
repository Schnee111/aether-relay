use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Payload too large")]
    PayloadTooLarge,

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unsupported provider: {0}")]
    UnsupportedProvider(String),

    #[error("Duplicate idempotency key: {status}")]
    DuplicateIdempotencyKey { status: String },

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl AppError {
    /// HTTP status this error maps to. Kept separate from `into_response`
    /// so non-response callers (metrics) can classify failures too.
    pub fn status_code(&self) -> axum::http::StatusCode {
        match self {
            AppError::Config(_) | AppError::Database(_) | AppError::Internal(_) => {
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            }
            AppError::Unauthorized(_) | AppError::InvalidSignature => {
                axum::http::StatusCode::UNAUTHORIZED
            }
            AppError::PayloadTooLarge => axum::http::StatusCode::PAYLOAD_TOO_LARGE,
            AppError::BadRequest(_) | AppError::UnsupportedProvider(_) => {
                axum::http::StatusCode::BAD_REQUEST
            }
            AppError::DuplicateIdempotencyKey { .. } | AppError::Conflict(_) => {
                axum::http::StatusCode::CONFLICT
            }
            AppError::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            // Client-facing messages below carry only what the caller can act
            // on. Internal detail (SQLite text, filesystem paths, upstream
            // error strings) is logged and replaced with a generic message:
            // leaking it tells an unauthenticated caller about the internals
            // of the box and of every endpoint behind it.
            AppError::Config(e) => {
                tracing::error!(error = %e, "configuration error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "CONFIG_ERROR",
                    GENERIC_INTERNAL.to_string(),
                )
            }
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg.clone()),
            AppError::InvalidSignature => (
                StatusCode::UNAUTHORIZED,
                "INVALID_SIGNATURE",
                "Cryptographic signature mismatch or expired".into(),
            ),
            AppError::PayloadTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                "Request body exceeds maximum allowed size".into(),
            ),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", msg.clone()),
            AppError::UnsupportedProvider(p) => (
                StatusCode::BAD_REQUEST,
                "UNSUPPORTED_PROVIDER",
                format!("Unsupported provider: {p}"),
            ),
            AppError::DuplicateIdempotencyKey { status: st } => (
                StatusCode::CONFLICT,
                "IDEMPOTENCY_CONFLICT",
                format!("Event with key already exists in state: {st}"),
            ),
            AppError::Database(e) => {
                tracing::error!(error = %e, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "DATABASE_ERROR",
                    GENERIC_INTERNAL.to_string(),
                )
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "CONFLICT", msg.clone()),
            AppError::Internal(e) => {
                tracing::error!(error = %e, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    GENERIC_INTERNAL.to_string(),
                )
            }
        };

        let body = Json(json!({
            "code": code,
            "error": message,
        }));

        (status, body).into_response()
    }
}

/// One message for every 5xx. Deliberately reveals nothing about the cause.
const GENERIC_INTERNAL: &str = "Internal server error";
