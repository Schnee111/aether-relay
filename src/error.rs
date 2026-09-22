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

    #[error("Duplicate idempotency key: {status}")]
    DuplicateIdempotencyKey { status: String },

    #[error("Database error: {0}")]
    Database(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::Config(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONFIG_ERROR",
                e.to_string(),
            ),
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
            AppError::DuplicateIdempotencyKey { status: st } => (
                StatusCode::CONFLICT,
                "IDEMPOTENCY_CONFLICT",
                format!("Event with key already exists in state: {st}"),
            ),
            AppError::Database(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "DATABASE_ERROR",
                msg.clone(),
            ),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            AppError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                msg.clone(),
            ),
        };

        let body = Json(json!({
            "code": code,
            "error": message,
        }));

        (status, body).into_response()
    }
}
