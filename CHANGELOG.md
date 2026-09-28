# Changelog

All notable changes to this project are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This project adheres to [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

_(Nothing pending yet — contributions go here before a release.)_

---

## [0.1.0] — 2026-09-28

### Added

**Core contract (`contracts/qv-core`)**

- `create_round(admin, option_ids, credits_per_voter)` — creates a voting round
  with configurable option IDs and fixed-allocation voice credits. Returns a
  monotonically increasing `round_id`.
- `cast_vote(voter, round_id, option_id, num_votes)` — quadratic cost voting
  with accumulation: repeated calls on the same option accumulate, and the
  marginal cost is `new_total² − old_total²`. Explicit overflow checks on all
  arithmetic.
- `close_round(admin, round_id)` — closes a round; only the round's own admin
  can close it. No global admin.
- `get_results(round_id)` — returns raw vote counts (not credits spent) per
  option. O(options) via stored running totals.
- `get_round_info(round_id)` — returns round metadata.
- `get_voter_credits(round_id, voter)` — returns remaining voice credits,
  returning the full allocation for voters who have not yet cast any votes.
- `QVError` enum with 11 distinct error codes — all arithmetic, auth, and state
  errors are distinct and explicit.
- `CreditSource` trait stub in `credit_source.rs` — interface for pluggable
  voice-credit sourcing strategies (v2 roadmap).

**Example (`examples/grant-vote`)**

- `GrantVoteContract` wrapping the core contract for a community grant proposal
  voting scenario with 3 named proposals.
- `winning_proposal()` helper that returns the option with the most votes.

**Tests**

- 20 tests covering: quadratic cost math (1, 2, 3, 5, 10 votes), accumulation
  with no-discount guarantee, insufficient credits, exact budget exhaustion,
  multi-option voting, accumulation marginal cost, round-close rejection, double
  close, non-admin close rejection, creation validation (empty options, zero
  credits, duplicates), invalid option, non-existent round, full tally
  correctness, full end-to-end grant round integration test, credit query before
  first vote, and round ID monotonicity.

**Infrastructure**

- Cargo workspace with `overflow-checks = true` in the release profile.
- `.github/workflows/ci.yml`: fmt, clippy (native + wasm32v1-none -D warnings),
  test (all-features), build-wasm (release). WASM artifacts uploaded.
- `rustfmt.toml`, `clippy.toml`.

**Documentation**

- `README.md`: plain-English QV introduction, cost table with examples,
  contract interface reference, quick start, known limitations (sybil resistance,
  fixed allocation, no delegation), roadmap.
- `ARCHITECTURE.md`: overflow analysis, accumulation rationale, round lifecycle
  state machine, auth model, storage key design.
- `CONTRIBUTING.md`, `SECURITY.md`, `LICENSE` (MIT), `DEPLOY.md`,
  `scripts/demo.sh`.

### Design decisions recorded

- **Accumulation:** repeated `cast_vote` calls on the same option use marginal
  quadratic cost, not per-call quadratic cost. Prevents gaming by call splitting.
- **Fixed allocation default:** token-weighted sourcing stubbed as `CreditSource`
  trait, planned for v0.2.
- **Per-round admin:** the `create_round` caller is the admin; no global admin.
- **Overflow:** `u32` votes, `u64` credits and squared values. `checked_*` on
  all critical paths.
