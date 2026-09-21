# Technical Architecture Specification (SPEC) — AetherRelay Rust

## 1. Technology Stack

| Layer | Choice | Rationale |
| :--- | :--- | :--- |
| Language | Rust (2024 edition, stable) | Zero-cost abstractions, no GC, memory safety |
| Async Runtime | tokio 1.x (multi-thread) | Industry standard, mature ecosystem |
| HTTP Framework | Axum 0.8+ (tower-based) | Best extractor ergonomics, Tower middleware reuse, first-class raw body extraction via `axum::body::Bytes` |
| Database | rusqlite 0.32+ (synchronous) | Direct C binding, lowest overhead for single-writer SQLite. Async wrapper unnecessary — SQLite is single-writer and rusqlite on `spawn_blocking` is optimal |
| Connection Pool | r2d2-rusqlite | Proven pool for synchronous connections, integrates with tokio via `spawn_blocking` |
| Crypto (HMAC) | hmac + sha2 (RustCrypto) | Pure Rust, constant-time `verify_slice()`, no system dependency on OpenSSL |
| Crypto (Ed25519) | ed25519-dalek 2.x | Standard for Discord interactions, `verify_strict()` prevents signature malleability |
| Serialization | serde + serde_json | De facto standard |
| Logging | tracing + tracing-subscriber | Structured spans, async-safe, JSON output via `tracing-subscriber::fmt::json()` |
| Metrics | axum-prometheus (metrics.rs) | Drop-in Axum layer, Prometheus text exposition |
| Config | config 0.14+ | TOML file + env override + typed deserialization |
| Error Handling | thiserror (library errors) | Compile-time error variants, no runtime overhead |
| HTTP Client | reqwest 0.12+ | Connection pooling, async, rustls-tls |
| CLI/Build | cargo, cargo-audit, clippy | Standard Rust toolchain |
| Testing | cargo test, proptest | Built-in + property-based testing |

## 2. Project Layout

```
aether-relay/
├── Cargo.toml
├── Cargo.lock
├── config/
│   └── default.toml            # Default configuration
├── src/
│   ├── main.rs                 # Entrypoint: config load, tracing init, server start
│   ├── config.rs               # Typed config structs (serde + config crate)
│   ├── error.rs                # AppError enum (thiserror), IntoResponse impl
│   ├── api/
│   │   ├── mod.rs              # Router assembly
│   │   ├── ingest.rs           # POST /v1/ingest/:endpoint_id
│   │   ├── endpoints.rs        # CRUD /v1/endpoints
│   │   ├── dlq.rs              # /v1/dlq routes
│   │   ├── health.rs           # GET /health
│   │   └── middleware/
│   │       ├── auth.rs         # API key gate (Tower layer)
│   │       └── body_limit.rs   # Request body size limit
│   ├── crypto/
│   │   ├── mod.rs              # VerifyResult enum, provider dispatch
│   │   ├── github.rs           # HMAC-SHA256
│   │   ├── stripe.rs           # v1 timestamped + replay window
│   │   ├── midtrans.rs         # SHA-512
│   │   ├── discord.rs          # Ed25519 (ed25519-dalek)
│   │   └── generic.rs          # Configurable HMAC
│   ├── db/
│   │   ├── mod.rs              # Pool initialization + pragma config
│   │   ├── migrations.rs       # Schema DDL (embedded SQL)
│   │   └── models.rs           # Row structs
│   ├── core/
│   │   └── idempotency.rs      # Atomic CAS state machine
│   ├── worker/
│   │   ├── mod.rs              # Dispatch loop spawner
│   │   ├── dispatcher.rs       # Jittered backoff dispatch
│   │   └── circuit_breaker.rs  # Per-endpoint state machine
│   └── observability/
│       └── metrics.rs          # Prometheus setup
├── tests/
│   ├── integration/
│   │   ├── ingest_test.rs
│   │   ├── crypto_test.rs
│   │   ├── idempotency_test.rs
│   │   └── dlq_test.rs
│   └── property/
│       └── idempotency_prop.rs # proptest
├── benches/
│   ├── ingest_bench.rs         # criterion
│   └── sqlite_bench.rs         # criterion
├── scripts/
│   └── kill9-drill.sh
├── docs/
├── Dockerfile
├── .github/workflows/ci.yml
└── README.md
```

