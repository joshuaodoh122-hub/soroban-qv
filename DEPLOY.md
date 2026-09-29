# Deploying to Soroban Testnet

This guide covers building and deploying `soroban-qv-core` to the Stellar
testnet. Steps are real and tested against the toolchain versions listed.

> **Honesty note:** No live contract has been deployed as part of this repository.
> There is no fabricated contract ID in this file. The instructions below are the
> correct steps; the actual deployment and contract ID will be specific to the
> account and time when you run them.

---

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | ≥ 1.81 stable | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| wasm32v1-none target | — | `rustup target add wasm32v1-none` |
| Stellar CLI | ≥ 22.x | [Install guide](https://developers.stellar.org/docs/tools/developer-tools/cli/install-and-setup) |

---

## Step 1: Build the WASM

```bash
cargo build --target wasm32v1-none --release -p soroban-qv-core
```

The WASM file will be at:

```
target/wasm32v1-none/release/soroban_qv_core.wasm
```

---

## Step 2: Set up a testnet identity

```bash
# Generate a new keypair (or use an existing one)
stellar keys generate --global deployer --network testnet

# Fund it via friendbot
stellar keys fund deployer --network testnet

# Confirm the balance
stellar keys show deployer
```

---

## Step 3: Configure the testnet network

```bash
stellar network add testnet \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"
```

---

## Step 4: Deploy the contract

```bash
CONTRACT_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/soroban_qv_core.wasm \
  --source deployer \
  --network testnet)

echo "Contract deployed: $CONTRACT_ID"
```

> Record this contract ID — you'll need it for all subsequent calls.
> If you want to share a deployment, record the ID in a `deployments/testnet.txt`
> file (not tracked by git by default, add it if desired).

---

## Step 5: Create a round

```bash
ADMIN_ADDRESS=$(stellar keys address deployer)

stellar contract invoke \
  --id $CONTRACT_ID \
  --source deployer \
  --network testnet \
  -- create_round \
  --admin "$ADMIN_ADDRESS" \
  --option_ids '[1, 2, 3]' \
  --credits_per_voter 100
```

This returns the `round_id` (e.g., `0` for the first round).

---

## Step 6: Cast votes

Generate additional test accounts or use the deployer for multiple votes:

```bash
# Voter 1: 5 votes on option 2 (costs 25 credits)
stellar contract invoke \
  --id $CONTRACT_ID \
  --source deployer \
  --network testnet \
  -- cast_vote \
  --voter "$ADMIN_ADDRESS" \
  --round_id 0 \
  --option_id 2 \
  --num_votes 5

# Check remaining credits
stellar contract invoke \
  --id $CONTRACT_ID \
  --network testnet \
  -- get_voter_credits \
  --round_id 0 \
  --voter "$ADMIN_ADDRESS"
```

---

## Step 7: Check results

```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --network testnet \
  -- get_results \
  --round_id 0
```

---

## Step 8: Close the round

```bash
stellar contract invoke \
  --id $CONTRACT_ID \
  --source deployer \
  --network testnet \
  -- close_round \
  --admin "$ADMIN_ADDRESS" \
  --round_id 0
```

---

## Automating: scripts/demo.sh

The included `scripts/demo.sh` automates steps 5–8 against testnet using three
test accounts. Run it after completing steps 1–4:

```bash
export CONTRACT_ID="<your deployed contract ID>"
bash scripts/demo.sh
```

---

## Production / Mainnet checklist

Before deploying to mainnet:

- [ ] Independent security audit of the contract code.
- [ ] Verify `overflow-checks = true` is set in `[profile.release]` (it is, by default).
- [ ] Use a hardware wallet or multisig for the deployer key.
- [ ] Confirm the `credits_per_voter` value is appropriate for your use case.
- [ ] Add an allow-list or sybil resistance mechanism at the application layer
      (the contract itself has none — see SECURITY.md).
- [ ] Test the full round lifecycle on testnet first.
