use crate::error::AppError;
use sha2::{Digest, Sha512};
use subtle::ConstantTimeEq;

pub fn verify(server_key: &str, body: &[u8]) -> Result<(), AppError> {
    let json: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| AppError::InvalidSignature)?;

    let order_id = json["order_id"]
        .as_str()
        .ok_or(AppError::InvalidSignature)?;
    let status_code = json["status_code"]
        .as_str()
        .ok_or(AppError::InvalidSignature)?;
    let gross_amount = json["gross_amount"]
        .as_str()
        .ok_or(AppError::InvalidSignature)?;
    let signature_key = json["signature_key"]
        .as_str()
        .ok_or(AppError::InvalidSignature)?;

    let payload = format!("{}{}{}{}", order_id, status_code, gross_amount, server_key);
    let mut hasher = Sha512::new();
    hasher.update(payload.as_bytes());
    let computed_hex = hex::encode(hasher.finalize());

    if computed_hex
        .as_bytes()
        .ct_eq(signature_key.as_bytes())
        .into()
    {
        Ok(())
    } else {
        Err(AppError::InvalidSignature)
    }
}
