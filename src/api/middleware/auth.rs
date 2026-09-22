use crate::error::AppError;
use axum::http::HeaderMap;

pub fn verify_api_key(headers: &HeaderMap, allowed_keys: &[String]) -> Result<(), AppError> {
    if allowed_keys.is_empty() {
        return Ok(());
    }

    let key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing X-Api-Key header".into()))?;

    if allowed_keys.iter().any(|k| k == key) {
        Ok(())
    } else {
        Err(AppError::Unauthorized("Invalid X-Api-Key".into()))
    }
}
