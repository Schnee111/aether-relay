# Hardening Crucible — Wave 1 Report (Scenarios 11–14)

**Date:** 2026-09-22
**Binary:** `target/release/aether-relay` @ main `08af4d8` (post #24/#26)
**Driver:** `scripts/crucible_driver.py` (executed verbatim, all evidence reproducible)

## Scenario 11 — Crash durability (kill -9) — PASS

10 events ingested with valid GitHub HMAC-SHA256 signatures, gateway killed with
`kill -9` mid-flight, then restarted against the same SQLite file.

- Pre-kill accepted events surviving restart: **10/10**
- `PRAGMA integrity_check` after restart: **ok**
- Restart cold-start: healthy, no crash loop

## Scenario 12 — WAL checkpoint stability — PASS

Continuous ingestion for 30s (15,290 events total). WAL size sampled at 10s/20s/30s:

| Sample | WAL size |
|---|---|
| 10s | 4,890,472 B |
| 20s | 4,890,472 B |
| 30s | 4,890,472 B |

WAL plateaus under the auto-checkpoint threshold: bounded growth confirmed
(no unbounded accumulation under sustained write load).

## Scenario 13 — Concurrent race guard (idempotency) — PASS

- 50 concurrent POSTs, ONE shared idempotency key:
  **1× 202 Accepted, 49× 409 Conflict, exactly 1 row in `incoming_events`**
- 50 concurrent POSTs, 50 distinct keys: **50/50 202**, 50 unique UUIDv7 ids,
  total 51 rows (1 shared + 50 distinct) verified

## Scenario 14 — Busy contention (SQLITE_BUSY) — PASS

pool_size=4, busy_timeout=5000ms, 100 concurrent requests across both bursts:

- `SQLITE_BUSY` surfaced to clients: **0**
- Internal database errors surfaced: **0**

## Verdict table

| Scenario | Verdict | Key number |
|---|---|---|
| 11 Crash durability | PASS | 10/10 survive, integrity ok |
| 12 WAL checkpoint | PASS | WAL flat at 4.78MB across 30s |
| 13 Race guard | PASS | 1 row from 50 racing duplicates |
| 14 Busy contention | PASS | 0 SQLITE_BUSY in 100 concurrent |

## Deferred to Wave 3 (metrics-affected, not run here)

- Memory soak 60s, cold start <50ms, binary size <10MB, throughput/p99,
  cargo audit — these must be measured on top of the Phase 7 metrics module
  so numbers are not invalidated by the middleware overhead.
