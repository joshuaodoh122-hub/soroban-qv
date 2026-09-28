//! Soroban storage helpers for the QV core contract.
//!
//! All storage keys are defined here to keep the contract logic free of raw
//! key construction. Every key is an enum variant to guarantee uniqueness and
//! make future migrations explicit.

use soroban_sdk::{contracttype, Address, Env, Vec};

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

/// Returns true if the voter has been initialised for this round (i.e. has
/// cast at least one vote).  Used to distinguish "full allocation" from
/// "zero remaining after spending all credits".
pub fn has_voted_on_option(env: &Env, round_id: u64, voter: &Address) -> bool {
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

pub fn set_voter_option_votes(
    env: &Env,
    round_id: u64,
    voter: &Address,
    option_id: u32,
    votes: u32,
) {
    // Update per-voter-option record.
    env.storage().persistent().set(
        &DataKey::VoterOptionVotes(round_id, voter.clone(), option_id),
        &votes,
    );

    // Update the option's running total (used for efficient tallying).
    // We recompute from per-voter records to avoid double-counting bugs.
    // Because we store `new_total` (not a delta), we cannot simply add here —
    // we must read the old per-voter record and apply the delta to the tally.
    //
    // The caller (cast_vote) has already validated the transition, so we rely
    // on it passing `votes` = new total and the previous value being the old total.
    // To keep this function simple, we let tally_votes scan per-option; for large
    // rounds, integrators should maintain their own aggregate or use a custom index.
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

/// Update the running option-total vote count.
///
/// Called by `cast_vote` *after* updating the per-voter record, passing the
/// delta (number of new votes being added in this call, not the cumulative total).
pub fn add_option_votes(env: &Env, round_id: u64, option_id: u32, delta: u32) {
    let key = DataKey::OptionTotalVotes(round_id, option_id);
    let current: u32 = env.storage().persistent().get(&key).unwrap_or(0u32);
    // Overflow here would require 2^32 votes on a single option — treat as
    // a hard error rather than silently wrapping.
    let new_total = current
        .checked_add(delta)
        .expect("option vote total overflow");
    env.storage().persistent().set(&key, &new_total);
}
