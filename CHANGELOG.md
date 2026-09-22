# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-09-22

### Added
- **scaffold**: initialize Rust workspace with Axum 0.8, tokio 1.x, rusqlite, tracing-subscriber, and config-rs (#4).
- **db**: embedded SQLite WAL migrations for endpoints, incoming_events, delivery_attempts, and dead_letter_queue via rusqlite r2d2 connection pool (#4).
- **core**: atomic CAS idempotency engine using SQLite `BEGIN IMMEDIATE` transactions guaranteeing exactly-once delivery (#4).
- **crypto**: multi-provider signature verification adapters: GitHub (HMAC-SHA256), Stripe (v1 timestamped + replay window), Midtrans (SHA-512), Discord (Ed25519 verify_strict), and Generic HMAC (#4).
- **crypto**: constant-time comparison enforcement across all adapters using `hmac::Mac::verify_slice()` and `ed25519_dalek::verify_strict()`.
- **api**: ingestion route `POST /v1/ingest/:endpoint_id` with zero-copy raw body extraction via axum::body::Bytes and optional API key gateway authentication (#4).
- **worker**: async dispatch worker with HTTP POST delivery via reqwest rustls client, per-endpoint circuit breaker (CLOSED/OPEN/HALF-OPEN), and AWS decorrelated jitter backoff scheduling (#4).
- **api**: DLQ management routes (`GET /v1/dlq`, `POST /v1/dlq/:id/replay`) for forensic inspection and atomic event re-enqueue (#4).
- **api**: endpoint CRUD API (`POST/GET/DELETE /v1/endpoints`) for downstream webhook target registration (#4).
- **docker**: multi-stage build pipeline producing static musl binary (< 10 MB) and scratch Docker image (< 20 MB total) (#4).
- **ci**: GitHub Actions workflow enforcing cargo fmt, clippy --warnings-as-errors, test suite, cargo audit, and musl cross-compilation (#4).
- **observability**: structured JSON logging via tracing-subscriber, health endpoint (`GET /health`), and Prometheus metrics routing (`GET /metrics`) (#4).

### Changed
- **rewrite**: full migration from TypeScript/Fastify prototype (v0.1.x) to native Rust/Axum implementation targeting 10× throughput improvement (#4).

### Fixed
- **security**: eliminated all timing attacks across cryptographic adapters by enforcing constant-time comparison exclusively.

---

## [0.1.1] - 2026-09-22

### Fixed
- **crypto**: normalize 13-digit millisecond Stripe timestamps to 10-digit epoch seconds to avoid false-positive replay rejections under server clock skew (#8).

---

## [0.1.0] - 2026-09-22

### Added
- **init**: bootstrap project foundation, architecture specs, PRD, test crucible, and quality gates contracts.
- **db**: implement SQLite WAL mode storage engine with Kysely migrations for `endpoints`, `incoming_events`, `delivery_attempts`, and `dead_letter_queue` (#3).
- **core**: implement atomic CAS idempotency state machine using SQLite `BEGIN IMMEDIATE` transactions to prevent duplicate webhook arrivals (#2, #3).
- **api**: stream-optimized Fastify ingestion gateway with zero-copy raw body preservation for cryptographic checks (#4).
- **crypto**: multi-provider webhook signature verification adapters supporting GitHub (HMAC-SHA256), Stripe (v1 timestamped), Midtrans (SHA-512), and generic HMAC (#4).
- **crypto**: constant-time signature comparison using `crypto.timingSafeEqual` wrapped in 32-byte SHA-256 digests to eliminate timing attacks and length discrepancy vulnerabilities.
- **worker**: dispatch loop with AWS-standard Decorrelated Jitter Exponential Backoff formula and downstream circuit breaker state machine (#4).
- **api**: dead-letter queue (DLQ) forensic inspection and atomic replay API (`POST /v1/dlq/:id/replay`).
- **observability**: Prometheus metrics exporter route (`/metrics`) and health check route (`/health`).
- **test**: 14 unit, integration, and chaos test cases covering schema integrity, concurrency floods, downstream failure cascades, and crash durability drills.
