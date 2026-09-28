#!/usr/bin/env bash
# scripts/demo.sh
#
# Full end-to-end demo of soroban-qv-core on Stellar testnet.
#
# What this script does:
#   1. Checks prerequisites (stellar CLI, CONTRACT_ID env var).
#   2. Generates three voter accounts and funds them via friendbot.
#   3. Creates a 3-option voting round (100 credits per voter).
#   4. Each voter casts votes across multiple options.
#   5. Checks intermediate results.
#   6. Closes the round.
#   7. Prints the final tally.
#
# Prerequisites:
#   - Stellar CLI installed and on PATH.
#   - CONTRACT_ID exported (from DEPLOY.md step 4).
#   - Network configured: `stellar network add testnet ...`
#
# Usage:
#   export CONTRACT_ID="C..."
#   bash scripts/demo.sh
#
# The script is intentionally verbose — every call is echoed so you can
# see exactly what is happening.

set -euo pipefail

# ─── Colour helpers ────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Colour

info()    { echo -e "${CYAN}[demo]${NC} $*"; }
success() { echo -e "${GREEN}[ok]${NC}   $*"; }
warn()    { echo -e "${YELLOW}[warn]${NC} $*"; }
die()     { echo -e "${RED}[error]${NC} $*" >&2; exit 1; }

# ─── Prerequisites ─────────────────────────────────────────────────────────────
info "Checking prerequisites..."

command -v stellar >/dev/null 2>&1 || die "stellar CLI not found. Install: https://developers.stellar.org/docs/tools/developer-tools/cli/install-and-setup"

[[ -n "${CONTRACT_ID:-}" ]] || die "CONTRACT_ID is not set. Deploy the contract first (see DEPLOY.md) and export CONTRACT_ID."

NETWORK="${NETWORK:-testnet}"
info "Using network: $NETWORK"
info "Contract ID:   $CONTRACT_ID"

# ─── Generate voter identities ────────────────────────────────────────────────
info ""
info "=== Step 1: Generate and fund voter accounts ==="

for VOTER in alice bob carol; do
    info "Generating key: $VOTER"
    stellar keys generate --global "demo-$VOTER" --network "$NETWORK" --overwrite 2>/dev/null || true
    info "Funding $VOTER via friendbot..."
    stellar keys fund "demo-$VOTER" --network "$NETWORK" || warn "Funding failed for $VOTER (may already be funded)"
done

ALICE=$(stellar keys address demo-alice)
BOB=$(stellar keys address demo-bob)
CAROL=$(stellar keys address demo-carol)
ADMIN=$(stellar keys address demo-alice)   # Alice is also the admin

success "Alice: $ALICE"
success "Bob:   $BOB"
success "Carol: $CAROL"

# ─── Create round ─────────────────────────────────────────────────────────────
info ""
info "=== Step 2: Create voting round (3 options, 100 credits/voter) ==="

ROUND_OUTPUT=$(stellar contract invoke \
    --id "$CONTRACT_ID" \
    --source demo-alice \
    --network "$NETWORK" \
    -- create_round \
    --admin "$ADMIN" \
    --option_ids '[1, 2, 3]' \
    --credits_per_voter 100)

# The output is a JSON-wrapped u64; parse it.
ROUND_ID=$(echo "$ROUND_OUTPUT" | tr -d '"' | tr -d ' ')
success "Round created with ID: $ROUND_ID"

# ─── Cast votes ───────────────────────────────────────────────────────────────
info ""
info "=== Step 3: Cast votes ==="

info "Alice: 7 votes on option 2 (cost: 49 credits)"
stellar contract invoke \
    --id "$CONTRACT_ID" \
    --source demo-alice \
    --network "$NETWORK" \
    -- cast_vote \
    --voter "$ALICE" \
    --round_id "$ROUND_ID" \
    --option_id 2 \
    --num_votes 7

ALICE_CREDITS=$(stellar contract invoke \
    --id "$CONTRACT_ID" \
    --network "$NETWORK" \
    -- get_voter_credits \
    --round_id "$ROUND_ID" \
    --voter "$ALICE")
success "Alice remaining credits: $ALICE_CREDITS (expected: 51)"

info "Alice: 1 vote on option 1 (marginal cost: 1 credit)"
stellar contract invoke \
    --id "$CONTRACT_ID" \
    --source demo-alice \
    --network "$NETWORK" \
    -- cast_vote \
    --voter "$ALICE" \
    --round_id "$ROUND_ID" \
    --option_id 1 \
    --num_votes 1

