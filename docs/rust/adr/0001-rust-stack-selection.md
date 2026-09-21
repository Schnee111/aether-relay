# ADR-0001: Rust Stack Selection for AetherRelay Rewrite

**Status:** Accepted  
**Date:** 2026-09-22  
**Deciders:** Schnee, Shorekeeper  

## Context

AetherRelay v0.1.x was implemented in TypeScript/Node.js as a workflow engineering test bed. The prototype validated the architectural design (Fastify + SQLite WAL + idempotency CAS) and proved all 16 Hardening Crucible scenarios. However, the claim "High-Performance" is not credible with a Node.js runtime achieving 2.2k req/s and 160 MB RSS.

A rewrite in a systems language is needed to make the performance claim genuine while reducing operational footprint (binary size, memory, startup time, Docker image size).

## Decision Drivers

1. **Credibility**: "High-Performance" requires evidence that a GC-less, native runtime delivers measurably superior throughput and latency.
2. **Ecosystem maturity**: HTTP server, SQLite binding, crypto libraries must be production-proven, not experimental.
3. **Operational simplicity**: Single static binary, zero runtime deps, scratch Docker image.
4. **Portfolio signal**: Demonstrates systems-level engineering capability.

## Considered Options

### Option A: Rust (Axum + rusqlite + RustCrypto)
- **Pros**: Zero GC, predictable latency, mature async runtime (tokio), Axum is the most popular Rust web framework (12.4M crates.io downloads), rusqlite directly wraps SQLite C library with zero overhead, RustCrypto provides constant-time HMAC `verify_slice()`, musl static binary < 10 MB, Docker scratch < 20 MB.
- **Cons**: Longer compile times, steeper learning curve (irrelevant — Shorekeeper writes code).

### Option B: Go (net/http + mattn/go-sqlite3)
- **Pros**: Fast compile, simple deployment, goroutine model.
- **Cons**: GC pauses under sustained load, mattn/go-sqlite3 uses CGO (complicates cross-compilation and static linking), less impressive as portfolio piece compared to Rust.

### Option C: Zig
- **Pros**: Minimal binary, manual memory control, C interop.
- **Cons**: HTTP server ecosystem immature (std.http, zap, httpz not battle-tested), no ORM or query builder, tiny community, high risk of reinventing solved problems.

## Decision

**Rust (Option A)** — Axum + tokio + rusqlite + RustCrypto + ed25519-dalek.

## Specific Stack Choices

### HTTP Framework: Axum over Actix-Web

| Factor | Axum | Actix-Web |
| :--- | :--- | :--- |
| Middleware model | Tower (cross-framework, reusable) | Framework-specific |
| Raw body extraction | `axum::body::Bytes` + `to_bytes()` with limit | Manual `Payload` stream assembly |
| Extractor ergonomics | `FromRequest` derive macro, compile-time body-taken checks | Macro-based handler (12 param limit) |
| Ecosystem | Reuse Tower layers across hyper, tonic, etc. | Locked to Actix ecosystem |
| Actor coupling | None | Legacy actor system overhead in 62% of codebases |
| Community momentum | 12.4M downloads, 7-day avg issue resolution | 8.7M downloads, 14-day avg |

### Database: rusqlite over sqlx/diesel

- **rusqlite**: Synchronous C binding, lowest possible overhead for single-writer SQLite. Wrapped in `tokio::task::spawn_blocking` for async compatibility. WAL + NORMAL mode benchmarked at 25k inserts/s in Rust.
- **sqlx**: Async with compile-time query checking — elegant but adds unnecessary async wrapper overhead for SQLite's inherently synchronous single-writer model. Better suited for PostgreSQL.
- **diesel**: Heavy ORM, schema macro DSL, overkill for 4 tables.

### Crypto: RustCrypto (hmac + sha2) over ring

- **RustCrypto**: Pure Rust, `hmac::Mac::verify_slice()` is constant-time, no system OpenSSL dependency, simpler API.
- **ring**: Also constant-time, but C/ASM core complicates musl cross-compilation and `cargo audit` visibility.
- **ed25519-dalek**: Standard for Discord Ed25519 verification, `verify_strict()` prevents signature malleability attacks.

## Consequences

- Compile times will be 30-60s for incremental, 2-5 min for clean release.
- All SQLite operations must go through `spawn_blocking` to avoid blocking the tokio event loop.
- No ORM — raw SQL with `rusqlite::params![]` macro. Schema migrations are embedded SQL strings.
- Binary size target < 10 MB requires `opt-level = "z"`, `lto = true`, `strip = true` in release profile.

## Validation

This decision will be validated by achieving:
- ≥ 10,000 req/s sustained (Autocannon/wrk2)
- ≤ 20 MB RSS under load
- ≤ 10 MB static binary
- All 16+ Hardening Crucible scenarios passing
