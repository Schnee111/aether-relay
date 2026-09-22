# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-09-22

### Added
- **Rewrite**: Complete native Rust implementation replacing the earlier prototype, powered by Axum 0.8 and Tokio 1.x.
- **Storage & Deduplication**: Embedded SQLite WAL storage with r2d2 connection pool and atomic compare-and-swap idempotency filtering on arrival.
- **Crypto Adapters**: Constant-time signature verification for GitHub (HMAC-SHA256), Stripe (v1 timestamped with 300s replay window), Midtrans (SHA-512), Discord (Ed25519 verify_strict), and Generic HMAC.
- **Dispatch Engine**: Asynchronous delivery worker with AWS decorrelated jitter exponential backoff, per-endpoint circuit breaker (Closed/Open/Half-Open), and Dead-Letter Queue (DLQ) management with replay endpoint.
- **Observability**: Structured JSON logging via `tracing-subscriber`, `/health` endpoint, and Prometheus metrics exporter on `/metrics` exposing ingest latency, counters, and dispatch state.
- **Deployment**: Multi-stage Docker build producing a static musl binary (~10.6 MB) inside a scratch container (~9.7 MB) running as unprivileged user (uid 65534).
- **Security**: Strict SSRF prevention on endpoint target URLs (rejecting private, link-local, and loopback CIDRs) and fail-closed validation for required headers.

### Removed
- Removed legacy prototype files, build artifacts, and stale workspace configurations.

---

## [0.1.1] - 2026-09-22

### Fixed
- Normalize millisecond Stripe timestamps to epoch seconds to prevent false-positive replay rejections under server clock skew.

---

## [0.1.0] - 2026-09-22

### Added
- Initial prototype implementation with basic webhook ingestion, SQLite storage, and retry dispatch.
