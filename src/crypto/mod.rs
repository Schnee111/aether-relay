pub mod discord;
pub mod generic;
pub mod github;
pub mod midtrans;
pub mod stripe;

use crate::error::AppError;
use axum::http::HeaderMap;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    GitHub,
    Stripe,
    Midtrans,
    Discord,
    Generic,
}

impl FromStr for Provider {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "github" => Ok(Self::GitHub),
            "stripe" => Ok(Self::Stripe),
            "midtrans" => Ok(Self::Midtrans),
            "discord" => Ok(Self::Discord),
            "generic" => Ok(Self::Generic),
            other => Err(AppError::Internal(format!("Unsupported provider: {other}"))),
        }
    }
}

pub fn verify_signature(
    provider: Provider,
    secret: &str,
    body: &[u8],
    headers: &HeaderMap,
) -> Result<(), AppError> {
    match provider {
        Provider::GitHub => {
            let sig = headers
                .get("x-hub-signature-256")
                .and_then(|v| v.to_str().ok())
                .ok_or(AppError::InvalidSignature)?;
            github::verify(secret.as_bytes(), body, sig)
        }
        Provider::Stripe => {
            let sig = headers
                .get("stripe-signature")
                .and_then(|v| v.to_str().ok())
                .ok_or(AppError::InvalidSignature)?;
            stripe::verify(secret.as_bytes(), body, sig, None)
        }
        Provider::Midtrans => midtrans::verify(secret, body),
        Provider::Discord => {
            let sig = headers
                .get("x-signature-ed25519")
                .and_then(|v| v.to_str().ok())
                .ok_or(AppError::InvalidSignature)?;
            let ts = headers
                .get("x-signature-timestamp")
                .and_then(|v| v.to_str().ok())
                .ok_or(AppError::InvalidSignature)?;
            discord::verify(secret, body, sig, ts)
        }
        Provider::Generic => {
            let sig = headers
                .get("x-signature")
                .or_else(|| headers.get("signature"))
                .and_then(|v| v.to_str().ok())
                .ok_or(AppError::InvalidSignature)?;
            generic::verify(secret.as_bytes(), body, sig)
        }
    }
}
