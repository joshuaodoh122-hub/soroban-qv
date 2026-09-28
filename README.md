# soroban-qv-core

**Quadratic Voting (QV) core boilerplate for Soroban smart contracts.**

[![CI](https://github.com/your-org/soroban-qv/actions/workflows/ci.yml/badge.svg)](https://github.com/your-org/soroban-qv/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## What is Quadratic Voting?

Quadratic Voting is a governance mechanism that prevents the richest or loudest
participant from dominating every vote. Instead of "one token, one vote," each
participant is given a budget of **voice credits**, and the cost to cast votes
is *quadratic*:

| Votes you want | Credits it costs |
|---------------|-----------------|
| 1             | 1               |
| 2             | 4               |
| 3             | 9               |
| 5             | 25              |
| 10            | 100             |

**Formula:** casting N votes costs N² credits.

This means you *can* express a strong preference — but doing so is expensive.
A participant who wants to express 10× the preference of another must spend 100×
the credits, not 10×. This compresses power asymmetries while still rewarding
genuine conviction.

### Concrete example

A DAO is choosing which of three grant proposals to fund. Each voter gets 100
voice credits.

- Alice cares deeply about proposal B: she puts 7 votes on it (cost: 49 credits)
  and 1 vote on proposal A (cost: 1 credit). Spent: 50.
- Bob splits evenly: 5 votes each on B and C (cost: 50 total).
- Carol goes all-in on C: 7 votes in two calls — first 3 (cost: 9), then 4 more
  (cost: 16, because she's topping up to 7 total). Spent: 49.

Final tally: A=1, B=12, C=12. A tie between B and C — a realistic outcome that
shows QV distributes weight meaningfully even across a small group.

---

## Why this matters

DAOs and governance systems on Stellar/Soroban need robust primitives. Most
existing on-chain voting is either token-weighted (whales dominate) or one-person
one-vote (ignores preference intensity). Quadratic Voting sits between these
extremes and is used in real-world governance (Colorado state legislature budget
experiments, Gitcoin grants, Optimism RPGF).

This library is meant as **reusable infrastructure** — a composable core that
DAO builders can integrate rather than re-implement. Getting the math, overflow
handling, and state machine right once, in a tested and auditable codebase, is
more valuable than every project rolling its own.

---

## Contract Interface

### `create_round(admin, option_ids, credits_per_voter) → Result<u64, QVError>`

Creates a new voting round. Returns the `round_id`.

| Parameter          | Type       | Description                                         |
|--------------------|------------|-----------------------------------------------------|
| `admin`            | `Address`  | Auth-required. Becomes the only address that can close this round. |
| `option_ids`       | `Vec<u32>` | Non-empty list of distinct option identifiers.     |
| `credits_per_voter`| `u64`      | Voice credits given to each voter (fixed per round). |

**Errors:** `NoOptions`, `DuplicateOption`, `ZeroCredits`.

---

### `cast_vote(voter, round_id, option_id, num_votes) → Result<(), QVError>`

Cast votes on a single option. Quadratic cost is deducted from the voter's credit balance.

**Accumulation:** repeated calls on the same option accumulate. The cost is always
`new_total² − old_total²`, so there is no way to get a discount by splitting calls.

| Parameter   | Type      | Description                                   |
|-------------|-----------|-----------------------------------------------|
| `voter`     | `Address` | Auth-required.                               |
| `round_id`  | `u64`     | Must be an open round.                        |
| `option_id` | `u32`     | Must be a registered option for this round.  |
| `num_votes` | `u32`     | Additional votes to cast (≥ 1).              |

**Errors:** `RoundNotFound`, `RoundNotOpen`, `InvalidOption`, `ZeroVotes`,
`VoteCountOverflow`, `CostOverflow`, `InsufficientCredits`.

---

### `get_results(round_id) → Result<Vec<(u32, u32)>, QVError>`

Returns `[(option_id, total_votes), ...]`. This is the **vote count**, not
credits spent. Results are available at any time (open or closed).

---

### `close_round(admin, round_id) → Result<(), QVError>`

Closes the round. Only the address that created the round may call this.
After closing, `cast_vote` will fail with `RoundNotOpen`.

---

### `get_round_info(round_id) → Result<RoundInfo, QVError>`

Returns round metadata: state, admin, options, credits per voter.

---

### `get_voter_credits(round_id, voter) → Result<u64, QVError>`

Returns the voter's remaining voice credits. Returns the full allocation if
they have not yet voted in this round.

---

## Quick Start

### Prerequisites

- Rust stable (≥ 1.81)
- `wasm32v1-none` target: `rustup target add wasm32v1-none`
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/install-and-setup)

### Build

```bash
cd soroban-qv
cargo build --target wasm32v1-none --release -p soroban-qv-core
```

### Test

```bash
cargo test --workspace --all-features
```

### Deploy to testnet

See [DEPLOY.md](DEPLOY.md) for full instructions.

### Example call sequence (Stellar CLI)

```bash
# Create a round with 3 options and 100 credits per voter
stellar contract invoke \
  --id $CONTRACT_ID \
  -- create_round \
  --admin $ADMIN_ADDRESS \
  --option_ids '[1, 2, 3]' \
  --credits_per_voter 100

# Cast votes
stellar contract invoke \
  --id $CONTRACT_ID \
  -- cast_vote \
  --voter $VOTER_ADDRESS \
  --round_id 0 \
  --option_id 2 \
  --num_votes 5

# Get results
stellar contract invoke \
  --id $CONTRACT_ID \
  -- get_results \
  --round_id 0

# Close the round
stellar contract invoke \
  --id $CONTRACT_ID \
  -- close_round \
  --admin $ADMIN_ADDRESS \
  --round_id 0
```

---

## Known Limitations

These are stated plainly, not glossed over:

1. **No sybil resistance.** Any address can vote. There is no identity
   verification, stake requirement, or uniqueness check. In practice, this
   means an attacker with many funded accounts can dominate a round. Sybil
   resistance is a hard, unsolved problem and is explicitly out of scope here.
   Integrators who need it should add an allow-list, a minimum stake requirement,
   or a third-party identity layer before calling `cast_vote`.

2. **Fixed credit allocation.** Every voter receives the same number of credits
   per round. Token-weighted allocation (proportional to balance at snapshot) is
   stubbed as the `CreditSource` trait but not implemented. This is a planned v2
   feature — see Roadmap.

3. **No vote delegation.** A voter cannot delegate their credits to another
   address. Planned for v2.

4. **No time-based round expiry.** Rounds only close when the admin calls
   `close_round`. Integrators who need time-based expiry must enforce it in a
   higher-level contract or off-chain orchestration.

5. **Option IDs are caller-supplied `u32`s.** There is no on-chain string label
   for options. Labels must be maintained off-chain or in a wrapper contract.

---

## Roadmap

Genuinely planned, not aspirational filler:

- **v0.2:** Token-weighted credit sourcing (`CreditSource` trait implementation
  using SAC token balance at a ledger-sequence snapshot).
- **v0.3:** Delegate voting — allow a voter to assign their credits to a
  trusted delegate.
- **v0.4:** Time-gated rounds using Soroban ledger sequence numbers.
- **v1.0:** Sybil resistance integration via Stellar's identity/attestation
  ecosystem, once that matures.

---

## Project Structure

```
soroban-qv/
├── contracts/
│   └── qv-core/         # Core QV contract (integrate this)
│       └── src/
│           ├── lib.rs          # Contract entry points
│           ├── types.rs        # RoundInfo, RoundState, VoteRecord
│           ├── errors.rs       # QVError enum
│           ├── storage.rs      # All ledger key/storage helpers
│           ├── credit_source.rs # CreditSource trait stub
│           └── tests.rs        # Comprehensive tests
├── examples/
│   └── grant-vote/      # Example: grant proposal voting
├── scripts/
│   └── demo.sh          # Testnet demo script
├── .github/workflows/
│   └── ci.yml           # CI: fmt, clippy, test, build-wasm
├── ARCHITECTURE.md
├── CHANGELOG.md
├── CONTRIBUTING.md
├── DEPLOY.md
└── SECURITY.md
```

---

## License

MIT — see [LICENSE](LICENSE).
