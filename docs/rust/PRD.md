# Product Requirements Document (PRD) — AetherRelay Rust

## 1. Executive Summary

AetherRelay is a high-performance webhook ingestion and reliable dispatch gateway rewritten in Rust. It replaces the TypeScript/Node.js prototype (v0.1.x) with a native binary that eliminates garbage collection pauses, reduces memory footprint by 10x, and targets 10,000+ sustained req/s on a single VPS core.

The system receives inbound webhooks from third-party providers, verifies cryptographic signatures in constant time, persists events atomically to an embedded SQLite WAL database, and dispatches them downstream with jittered exponential backoff and per-endpoint circuit breakers. Poisoned events are isolated into a forensic dead-letter queue with replay capability.

## 2. Motivation for Rust Rewrite

| Dimension | TypeScript v0.1.x | Rust Target |
| :--- | :--- | :--- |
| Throughput (req/s) | 2,260 (Autocannon) | 10,000+ |
| SQLite write tx/s | 18,140 (better-sqlite3) | 25,000+ (rusqlite direct) |
| Memory (RSS) | ~160 MB | < 20 MB |
| Binary size | ~180 MB (node_modules) | < 10 MB (musl static) |
| Startup time | ~800 ms | < 50 ms |
| GC pauses | V8 GC jitter | Zero (no GC) |
| Docker image | ~350 MB (node:22-slim) | < 20 MB (scratch) |
| Dependencies | 147 npm packages | ~30 crates (auditable) |

## 3. Target Persona & User Stories

- **Backend / Platform Engineer**: Needs a drop-in webhook buffer that "just works" as a single binary with zero runtime dependencies.
- **SRE / DevOps Engineer**: Wants sub-20MB Docker images, sub-50ms cold start, Prometheus metrics, and structured JSON logs for aggregation.
- **Autonomous AI Agent**: Requires deterministic API contracts, strict type safety, and auditable execution traces.

## 4. Scope Boundaries

### In-Scope
- HTTP ingestion endpoint with zero-copy raw body capture for HMAC verification.
- Multi-provider signature adapters: GitHub (HMAC-SHA256), Stripe (v1 timestamped), Midtrans (SHA-512), Discord (Ed25519), Generic HMAC.
- Embedded SQLite WAL storage via rusqlite with `BEGIN IMMEDIATE` atomic idempotency.
- Async dispatch worker with decorrelated jitter backoff (AWS formula) on tokio runtime.
- Per-endpoint circuit breaker state machine (CLOSED → OPEN → HALF-OPEN).
- Dead-letter queue with forensic error snapshots and atomic replay API.
- Endpoint registration CRUD API.
- Runtime configuration via TOML file + environment variable overrides.
- Gateway-level API key authentication.
- Prometheus metrics endpoint (`/metrics`) and structured JSON logging via `tracing`.
- Static musl binary and Docker scratch image (< 20 MB).
- GitHub Actions CI: `cargo clippy`, `cargo fmt`, `cargo test`, `cargo audit`.

### Out-of-Scope
- Complex event processing or stream analytics.
- Multi-node clustering or distributed consensus.
- WebSocket or gRPC ingestion (HTTP POST only).
- GUI or web dashboard.

## 5. API Contract

### 5.1 Ingest Webhook
```
POST /v1/ingest/:endpoint_id
```

**Request Headers:**
- `Content-Type: application/json`
- `Idempotency-Key: <unique-id>`
- `X-Api-Key: <gateway-api-key>` (gateway auth)
- Provider signature header (provider-specific)

**Responses:**

| Status | Meaning |
| :--- | :--- |
| `202 Accepted` | Event queued for dispatch |
| `401 Unauthorized` | Invalid API key or signature |
| `409 Conflict` | Duplicate idempotency key |
| `413 Payload Too Large` | Body exceeds 1 MB limit |
| `429 Too Many Requests` | Rate limit exceeded |

### 5.2 Endpoint Registration
```
POST   /v1/endpoints          — Register new endpoint
GET    /v1/endpoints          — List all endpoints
GET    /v1/endpoints/:id      — Get endpoint details
PATCH  /v1/endpoints/:id      — Update endpoint config
DELETE /v1/endpoints/:id      — Remove endpoint
```

### 5.3 Dead-Letter Queue
```
GET    /v1/dlq                — List dead-letter events (paginated)
GET    /v1/dlq/:id            — Get dead-letter event details
POST   /v1/dlq/:id/replay     — Re-enqueue for dispatch
DELETE /v1/dlq/:id            — Permanently discard
```

### 5.4 Observability
```
GET /health   → { "status": "ok", "db": true, "uptime_secs": N }
GET /metrics  → Prometheus text exposition format
```

## 6. Performance Targets

| Metric | Target | Measurement Method |
| :--- | :--- | :--- |
| Ingestion throughput | ≥ 10,000 req/s | Autocannon / wrk2, 10s sustained |
| Ingestion p99 latency | ≤ 5 ms | Histogram from `/metrics` |
| SQLite write throughput | ≥ 25,000 tx/s | Benchmark with `criterion` |
| Memory (RSS) under load | ≤ 20 MB | `/proc/self/status` VmRSS |
| Cold start to listening | ≤ 50 ms | Timestamp delta |
| Binary size (musl) | ≤ 10 MB | `ls -lh` on release build |
| Docker image size | ≤ 20 MB | `docker images` |

## 7. Quality Gates (Definition of Done)

- `cargo clippy -- -D warnings` passes with zero warnings.
- `cargo fmt --check` passes.
- `cargo test` — all unit, integration, and property-based tests green.
- `cargo audit` — zero known vulnerabilities.
- Crucible test scenarios (16+) all verified empirically.
- README with architecture diagram, quickstart, API reference, benchmarks.
- CHANGELOG.md following Keep a Changelog format.
- Tagged release with SemVer.

## 8. Milestones

| Phase | Deliverable | PR Target |
| :--- | :--- | :--- |
| Phase 1 | Project scaffold, config, logging, health endpoint | PR #1 |
| Phase 2 | SQLite WAL schema + rusqlite migrations + idempotency engine | PR #2 |
| Phase 3 | Crypto adapters (GitHub, Stripe, Midtrans, Discord, Generic) | PR #3 |
| Phase 4 | Axum ingestion routes + raw body extraction | PR #4 |
| Phase 5 | Dispatch worker + circuit breaker + DLQ | PR #5 |
| Phase 6 | Endpoint CRUD API + gateway auth | PR #6 |
| Phase 7 | Prometheus metrics + observability polish | PR #7 |
| Phase 8 | Hardening Crucible (16 scenarios) + benchmarks | PR #8 |
| Phase 9 | Docker scratch image + CI + README + release | PR #9 |
