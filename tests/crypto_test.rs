use aether_relay::crypto::{
    Provider, discord, generic, github, midtrans, stripe, verify_signature,
};
use axum::http::HeaderMap;
use ed25519_dalek::Signer;
use ed25519_dalek::SigningKey;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[test]
fn test_github_signature_valid_and_corrupted() {
    let secret = b"super-secret-key";
    let body = b"{\"action\":\"opened\",\"issue\":{\"number\":1}}";

    let mut mac = HmacSha256::new_from_slice(secret).unwrap();
    mac.update(body);
    let valid_hex = hex::encode(mac.finalize().into_bytes());
    let valid_header = format!("sha256={}", valid_hex);

    assert!(github::verify(secret, body, &valid_header).is_ok());

    // Corrupt last character
    let mut corrupted = valid_hex.clone();
    corrupted.pop();
    corrupted.push(if valid_hex.ends_with('0') { '1' } else { '0' });
    let corrupted_header = format!("sha256={}", corrupted);

    assert!(github::verify(secret, body, &corrupted_header).is_err());
}

#[test]
fn test_stripe_signature_and_replay_window() {
    let secret = b"whsec_test_secret";
    let body = b"{\"type\":\"charge.succeeded\"}";
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut mac = HmacSha256::new_from_slice(secret).unwrap();
    mac.update(format!("{}.{}", now, std::str::from_utf8(body).unwrap()).as_bytes());
    let valid_v1 = hex::encode(mac.finalize().into_bytes());

    let header = format!("t={},v1={}", now, valid_v1);
    assert!(stripe::verify(secret, body, &header, None).is_ok());

    // Expired timestamp (400 seconds ago, exceeding 300s window)
    let expired_ts = now - 400;
    let mut exp_mac = HmacSha256::new_from_slice(secret).unwrap();
    exp_mac.update(format!("{}.{}", expired_ts, std::str::from_utf8(body).unwrap()).as_bytes());
    let exp_v1 = hex::encode(exp_mac.finalize().into_bytes());
    let exp_header = format!("t={},v1={}", expired_ts, exp_v1);

    assert!(stripe::verify(secret, body, &exp_header, None).is_err());
}

#[test]
fn test_midtrans_signature() {
    let server_key = "SB-Mid-server-TEST123";
    let order_id = "ORDER-2026-001";
    let status_code = "200";
    let gross_amount = "150000.00";

    use sha2::{Digest, Sha512};
    let mut hasher = Sha512::new();
    hasher.update(format!("{}{}{}{}", order_id, status_code, gross_amount, server_key).as_bytes());
    let signature_key = hex::encode(hasher.finalize());

    let body = format!(
        r#"{{"order_id":"{}","status_code":"{}","gross_amount":"{}","signature_key":"{}"}}"#,
        order_id, status_code, gross_amount, signature_key
    );

    assert!(midtrans::verify(server_key, body.as_bytes()).is_ok());

    // Corrupted signature
    let corrupted_body = format!(
        r#"{{"order_id":"{}","status_code":"{}","gross_amount":"{}","signature_key":"deadbeef"}}"#,
        order_id, status_code, gross_amount
    );
    assert!(midtrans::verify(server_key, corrupted_body.as_bytes()).is_err());
}

#[test]
fn test_discord_ed25519_signature() {
    let secret_bytes: [u8; 32] = [42u8; 32];
    let signing_key = SigningKey::from_bytes(&secret_bytes);
    let verifying_key = signing_key.verifying_key();
    let pk_hex = hex::encode(verifying_key.to_bytes());

    let timestamp = "1720000000";
    let body = b"{\"type\":1}";

    let mut msg = Vec::new();
    msg.extend_from_slice(timestamp.as_bytes());
    msg.extend_from_slice(body);

    let signature = signing_key.sign(&msg);
    let sig_hex = hex::encode(signature.to_bytes());

    assert!(discord::verify(&pk_hex, body, &sig_hex, timestamp).is_ok());

    // Tampered body
    let tampered_body = b"{\"type\":2}";
    assert!(discord::verify(&pk_hex, tampered_body, &sig_hex, timestamp).is_err());
}

#[test]
fn test_generic_hmac() {
    let secret = b"shared_secret";
    let body = b"sample_event_payload";

    let mut mac = HmacSha256::new_from_slice(secret).unwrap();
    mac.update(body);
    let sig_hex = hex::encode(mac.finalize().into_bytes());

    assert!(generic::verify(secret, body, &sig_hex).is_ok());
    assert!(generic::verify(secret, body, "bad_hex_sig").is_err());
}

#[test]
fn test_provider_dispatch() {
    let secret = "github_secret";
    let body = b"{\"action\":\"test\"}";

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    let sig_hex = hex::encode(mac.finalize().into_bytes());

    let mut headers = HeaderMap::new();
    headers.insert(
        "x-hub-signature-256",
        format!("sha256={}", sig_hex).parse().unwrap(),
    );

    assert!(verify_signature(Provider::GitHub, secret, body, &headers).is_ok());
}
