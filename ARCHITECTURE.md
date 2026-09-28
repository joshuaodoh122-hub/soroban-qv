# Architecture

This document explains the design rationale behind `soroban-qv-core` — why
things are the way they are, and what alternatives were considered.

---

## Quadratic cost computation

### Why N²?

Quadratic voting's defining property is that the *marginal cost* of each
additional vote on an option increases linearly. The first vote costs 1 credit,
the second costs 3 more (total 4), the third costs 5 more (total 9), and so on.
This makes expressing very strong preferences expensive, which prevents runaway
concentration of influence.

The formula is simply: **cost(N votes) = N²**.

### How it's computed in the contract

```rust
let new_sq: u64 = (new_total as u64).checked_mul(new_total as u64)?;
let old_sq: u64 = (prev_votes as u64).checked_mul(prev_votes as u64)?;
let marginal_cost: u64 = new_sq - old_sq;
```

Votes are stored as `u32` (max ~4.29 billion votes per voter per option).
The squaring is done in `u64` to avoid overflow during multiplication.

**Why u64 is sufficient:** `u32::MAX² = 4_294_967_295² ≈ 1.844×10¹⁹`, and
`u64::MAX ≈ 1.844×10¹⁹`. They are close but `u32::MAX²` fits in a `u64`
(the precise value is `18_446_744_065_119_617_025`, just below
`u64::MAX = 18_446_744_073_709_551_615`). We use `checked_mul` regardless to
be explicit and future-safe. The subtraction `new_sq - old_sq` cannot underflow
because `new_total ≥ prev_votes` always.

**Why not `u128` everywhere?** Soroban storage and XDR encoding have overhead
proportional to value size. Keeping `u32` for vote counts and `u64` for credits
is a deliberate size/cost optimisation. If you need higher precision, you can
change `u32 → u64` and recompute the safety analysis.

### Overflow protection summary

| Overflow scenario                         | How handled                         |
|------------------------------------------|-------------------------------------|
| `prev_votes + num_votes` overflows `u32` | `checked_add` → `VoteCountOverflow` |
| `new_total²` overflows `u64`             | `checked_mul` → `CostOverflow`      |
| Option total votes overflows `u32`       | `checked_add` → panic (unreachable in practice) |

The last case panics rather than returning an error because reaching `u32::MAX`
total votes on a single option would require 4+ billion votes, which is
economically impossible given the quadratic cost structure.

---

## Accumulation behaviour

When a voter calls `cast_vote` multiple times on the same option, votes
**accumulate**. The cost charged is the *marginal increment*:

```
prev_votes = existing votes by this voter on this option
new_total  = prev_votes + num_votes
cost       = new_total² − prev_votes²
```

This means two calls of 3 and 2 votes cost the same as one call of 5 votes
(both cost 25 credits total). There is no way to obtain a discount by splitting
calls. This is the economically correct QV behavior and matches how QV is
described in the academic literature (Lalley & Weyl 2018).

**Why not reset per call?** If each call computed `cost = num_votes²`
independently, a voter could cast 1 vote 100 times on the same option, paying
1 credit per call (100 credits total) instead of 10,000 credits for 100 votes
in one call. This would completely break the quadratic mechanism.

---

## Round lifecycle state machine

```
             create_round()
                  │
                  ▼
              ┌───────┐
              │  Open  │◄──── cast_vote() allowed
              └───────┘
                  │
            close_round()
                  │
                  ▼
             ┌────────┐
             │ Closed │◄──── get_results() allowed
             └────────┘       cast_vote() rejected (RoundNotOpen)
```

There are only two states. A closed round cannot be reopened (by design — this
prevents retroactive vote manipulation).

---

## Auth model for round creation and closing

**Decision made:** The address that calls `create_round` becomes the admin of
that round and is the only address that can call `close_round` on it. There is
no global admin.

**Why no global admin?** A global admin would make this contract a single point
of failure and would require integrating projects to trust the deployer of this
boilerplate. The per-round admin model means each integrating DAO can use its
own governance multisig as the admin without any coordination with the boilerplate
deployer.

**Alternative considered:** A two-stage setup (deploy → set admin → create rounds).
Rejected because it adds complexity without benefit for a composable boilerplate.

**Require-auth placement:** `admin.require_auth()` is called at the start of
`create_round` and `close_round`. This uses Soroban's native auth framework,
which supports both single-key and multi-auth scenarios transparently.

---

## Voice credit sourcing

**Decision made:** Fixed allocation — every voter gets `credits_per_voter` credits
(set at round creation) on their first vote in that round.

**Why fixed?** It is the simplest default that makes the contract self-contained
with no external dependencies. It is correct and useful for many governance
scenarios (e.g., equal-stake DAOs, community votes).

**Token-weighted sourcing** (where credits are proportional to a token balance at
a ledger-sequence snapshot) is the most commonly requested alternative. It is
stubbed in `credit_source.rs` as the `CreditSource` trait and documented as a
planned v0.2 feature. It requires a cross-contract call to a SAC token and a
snapshot mechanism, which adds meaningful complexity and an external dependency
that is inappropriate for a minimal boilerplate.

---

## Storage key design

All keys are `#[contracttype]` enum variants, which serializes them as XDR
discriminants — stable, versioned, and readable by SDK tooling.

```rust
enum DataKey {
    NextRoundId,
    Round(u64),
    VoterInitialised(u64, Address),
    VoterCredits(u64, Address),
    VoterOptionVotes(u64, Address, u32),
    OptionTotalVotes(u64, u32),
}
```

`VoterInitialised` is a sentinel that distinguishes "voter has never voted in this
round" (full credit allocation should be returned) from "voter has voted and has
zero credits left" (return 0). Without this sentinel, both cases would look
identical (no `VoterCredits` entry or an entry of 0).

`OptionTotalVotes` is a running total updated on each `cast_vote` call. This
allows `get_results` to be O(options) rather than requiring a scan over all voter
keys (which Soroban does not support with prefix scans).

---

## Why not a single-file contract?

The source is split into `lib.rs`, `types.rs`, `errors.rs`, `storage.rs`, and
`credit_source.rs` to make the codebase reviewable by someone focused on a
specific concern (e.g., a security auditor can read `errors.rs` and `storage.rs`
independently). This is a deliberate choice for a boilerplate intended to be
audited and forked.
