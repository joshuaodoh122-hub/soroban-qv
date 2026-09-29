//! Soroban storage helpers for the QV core contract.
//!
//! All storage keys are defined here to keep the contract logic free of raw
//! key construction. Every key is an enum variant to guarantee uniqueness and
//! make future migrations explicit.

use soroban_sdk::{contracttype, Address, Env, Vec};

use crate::errors::QVError;
use crate::types::RoundInfo;

// ─── Storage keys ─────────────────────────────────────────────────────────────

/// Top-level storage keys.
#[contracttype]
#[derive(Clone)]
enum DataKey {
    /// Monotonic counter for round IDs.
    NextRoundId,
    /// Metadata for a specific round.
    Round(u64),
    /// Whether a voter has initialised their credit ledger for a round.
    /// Stored as a bool sentinel to distinguish "never voted" from "voted,
    /// has zero remaining credits".
    VoterInitialised(u64, Address),
    /// Remaining voice credits for a voter in a round.
    VoterCredits(u64, Address),
    /// Accumulated votes by a voter on a specific option in a round.
    VoterOptionVotes(u64, Address, u32),
    /// Accumulated total votes on an option across all voters (for fast tallying).
    OptionTotalVotes(u64, u32),
}

// ─── Round counter ─────────────────────────────────────────────────────────────

/// Read the next available round ID and atomically increment the counter.
pub fn next_round_id(env: &Env) -> u64 {
    let key = DataKey::NextRoundId;
    let current: u64 = env.storage().instance().get(&key).unwrap_or(0u64);
    let next = current
        .checked_add(1)
        .expect("round ID counter overflow — impossibly many rounds");
    env.storage().instance().set(&key, &next);
    // We use `current` as the ID so the first round is 0.
    current
}

// ─── Round metadata ────────────────────────────────────────────────────────────

pub fn save_round(env: &Env, round_id: u64, round: &RoundInfo) {
    env.storage()
        .persistent()
        .set(&DataKey::Round(round_id), round);
}

pub fn get_round(env: &Env, round_id: u64) -> Option<RoundInfo> {
    env.storage().persistent().get(&DataKey::Round(round_id))
}

// ─── Voter credit ledger ────────────────────────────────────────────────────────

/// Returns `true` if the voter has been initialised for this round (i.e. has
/// cast at least one vote in it).
///
/// Used to distinguish "voter has never voted → return full allocation" from
/// "voter has voted and may have zero credits left → return the stored value".
/// The sentinel key `VoterInitialised` is written on first `set_voter_credits_remaining`
/// call, regardless of how many credits are left.
pub fn is_voter_initialised(env: &Env, round_id: u64, voter: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::VoterInitialised(round_id, voter.clone()))
}

pub fn get_voter_credits_remaining(env: &Env, round_id: u64, voter: &Address) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::VoterCredits(round_id, voter.clone()))
        .unwrap_or(0u64)
}

pub fn set_voter_credits_remaining(env: &Env, round_id: u64, voter: &Address, credits: u64) {
    // Mark voter as initialised on first write.
    let init_key = DataKey::VoterInitialised(round_id, voter.clone());
    if !env.storage().persistent().has(&init_key) {
        env.storage().persistent().set(&init_key, &true);
    }
    env.storage()
        .persistent()
        .set(&DataKey::VoterCredits(round_id, voter.clone()), &credits);
}

// ─── Per-(voter, option) vote counts ───────────────────────────────────────────

pub fn get_voter_option_votes(env: &Env, round_id: u64, voter: &Address, option_id: u32) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::VoterOptionVotes(
            round_id,
            voter.clone(),
            option_id,
        ))
        .unwrap_or(0u32)
}

/// Persist the cumulative vote count for a (voter, option) pair.
///
/// This function stores only the per-voter record.  The option-level aggregate
/// is maintained separately by `add_option_votes`, which must be called in the
/// same transaction (as `cast_vote` does) to keep the two counters consistent:
///
/// - `VoterOptionVotes(round, voter, option)` — how many votes *this voter*
///   has cast on *this option* (cumulative, used for marginal QV cost computation).
/// - `OptionTotalVotes(round, option)` — running sum across *all voters* on
///   *this option* (used by `get_results` / `tally_votes` for O(options) tallying).
///
/// Always call `add_option_votes` with the delta (not the new cumulative total)
/// immediately after this call.
pub fn set_voter_option_votes(
    env: &Env,
    round_id: u64,
    voter: &Address,
    option_id: u32,
    votes: u32,
) {
    env.storage().persistent().set(
        &DataKey::VoterOptionVotes(round_id, voter.clone(), option_id),
        &votes,
    );
}

// ─── Vote tallying ──────────────────────────────────────────────────────────────

/// Tally votes across all voters for each option.
///
/// # Implementation note
/// We iterate the option list and scan for all voter-option keys.  Because
/// Soroban does not support range/prefix scans on storage, we store a running
/// option total that is updated on each `cast_vote`.  See `OptionTotalVotes`.
pub fn tally_votes(env: &Env, round_id: u64, option_ids: &Vec<u32>) -> Vec<(u32, u32)> {
    let mut results = Vec::new(env);
    for option_id in option_ids.iter() {
        let total: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::OptionTotalVotes(round_id, option_id))
            .unwrap_or(0u32);
        results.push_back((option_id, total));
    }
    results
}

/// Increment the running per-option vote total by `delta`.
///
/// Called by `cast_vote` after updating the per-voter record, passing the
/// number of new votes being added in this call (not the cumulative total).
///
/// # Errors
/// Returns [`QVError::OptionTotalOverflow`] if adding `delta` would overflow
/// `u32`.  In practice this requires over 4 billion aggregate votes on a single
/// option, which is economically implausible given quadratic costs — but the
/// contract returns a typed error rather than panicking.
pub fn add_option_votes(
    env: &Env,
    round_id: u64,
    option_id: u32,
    delta: u32,
) -> Result<(), QVError> {
    let key = DataKey::OptionTotalVotes(round_id, option_id);
    let current: u32 = env.storage().persistent().get(&key).unwrap_or(0u32);
    let new_total = current
        .checked_add(delta)
        .ok_or(QVError::OptionTotalOverflow)?;
    env.storage().persistent().set(&key, &new_total);
    Ok(())
}

// ─── Test-only helpers ──────────────────────────────────────────────────────────

/// Directly write the option total vote counter.
///
/// **For test setup only** — allows tests to position the counter near
/// `u32::MAX` without running an impractically large number of `cast_vote`
/// calls.  Not compiled into production WASM.
#[cfg(test)]
pub fn set_option_total_votes_for_test(env: &Env, round_id: u64, option_id: u32, total: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::OptionTotalVotes(round_id, option_id), &total);
}