## 3. Data Flow Architecture

```
Provider (GitHub/Stripe/Midtrans/Discord)
  │
  │  POST /v1/ingest/:endpoint_id
  │  Headers: signature, timestamp, idempotency-key, api-key
  │  Body: raw JSON bytes
  │
  ▼
┌─────────────────────────────────────────┐
│  Axum Router                            │
│  ├── Tower Layer: API Key Auth          │
│  ├── Tower Layer: Body Limit (1 MB)     │
│  ├── Tower Layer: Prometheus Metrics    │
│  └── Tower Layer: Request Tracing       │
└──────────────────┬──────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────┐
│  Ingestion Handler                      │
│  1. Extract raw body as Bytes           │
│  2. Lookup endpoint config from DB      │
│  3. Dispatch to crypto adapter          │
│  4. Verify signature (constant-time)    │
│  5. Idempotency check (CAS)            │
│  6. Persist event (status: RECEIVED)    │
│  7. Return 202 Accepted                 │
└──────────────────┬──────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────┐
│  SQLite WAL (rusqlite)                  │
│  PRAGMA journal_mode = WAL              │
│  PRAGMA synchronous = NORMAL            │
│  PRAGMA busy_timeout = 5000             │
│  PRAGMA mmap_size = 268435456 (256 MB)  │
│  PRAGMA cache_size = -64000 (64 MB)     │
│                                         │
│  Tables:                                │
│  - endpoints (provider, secret, url)    │
│  - incoming_events (idempotency CAS)    │
│  - delivery_attempts (status, retry)    │
│  - dead_letter_queue (forensic)         │
└──────────────────┬──────────────────────┘
                   │
                   ▼  (tokio::spawn background task)
┌─────────────────────────────────────────┐
│  Dispatch Worker                        │
│  1. Poll RECEIVED events (lease-based)  │
│  2. Transition to PROCESSING            │
│  3. HTTP POST to downstream target      │
│  4. On success: DELIVERED               │
│  5. On failure: increment attempt       │
│     a. Retry with decorrelated jitter   │
│     b. Circuit breaker check            │
│     c. Max attempts → DLQ eviction      │
└─────────────────────────────────────────┘
```

## 4. Cryptographic Verification Architecture

### 4.1 Provider Adapter Dispatch

```rust
pub enum Provider {
    GitHub,
    Stripe,
    Midtrans,
    Discord,
    Generic,
}

pub fn verify(
    provider: Provider,
    secret: &[u8],
    raw_body: &[u8],
    headers: &HeaderMap,
) -> Result<(), CryptoError> {
    match provider {
        Provider::GitHub   => github::verify(secret, raw_body, headers),
        Provider::Stripe   => stripe::verify(secret, raw_body, headers),
        Provider::Midtrans => midtrans::verify(secret, raw_body, headers),
        Provider::Discord  => discord::verify(secret, raw_body, headers),
        Provider::Generic  => generic::verify(secret, raw_body, headers),
    }
}
```

### 4.2 HMAC Verification (RustCrypto)

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub fn verify_hmac_sha256(
    secret: &[u8],
    message: &[u8],
    signature_hex: &str,
) -> Result<(), CryptoError> {
    let expected_bytes = hex::decode(signature_hex)?;
    let mut mac = HmacSha256::new_from_slice(secret)?;
    mac.update(message);
    // verify_slice is constant-time internally
    mac.verify_slice(&expected_bytes)
        .map_err(|_| CryptoError::SignatureMismatch)
}
```

### 4.3 Ed25519 Verification (Discord)

```rust
use ed25519_dalek::{Signature, VerifyingKey};

