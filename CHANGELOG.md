# Changelog

All notable changes to this project are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This project adheres to [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Fixed

**BUG 1 — CI workflow broken (`working-directory: soroban-qv`)**

- `.github/workflows/ci.yml`: removed `working-directory: soroban-qv` from all
  four jobs (`fmt`, `clippy`, `test`, `build-wasm`).  No such subdirectory
  exists — the workspace lives at the repo root.  Every CI job was failing
  immediately on checkout before executing any command.  Also removed the
  matching `workspaces: soroban-qv` key from each `Swatinem/rust-cache` step
  and corrected the WASM artifact upload path from `soroban-qv/target/...` to
  `target/...`.

**BUG 2 — `add_option_votes` panicked instead of returning a typed error**

- `contracts/qv-core/src/errors.rs`: added `OptionTotalOverflow = 12` variant.
- `contracts/qv-core/src/storage.rs`: changed `add_option_votes` return type
  from `()` to `Result<(), QVError>`.  Replaced `.expect("option vote total
  overflow")` with `.ok_or(QVError::OptionTotalOverflow)?`.
- `contracts/qv-core/src/lib.rs`: propagated the error in `cast_vote` with `?`.
- Added `set_option_total_votes_for_test` helper (`#[cfg(test)]` only) so tests
  can position the option aggregate counter without calling `cast_vote` billions
  of times.
- New test: `test_option_total_overflow_returns_typed_error`.

### Added

**IMPROVEMENT 1 — `MAX_OPTIONS` cap on `create_round`**

- `contracts/qv-core/src/lib.rs`: added `pub const MAX_OPTIONS: u32 = 50` with
  an inline comment explaining the CPU-budget rationale (at most 1 225 inner
  loop iterations).
- `contracts/qv-core/src/errors.rs`: added `TooManyOptions = 13` variant.
- `create_round` now returns `TooManyOptions` before the O(n²) duplicate check
  if `option_ids.len() > MAX_OPTIONS`.
- New tests: `test_create_round_at_max_options_succeeds` (boundary — exactly 50),
  `test_create_round_over_max_options_rejected` (51 options → `TooManyOptions`).
- Documented in `README.md` (`create_round` parameter table) and `ARCHITECTURE.md`
  (new "Resource exhaustion: MAX_OPTIONS cap" subsection in the overflow section).

**IMPROVEMENT 2 — `get_voter_option_votes` public getter**

- `contracts/qv-core/src/lib.rs`: exposed the existing internal
  `storage::get_voter_option_votes` as a public contract method
  `get_voter_option_votes(env, round_id, voter, option_id) → Result<u32, QVError>`.
  Returns `0` for voters who have not yet cast any votes on that option.
  Returns `RoundNotFound` for unknown rounds (consistent with other getters).
- New tests: `test_get_voter_option_votes_before_any_vote`,
  `test_get_voter_option_votes_accumulates_correctly`,
  `test_get_voter_option_votes_round_not_found`.
- Documented in `README.md` Contract Interface section.

### Changed (internal, no public ABI change)

**IMPROVEMENT 3 — storage.rs internal naming and documentation**

- `storage::has_voted_on_option` renamed to `storage::is_voter_initialised`.
  The old name incorrectly implied per-option state; the function actually checks
  whether a voter's credit ledger has been initialised for a round at all.  All
  call sites in `lib.rs` updated.  This is a private storage function — no
  public contract ABI change.
- `set_voter_option_votes` doc comment rewritten to accurately describe the
  two-counter pattern: this function updates the per-(voter, option) record;
  `add_option_votes` separately maintains the option aggregate; both must be
  called together (as `cast_vote` does).  Removed the old inaccurate "scan
  per-option" description.
- `ARCHITECTURE.md` overflow table: corrected the "option total overflow"
  row from "panic (unreachable in practice)" to "checked_add →
  `OptionTotalOverflow`".

**IMPROVEMENT 4 — naming consistency**

- `contracts/qv-core/Cargo.toml`: corrected `repository` field from
  `https://github.com/your-org/soroban-qv` to
  `https://github.com/joshuaodoh122-hub/soroban-qv-core`.
- `README.md`: CI badge URL updated to `joshuaodoh122-hub/soroban-qv-core`; 
  `cd soroban-qv` build snippet removed (commands run at repo root); project
  structure tree renamed from `soroban-qv/` to `soroban-qv-core/`.
- `DEPLOY.md`: removed stale `cd soroban-qv` build step.
- `CONTRIBUTING.md`: updated clone URL and `cd` target to
  `https://github.com/joshuaodoh122-hub/soroban-qv-core` /
  `cd soroban-qv-core`.

**Dependency pinning**

- `Cargo.toml` workspace: tightened `soroban-sdk` to `"=22.0.7"` (was `"22.0.7"`)
  and used `cargo update ed25519-dalek@3.0.0 --precise 2.2.0` to prevent
  `soroban-env-host 22.1.3` from pulling `ed25519-dalek 3.0.0`, which has an
  incompatible `SigningKey::generate` signature in testutils.

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
