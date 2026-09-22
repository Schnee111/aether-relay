# AetherRelay

> **High-Performance Webhook Ingestion & Reliable Dispatch Gateway**  
> *A crash-resilient, at-least-once webhook shock-absorber with ingest-time deduplication, built with Axum and embedded SQLite WAL.*

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust: Stable](https://img.shields.io/badge/Rust-stable-red.svg?logo=rust)](https://www.rust-lang.org/)
[![Axum](https://img.shields.io/badge/Axum-v0.8-orange.svg?logo=tower)](https://axum.rs/)
[![SQLite WAL](https://img.shields.io/badge/SQLite-WAL%20Mode-003B57.svg?logo=sqlite)](https://sqlite.org/wal.html)
[![rusqlite](https://img.shields.io/badge/rusqlite-Synchronous-blue.svg)](https://docs.rs/rusqlite/)
[![tokio](https://img.shields.io/badge/tokio-Async-dc6fff.svg?logo=tokio)](https://tokio.rs/)
[![tests](https://github.com/Schnee111/aether-relay/actions/workflows/ci.yml/badge.svg)](https://github.com/Schnee111/aether-relay/actions/workflows/ci.yml)
[![audit](https://img.shields.io/badge/cargo--audit-clean-brightgreen.svg)](https://rustsec.org/advisories/)
[![bin: <10MB](https://img.shields.io/badge/bin-size-%3C10MB-brightgreen.svg)](https://github.com/Schnee111/aether-relay/releases)

## Tech Stack

| Layer | Technology |
| :--- | :--- |
| Language | Rust 2024 edition (stable) |
| Async Runtime | tokio 1.x (multi-thread, full features) |
| HTTP Server | Axum 0.8 + Tower middleware |
| Database | SQLite 3 WAL mode via rusqlite + r2d2 pool |
| Crypto | hmac + sha2 (RustCrypto) + ed25519-dalek (constant-time) |
| Observability | tracing + tracing-subscriber (JSON/pretty) |
| Testing | cargo test + tower-http for integration |
| Build | cargo + musl static linking |

---

## Why AetherRelay?

Connecting third-party webhooks (Stripe, GitHub, Midtrans, Discord) directly to your backend introduces severe reliability hazards:

- **Retry Storms & Double Execution** — Aggressive provider retries during slow network conditions cause duplicate processing without atomic idempotency guards.
- **Downstream Outages & Data Loss** — Temporary database locks or deployment restarts cause webhooks to fail and be permanently lost.
- **Cryptographic Vulnerabilities** — Native string equality comparisons leak timing signals, exposing HMAC verification to byte-by-byte attack.
- **Cascading Service Failures** — Hammering a struggling internal service without backoff or circuit breaking worsens degradation.

**AetherRelay** sits as a lightweight shock absorber in front of your infrastructure:

- Ingests incoming payloads in **< 10ms** returning `HTTP 202 Accepted`.
- Captures raw stream bytes for **constant-time cryptographic verification** (HMAC-SHA256, SHA-512, Ed25519).
- Atomically deduplicates replayed webhooks via SQLite compare-and-swap transactions, so a provider retry is never persisted twice.
- Retries downstream delivery with **Decorrelated Jitter Exponential Backoff** and isolates poisoned events into a forensic **Dead-Letter Queue (DLQ)**.

---

## Architecture

```mermaid
flowchart TD
    subgraph Sources["Webhook Sources"]
        GH["GitHub\nHMAC-SHA256"]
        ST["Stripe\nv1 Timestamped"]
        MT["Midtrans\nSHA-512"]
        DC["Discord\nEd25519"]
        GN["Generic\nHMAC"]
    end

    Sources -->|"POST /v1/ingest/:endpointId"| IL

    subgraph IL["Ingestion Layer"]
        direction LR
        RH["Axum Raw Body\nBytes Extraction"] --> SV["Signature\nVerification"]
        SV --> IG["Idempotency Guard\nBEGIN IMMEDIATE + CAS"]
    end

    IL -->|"202 Accepted"| DB

    subgraph DB["SQLite WAL Storage Engine"]
        direction LR
        EP["endpoints"] --> IE["incoming_events"]
        IE --> DA["delivery_attempts"]
        DA --> DLQ["dead_letter_queue"]
    end

    DB -->|"Lease Loop"| DW

    subgraph DW["Dispatch Worker Engine"]
        direction LR
        JB["Decorrelated Jitter\nBackoff"] --> CB["Circuit Breaker\nCLOSED / OPEN / HALF-OPEN"]
        CB --> DQ["DLQ Eviction\n& Replay API"]
    end

    DW -->|"HTTP POST"| DS["Downstream Services"]

    OB["Observability\n/health · tracing"]
    IL -.-> OB
    DW -.-> OB
```

---

## Features

**Ingestion**
- Zero-copy raw body extraction via `axum::body::Bytes` preserves byte-exact payload for HMAC verification while parsing JSON in a single pass.

**Cryptographic Verification**
- Multi-provider signature adapters: GitHub (HMAC-SHA256), Stripe (v1 timestamped with 300s replay window), Midtrans (SHA-512), Discord (Ed25519), and Generic HMAC.
- Constant-time comparison via `hmac::Mac::verify_slice()` and `ed25519_dalek::verify_strict()` prevents timing attacks.

**Atomic Persistence**
- Embedded SQLite WAL mode with `PRAGMA synchronous = NORMAL`, `busy_timeout = 5000ms`, memory-mapped I/O (256 MB).
- Compare-And-Swap idempotency engine using SQLite transactions guarantees a replayed webhook is accepted once. Downstream delivery is **at-least-once**: if the process dies after the POST lands but before the attempt is recorded, the event is delivered again — consumers must stay idempotent.

**Resilient Dispatch**
- AWS-style decorrelated jitter backoff (`sleep = min(cap, random(base..prev*3))`) for retry scheduling.
- Per-endpoint circuit breaker state machine (CLOSED → OPEN → HALF-OPEN) with configurable thresholds.
- Forensic dead-letter queue with replay capability for poisoned event recovery.

**Observability**
- Structured JSON logging via `tracing-subscriber` with span-based request tracking.
- Health endpoint (`GET /health`) returning status, version, and server timestamp.
- Prometheus metrics at `GET /metrics` (enable via `[metrics] enabled = true`): ingest counts and latency histograms per endpoint/provider, dispatch attempt outcomes, SQLite write errors, HTTP request totals. Returns `404` when disabled.

---

## Performance Targets

| Metric | Target |
| :--- | :--- |
| Ingestion throughput | ≥ 10,000 req/s sustained |
| Ingestion p99 latency | ≤ 5 ms |
| SQLite write throughput | ≥ 25,000 tx/s |
| Memory (RSS) under load | ≤ 20 MB |
| Cold start to listening | ≤ 50 ms |
| Binary size (musl) | ≤ 10 MB |
| Docker image size | ≤ 20 MB |

---

## Quickstart

### Prerequisites
- Rust toolchain (stable, 2024 edition): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- Docker (for containerized deployment): any modern version supporting scratch images

### Build from Source
```bash
git clone https://github.com/Schnee111/aether-relay.git
cd aether-relay

# Run tests
cargo test

# Build release binary
cargo build --release --target x86_64-unknown-linux-musl

# Run locally
./target/x86_64-unknown-linux-musl/release/aether-relay
```

### Docker Deployment
The image runs unprivileged as uid `65534` and keeps its SQLite database in the
`/app/data` volume. The host directory you bind must be owned by that uid, or
the gateway cannot create its database:

```bash
docker build -t aether-relay:latest .
mkdir -p ./data && sudo chown -R 65534:65534 ./data
docker run -p 3000:3000 -v "$PWD/data:/app/data" aether-relay:latest
```

Without a bind mount the volume is anonymous, so the database is lost when the
container is removed.

### Configuration
Environment variable overrides take precedence over `config/default.toml`:
```bash
export RELAY__SERVER__HOST="0.0.0.0"
export RELAY__SERVER__PORT=3000
export RELAY__DATABASE__PATH="./aether-relay.db"
export RELAY__AUTH__API_KEYS="your-secret-key-here"
```

---

## API Reference

### Register Endpoint
```bash
curl -X POST http://localhost:3000/v1/endpoints \
  -H "Content-Type: application/json" \
  -H "X-Api-Key: your-secret-key-here" \
  -d '{
    "name": "GitHub Production",
    "provider": "github",
    "secret": "gh_webhook_secret",
    "target_url": "https://internal.example.com/hook"
  }'
```

Responds `201 Created` with the new endpoint id (when `auth.api_keys` is
non-empty, `X-Api-Key` is required):

```json
{ "id": "9f1c2a7e-...", "name": "GitHub Production", "provider": "github", "target_url": "https://internal.example.com/hook" }
```

`provider` must be one of `github`, `stripe`, `midtrans`, `discord`, `generic`;
an unknown value is rejected with `400`. `target_url` must be an `http`/`https`
URL pointing at a reachable host — loopback, link-local and unspecified
addresses are rejected to prevent the gateway being used as an SSRF probe.

### Ingest Webhook
```bash
curl -X POST http://localhost:3000/v1/ingest/{endpoint_id} \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: unique-request-id" \
  -H "X-Hub-Signature-256: sha256=<signature>" \
  -d '{"event":"push","ref":"refs/heads/main"}'
```

`Idempotency-Key` is **required**: omitting it returns `400 BAD_REQUEST`, not
`500`. Replaying the same key for the same endpoint returns the original event
id instead of creating a second delivery. `X-Hub-Signature-256` is required for
`github` endpoints and must be computed over the **raw request body**.

### View Dead-Letter Queue
```bash
curl http://localhost:3000/v1/dlq
```

### Replay Failed Event
```bash
curl -X POST http://localhost:3000/v1/dlq/{dlq_id}/replay \
  -H "X-Api-Key: your-secret-key-here"
```

Replay clears the event's prior delivery attempts, so the event returns to
`RECEIVED` with its full retry budget restored.

---

## Quality Gates

This project enforces strict engineering standards:
- **Clippy**: `cargo clippy -- -D warnings` must pass with zero warnings.
- **Format**: `cargo fmt --check` enforced in CI.
- **Testing**: All unit and integration tests must pass.
- **Security**: `cargo audit` scans dependencies for known vulnerabilities.
- **Reviews**: PRs reviewed via Shorekeeper Sentinel before merge.

---

## Contributing

See [SPEC.md](docs/rust/SPEC.md) for technical architecture details and [TEST_CRUCIBLE.md](docs/rust/TEST_CRUCIBLE.md) for the 20 mandatory verification scenarios.

All contributions follow the development workflow defined in [GATES.md](docs/rust/GATES.md) (Definition of Ready / Definition of Done) pipeline: DEFINE → PLAN → BUILD → VERIFY → POLISH → REVIEW → SHIP.

---

## License

MIT License — see LICENSE file for details.

---

Built by Schnee & Shorekeeper 🚀
