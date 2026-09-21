# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