pub fn verify_ed25519(
    public_key_hex: &str,
    timestamp: &str,
    body: &[u8],
    signature_hex: &str,
) -> Result<(), CryptoError> {
    let sig_bytes: [u8; 64] = hex::decode(signature_hex)?
        .try_into()
        .map_err(|_| CryptoError::InvalidSignatureLength)?;
    let pk_bytes: [u8; 32] = hex::decode(public_key_hex)?
        .try_into()
        .map_err(|_| CryptoError::InvalidKeyLength)?;

    let signature = Signature::from_bytes(&sig_bytes);
    let verifying_key = VerifyingKey::from_bytes(&pk_bytes)?;

    let mut msg = Vec::with_capacity(timestamp.len() + body.len());
    msg.extend_from_slice(timestamp.as_bytes());
    msg.extend_from_slice(body);

    verifying_key.verify_strict(&msg, &signature)
        .map_err(|_| CryptoError::SignatureMismatch)
}
```

## 5. Idempotency Engine (Atomic CAS)

```rust
pub fn accept_event(
    conn: &rusqlite::Connection,
    endpoint_id: &str,
    idempotency_key: &str,
    raw_body: &[u8],
) -> Result<EventId, IdempotencyError> {
    let tx = conn.transaction_with_behavior(
        rusqlite::TransactionBehavior::Immediate
    )?;

    // Check existing
    let existing: Option<String> = tx.query_row(
        "SELECT status FROM incoming_events
         WHERE endpoint_id = ?1 AND idempotency_key = ?2",
        params![endpoint_id, idempotency_key],
        |row| row.get(0),
    ).optional()?;

    if let Some(status) = existing {
        return Err(IdempotencyError::Duplicate { status });
    }

    let event_id = uuid7::uuid7().to_string();
    tx.execute(
        "INSERT INTO incoming_events
         (id, endpoint_id, idempotency_key, raw_body, status, created_at)
         VALUES (?1, ?2, ?3, ?4, 'RECEIVED', strftime('%s','now'))",
        params![event_id, endpoint_id, idempotency_key, raw_body],
    )?;

    tx.commit()?;
    Ok(EventId(event_id))
}
```

## 6. Dispatch Worker

### 6.1 Decorrelated Jitter Backoff (AWS Formula)

```rust
use rand::Rng;
use std::time::Duration;

pub fn decorrelated_jitter(
    base: Duration,
    cap: Duration,
    prev_sleep: Duration,
) -> Duration {
    let mut rng = rand::rng();
    let base_ms = base.as_millis() as u64;
    let cap_ms = cap.as_millis() as u64;
    let prev_ms = prev_sleep.as_millis() as u64;

    let sleep_ms = rng.random_range(base_ms..=(prev_ms.saturating_mul(3)));
    Duration::from_millis(sleep_ms.min(cap_ms))
}
```

### 6.2 Circuit Breaker

```rust
pub enum CircuitState {
    Closed,
    Open { until: Instant },
    HalfOpen,
}

pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    failure_threshold: u32,    // default: 5
    recovery_timeout: Duration, // default: 30s
    half_open_max: u32,        // default: 2
}
```

## 7. Configuration Schema

```toml
# config/default.toml

[server]
host = "0.0.0.0"
port = 3000
body_limit_bytes = 1_048_576  # 1 MB

[database]
path = "./data/aether-relay.db"
busy_timeout_ms = 5000
mmap_size = 268435456  # 256 MB
cache_size = -64000    # 64 MB
pool_size = 4

[auth]
api_keys = []  # override via env: RELAY_AUTH__API_KEYS

[worker]
poll_interval_ms = 100
max_attempts = 5
backoff_base_ms = 1000
backoff_cap_ms = 60000
circuit_failure_threshold = 5
circuit_recovery_timeout_secs = 30

[logging]
level = "info"
format = "json"  # "json" | "pretty"

[metrics]
enabled = true
```

## 8. Deployment

### 8.1 Static Binary (musl)

```dockerfile
FROM rust:1.82-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM scratch
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/aether-relay /
COPY --from=builder /app/config/default.toml /config/default.toml
EXPOSE 3000
ENTRYPOINT ["/aether-relay"]
```

### 8.2 CI Pipeline

```yaml
name: CI
on: [push, pull_request]
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test
      - run: cargo audit
```
