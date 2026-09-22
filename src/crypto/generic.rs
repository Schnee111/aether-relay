use crate::error::AppError;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub fn verify(secret: &[u8], body: &[u8], signature_hex: &str) -> Result<(), AppError> {
    let expected = hex::decode(signature_hex.trim()).map_err(|_| AppError::InvalidSignature)?;

    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| AppError::InvalidSignature)?;
    mac.update(body);
    mac.verify_slice(&expected)
        .map_err(|_| AppError::InvalidSignature)
}
