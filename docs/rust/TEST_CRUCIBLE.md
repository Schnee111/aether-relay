# The Hardening Crucible — AetherRelay Rust

16 mandatory verification scenarios + 4 Rust-specific bonus scenarios. Every scenario must be verified empirically with evidence before the project can be considered complete.

---

## Phase 1: Governance & Hygiene (Scenarios 1-3)

### Scenario 01: Cargo Clippy Gauntlet
- Run `cargo clippy -- -D warnings` on the entire workspace.
- Introduce 5 deliberate lint violations (unused variable, needless borrow, `unwrap()` in library code, missing docs on public API, `as` cast on lossy integer conversion).
- Verify all 5 are caught and rejected.
- **Evidence**: Terminal output showing 5 errors and zero-warning clean build after fix.

### Scenario 02: Formatting Gate
- Run `cargo fmt --check` on deliberately mis-formatted code (wrong indentation, trailing whitespace, missing newline at EOF).
- Verify CI rejects the diff.
- **Evidence**: Non-zero exit code from `cargo fmt --check`.

### Scenario 03: Multi-Issue DoR Gate
- Create Issue A: vague, no acceptance criteria → rejected by DoR gate, labeled `status:blocked`.
- Create Issue B: structured with binary DoD, MRE, references → accepted, labeled `status:ready-for-dev`.
- **Evidence**: GitHub issue comments showing rejection rationale and acceptance.

---

## Phase 2: Cryptographic Integrity (Scenarios 4-8)

### Scenario 04: GitHub HMAC-SHA256 Byte-Exact
- Send valid GitHub webhook payload with `X-Hub-Signature-256` header.
- Send same payload with 1-bit corrupted signature.
- Verify: valid → 202, corrupted → 401.
- **Evidence**: Two curl commands with responses.

### Scenario 05: Stripe v1 Timestamped + Replay Window
- Send valid Stripe payload with current timestamp in `Stripe-Signature`.
- Send same payload with timestamp 400 seconds in the past (exceeds 300s tolerance).
- Verify: current → 202, expired → 401 (replay detected).
- **Evidence**: Two curl commands, second rejected with replay reason.

### Scenario 06: Midtrans SHA-512
- Send valid Midtrans notification with SHA-512 signature.
- Verify: 202 Accepted.
- **Evidence**: curl command with response.

### Scenario 07: Discord Ed25519
- Generate Ed25519 keypair, configure endpoint with public key.
- Send interaction payload signed with private key, including `X-Signature-Ed25519` and `X-Signature-Timestamp`.
- Verify: valid signature → 202, tampered body → 401.
- **Evidence**: Two curl commands.

### Scenario 08: Constant-Time Verification
- Verify that `hmac::Mac::verify_slice()` (RustCrypto) and `ed25519_dalek::verify_strict()` are used instead of byte-by-byte comparison.
- Code audit: grep for `==` on signature bytes — must find zero instances.
- **Evidence**: grep output and code reference.

---

## Phase 3: Concurrency & Durability (Scenarios 9-12)

### Scenario 09: 50-Request Concurrent Race Guard
- Send 50 simultaneous POST requests with identical `Idempotency-Key`.
- Verify: exactly 1 returns 202, exactly 49 return 409.
- Verify: database contains exactly 1 row for that key.
- **Evidence**: Script output showing status code distribution and row count.

### Scenario 10: SQLite BEGIN IMMEDIATE Under Contention
- Spawn 10 tokio tasks each performing 100 idempotent inserts with unique keys.
- Verify: all 1000 events persisted, zero `SQLITE_BUSY` panics, all handled via `busy_timeout`.
- **Evidence**: Test output showing 1000 rows and zero errors.

### Scenario 11: Crash Durability (Kill -9 Drill)
- Start server, begin inserting 2000 events at high rate.
- Send SIGKILL mid-way through.
- Restart, run `PRAGMA integrity_check` and `PRAGMA quick_check`.
- Verify: both return "ok", no data corruption, committed events recoverable.
- **Evidence**: Terminal output showing kill, restart, integrity check results, and recovered row count.

### Scenario 12: WAL Checkpoint Stability
- Insert 10,000 events to grow WAL file.
- Trigger manual `PRAGMA wal_checkpoint(TRUNCATE)`.
- Verify: WAL file truncated, database file intact, subsequent reads/writes succeed.
- **Evidence**: File sizes before/after checkpoint and successful query.

---

## Phase 4: Dispatch Resilience (Scenarios 13-16)

### Scenario 13: Decorrelated Jitter Backoff
- Configure downstream mock to return 500 for first 3 attempts, then 200.
- Verify: event transitions RECEIVED → PROCESSING → (3 failures with increasing jitter delays) → DELIVERED.
- Verify: delay values follow decorrelated jitter formula (not constant, not purely exponential).
- **Evidence**: Log entries showing retry timestamps and computed sleep durations.

### Scenario 14: Circuit Breaker Trip and Recovery
- Configure downstream mock to return 500 consistently.
- Verify: after 5 consecutive failures, circuit opens and subsequent dispatches fail-fast without HTTP call.
- After recovery timeout, verify circuit transitions to HALF-OPEN and allows trial request.
- On success, verify circuit transitions back to CLOSED.
- **Evidence**: Log entries showing state transitions with timestamps.

### Scenario 15: DLQ Eviction and Replay
- Trigger event that exceeds max_attempts → verify eviction to `dead_letter_queue` with error snapshot.
- Call `POST /v1/dlq/:id/replay` → verify event re-enqueued to `incoming_events` with status RECEIVED.
- **Evidence**: Database state before and after replay.

### Scenario 16: Emergency P0 Hotfix SemVer
- Simulate production bug: create issue, branch `hotfix/...`, fix, test, PR, full merge, version bump.
- Verify: SemVer patch increment, CHANGELOG updated, GitHub release created.
- **Evidence**: Git log, PR merge commit, release tag.

---

## Phase 5: Rust-Specific Bonus (Scenarios 17-20)

### Scenario 17: Memory Soak Test
- Run Autocannon 60-second sustained load at 10k req/s target.
- Monitor RSS via `/proc/self/status` every second.
- Verify: RSS stays ≤ 20 MB, no monotonic growth (no leak).
- **Evidence**: RSS time series and final value.

### Scenario 18: Binary Size Verification
- Build release with `--target x86_64-unknown-linux-musl`.
- Verify: binary ≤ 10 MB, Docker image ≤ 20 MB.
- **Evidence**: `ls -lh` and `docker images` output.

### Scenario 19: Cold Start Benchmark
- Measure time from process start to first successful `/health` response.
- Verify: ≤ 50 ms.
- **Evidence**: Timestamp delta measurement.

### Scenario 20: cargo audit Clean
- Run `cargo audit` on the locked dependency tree.
- Verify: zero known vulnerabilities.
- **Evidence**: Terminal output.
