# Quality Gates — AetherRelay Rust

## Definition of Ready (DoR)

An issue/task MUST satisfy ALL of the following before work begins:

1. **Problem Clarity & Boundary**: Clear in-scope vs out-of-scope definition. No ambiguous requirements.
2. **Binary Acceptance Criteria**: Each criterion is objectively verifiable (pass/fail, measurable threshold, or exact output).
3. **Contract & Signature Specification**: API endpoints define request/response schema. Crypto operations define algorithm and header names.
4. **Architecture Reference**: Links to relevant SPEC.md section or ADR.
5. **Size Constraint**: Estimated LOC change ≤ 400 (hard ceiling). If larger, must be decomposed into sub-issues.

## Definition of Done (DoD)

A PR MUST satisfy ALL of the following before merge:

### Code Quality
- [ ] `cargo clippy -- -D warnings` passes with zero warnings.
- [ ] `cargo fmt --check` passes.
- [ ] No `unwrap()` or `expect()` in library code (only in tests and `main.rs` bootstrap).
- [ ] All public types and functions have doc comments.
- [ ] No `unsafe` blocks unless justified in an inline comment with safety invariant.

### Testing
- [ ] All unit tests pass: `cargo test`.
- [ ] Integration tests cover the happy path and at least one error path.
- [ ] Property-based tests (proptest) for invariants where applicable.
- [ ] No flaky tests — all assertions are deterministic.

### Security
- [ ] `cargo audit` reports zero known vulnerabilities.
- [ ] No hardcoded secrets or credentials in source code.
- [ ] Crypto operations use constant-time comparison (no `==` on signatures).
- [ ] SSRF prevention: outbound HTTP dispatch blocks private IP ranges.

### Performance
- [ ] No blocking operations on the tokio event loop (SQLite via `spawn_blocking`).
- [ ] Request body size limited to 1 MB.
- [ ] No unbounded allocations in request handlers.

### Documentation
- [ ] CHANGELOG.md updated (Keep a Changelog format).
- [ ] README updated if public API or behavior changes.
- [ ] ADR written for non-trivial architectural decisions.

### Git & Review
- [ ] Commits follow Conventional Commits 1.0.0 with body explaining why.
- [ ] PR description includes: Context, Problem, Solution, Verification Evidence, Risk Assessment, DoD Checklist.
- [ ] PR size ≤ 400 LOC (hard ceiling), sweet spot < 300 LOC.
- [ ] At least one substantive review (Sentinel or manual) before merge.
- [ ] Merge via `--merge` (no squash, no rebase merge).

## Commit Message Standard

```
<type>(<scope>): <subject (imperative, lowercase, ≤72 chars)>

<body: why this change, what it does, side effects (wrap at 72 cols)>

<trailers: Refs #N, Fixes #N, Co-authored-by:>
```

**Types**: `feat`, `fix`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `docs`, `style`, `revert`

## PR Review Protocol

Reviews use Conventional Comments format:
- `issue [blocking]`: Must be fixed before merge.
- `suggestion [non-blocking]`: Recommended improvement, author decides.
- `question`: Requires author response (may block if answer reveals issue).
- `nitpick [non-blocking]`: Style/naming preference.

Review verdicts:
- **Approved**: All blocking issues resolved, code is merge-ready.
- **Changes Requested**: Blocking issues found, author must remediate and re-request review.

Author responses:
- Fix the issue (commit hash reference), OR
- Present empirical evidence for pushback (benchmark data, documentation citation).
- Never respond with sycophantic agreement without evidence.

## Release Protocol

1. All Crucible scenarios verified with empirical evidence.
2. `cargo audit` clean.
3. Version bump in `Cargo.toml` following SemVer.
4. CHANGELOG.md updated.
5. Git tag `vX.Y.Z` created and pushed.
6. GitHub Release created with changelog as release notes.
7. Docker image built and tagged.
