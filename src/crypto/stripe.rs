use crate::error::AppError;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const DEFAULT_TOLERANCE_SECS: u64 = 300;

pub fn verify(
    secret: &[u8],
    body: &[u8],
    signature_header: &str,
    tolerance_secs: Option<u64>,
) -> Result<(), AppError> {
    let mut timestamp: Option<u64> = None;
    let mut signatures: Vec<&str> = Vec::new();

    for item in signature_header.split(',') {
        let parts: Vec<&str> = item.splitn(2, '=').collect();
        if parts.len() == 2 {
            let key = parts[0].trim();
            let val = parts[1].trim();
            if key == "t" {
                if let Ok(ts) = val.parse::<u64>() {
                    timestamp = Some(ts);
                }
            } else if key == "v1" {
                signatures.push(val);
            }
        }
    }

    let ts = timestamp.ok_or(AppError::InvalidSignature)?;
    if signatures.is_empty() {
        return Err(AppError::InvalidSignature);
    }

    let tolerance = tolerance_secs.unwrap_or(DEFAULT_TOLERANCE_SECS);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if now.abs_diff(ts) > tolerance {
        return Err(AppError::InvalidSignature);
    }

    let mut payload = Vec::with_capacity(20 + 1 + body.len());
    payload.extend_from_slice(ts.to_string().as_bytes());
    payload.push(b'.');
    payload.extend_from_slice(body);

    for sig_hex in signatures {
        if let Ok(expected) = hex::decode(sig_hex)
            && let Ok(mut mac) = HmacSha256::new_from_slice(secret)
        {
            mac.update(&payload);
            if mac.verify_slice(&expected).is_ok() {
                return Ok(());
            }
        }
    }

    Err(AppError::InvalidSignature)
}
