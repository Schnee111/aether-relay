use crate::error::AppError;
use ed25519_dalek::{Signature, VerifyingKey};

pub fn verify(
    public_key_hex: &str,
    body: &[u8],
    signature_hex: &str,
    timestamp: &str,
) -> Result<(), AppError> {
    let pk_bytes = hex::decode(public_key_hex).map_err(|_| AppError::InvalidSignature)?;
    let sig_bytes = hex::decode(signature_hex).map_err(|_| AppError::InvalidSignature)?;

    let pk_array: [u8; 32] = pk_bytes
        .try_into()
        .map_err(|_| AppError::InvalidSignature)?;
    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| AppError::InvalidSignature)?;

    let verifying_key =
        VerifyingKey::from_bytes(&pk_array).map_err(|_| AppError::InvalidSignature)?;
    let signature = Signature::from_bytes(&sig_array);

    let mut message = Vec::with_capacity(timestamp.len() + body.len());
    message.extend_from_slice(timestamp.as_bytes());
    message.extend_from_slice(body);

    verifying_key
        .verify_strict(&message, &signature)
        .map_err(|_| AppError::InvalidSignature)
}