info "Bob: 5 votes on option 2 (cost: 25 credits)"
stellar contract invoke \
    --id "$CONTRACT_ID" \
    --source demo-bob \
    --network "$NETWORK" \
    -- cast_vote \
    --voter "$BOB" \
    --round_id "$ROUND_ID" \
    --option_id 2 \
    --num_votes 5

info "Bob: 5 votes on option 3 (cost: 25 credits)"
stellar contract invoke \
    --id "$CONTRACT_ID" \
    --source demo-bob \
    --network "$NETWORK" \
    -- cast_vote \
    --voter "$BOB" \
    --round_id "$ROUND_ID" \
    --option_id 3 \
    --num_votes 5

info "Carol: 3 votes on option 3 (cost: 9 credits) — first call"
stellar contract invoke \
    --id "$CONTRACT_ID" \
    --source demo-carol \
    --network "$NETWORK" \
    -- cast_vote \
    --voter "$CAROL" \
    --round_id "$ROUND_ID" \
    --option_id 3 \
    --num_votes 3

info "Carol: 4 more votes on option 3 (marginal cost: 7²-3²=40) — demonstrates accumulation"
stellar contract invoke \
    --id "$CONTRACT_ID" \
    --source demo-carol \
    --network "$NETWORK" \
    -- cast_vote \
    --voter "$CAROL" \
    --round_id "$ROUND_ID" \
    --option_id 3 \
    --num_votes 4

CAROL_CREDITS=$(stellar contract invoke \
    --id "$CONTRACT_ID" \
    --network "$NETWORK" \
    -- get_voter_credits \
    --round_id "$ROUND_ID" \
    --voter "$CAROL")
success "Carol remaining credits: $CAROL_CREDITS (expected: 51 — paid 49 total for 7 votes)"

# ─── Intermediate results ──────────────────────────────────────────────────────
info ""
info "=== Step 4: Intermediate results (before close) ==="

RESULTS_OPEN=$(stellar contract invoke \
    --id "$CONTRACT_ID" \
    --network "$NETWORK" \
    -- get_results \
    --round_id "$ROUND_ID")

echo "$RESULTS_OPEN"
info "Expected: option 1=1, option 2=12, option 3=12"

# ─── Close round ──────────────────────────────────────────────────────────────
info ""
info "=== Step 5: Close the round ==="

stellar contract invoke \
    --id "$CONTRACT_ID" \
    --source demo-alice \
    --network "$NETWORK" \
    -- close_round \
    --admin "$ADMIN" \
    --round_id "$ROUND_ID"

success "Round $ROUND_ID closed."

# ─── Verify post-close vote rejected ──────────────────────────────────────────
info ""
info "=== Step 6: Verify voting is rejected after close ==="

LATE_VOTE_RESULT=0
stellar contract invoke \
    --id "$CONTRACT_ID" \
    --source demo-bob \
    --network "$NETWORK" \
    -- cast_vote \
    --voter "$BOB" \
    --round_id "$ROUND_ID" \
    --option_id 1 \
    --num_votes 1 \
    2>&1 | grep -q "RoundNotOpen" && LATE_VOTE_RESULT=1 || true

if [[ $LATE_VOTE_RESULT -eq 1 ]]; then
    success "Post-close vote correctly rejected with RoundNotOpen."
else
    warn "Post-close vote did not return expected error (check output above)."
fi

# ─── Final results ─────────────────────────────────────────────────────────────
info ""
info "=== Step 7: Final results ==="

RESULTS_FINAL=$(stellar contract invoke \
    --id "$CONTRACT_ID" \
    --network "$NETWORK" \
    -- get_results \
    --round_id "$ROUND_ID")

echo ""
echo "────────────────────────────────────────"
echo "  Final vote tally for round $ROUND_ID"
echo "────────────────────────────────────────"
echo "$RESULTS_FINAL"
echo ""
echo "  Option 1 (Proposal A):   1 vote  (Alice)"
echo "  Option 2 (Proposal B):  12 votes (Alice×7 + Bob×5)"
echo "  Option 3 (Proposal C):  12 votes (Bob×5 + Carol×7)"
echo "────────────────────────────────────────"
echo ""
success "Demo complete. Contract ID: $CONTRACT_ID  Round ID: $ROUND_ID"
