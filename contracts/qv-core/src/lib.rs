//! # soroban-qv-core
//!
//! Quadratic Voting (QV) core boilerplate for Soroban smart contracts.
//!
//! ## What is Quadratic Voting?
//!
//! Quadratic Voting is a governance mechanism where voters allocate "voice credits"
//! to options, but the cost of votes is *quadratic*: casting N votes on an option
//! costs N² credits. This means expressing strong preference is possible but
//! expensive, preventing whale domination while still allowing nuanced preference
//! expression.
//!
//! Example: 1 vote = 1 credit, 2 votes = 4 credits, 5 votes = 25 credits.
//!
//! ## Design Decisions (see ARCHITECTURE.md for full rationale)
//!
//! - **Accumulated votes per (voter, option)**: Repeated `cast_vote` calls on the
//!   same option by the same voter accumulate. The marginal cost is
//!   `new_total² − old_total²`. This is the economically correct QV behavior.
//! - **Fixed credit allocation**: Each voter receives a configurable fixed number
//!   of voice credits per round. Token-weighted allocation is stubbed as a trait.
//! - **Round admin**: The address that creates a round is its admin and is the only
//!   one who can close it. No global admin exists — composable by design.

#![no_std]

pub mod credit_source;
pub mod errors;
pub mod storage;
pub mod types;

#[cfg(test)]
mod tests;

pub use errors::QVError;
pub use types::{RoundInfo, RoundState, VoteRecord};

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Vec};

use storage::{
    add_option_votes, get_round, get_voter_credits_remaining, get_voter_option_votes,
    has_voted_on_option, next_round_id, save_round, set_voter_credits_remaining,
    set_voter_option_votes,
};

// ─── Contract ────────────────────────────────────────────────────────────────

#[contract]
pub struct QVContract;

#[contractimpl]
impl QVContract {
    // ── Round lifecycle ───────────────────────────────────────────────────────

    /// Create a new voting round.
    ///
    /// # Arguments
    /// * `admin`             – The address authorised to close this round. Must sign.
    /// * `option_ids`        – Non-empty list of distinct `u32` option identifiers.
    /// * `credits_per_voter` – Voice credits allocated to each voter on first vote.
    ///
    /// # Returns
    /// The new `round_id` (a `u64` assigned monotonically from 0).
    ///
    /// # Errors
    /// * [`QVError::NoOptions`]       – `option_ids` is empty.
    /// * [`QVError::DuplicateOption`] – `option_ids` contains duplicates.
    /// * [`QVError::ZeroCredits`]     – `credits_per_voter` is zero.
    pub fn create_round(
        env: Env,
        admin: Address,
        option_ids: Vec<u32>,
        credits_per_voter: u64,
    ) -> Result<u64, QVError> {
        admin.require_auth();

        if option_ids.is_empty() {
            return Err(QVError::NoOptions);
        }
        if credits_per_voter == 0 {
            return Err(QVError::ZeroCredits);
        }

        // Check for duplicate option IDs (O(n²) — option lists are expected small).
        let len = option_ids.len();
        for i in 0..len {
            for j in (i + 1)..len {
                if option_ids.get(i).unwrap() == option_ids.get(j).unwrap() {
                    return Err(QVError::DuplicateOption);
                }
            }
        }

        let round_id = next_round_id(&env);

        let round = RoundInfo {
            admin: admin.clone(),
            option_ids,
            credits_per_voter,
            state: RoundState::Open,
        };

        save_round(&env, round_id, &round);

        env.events()
            .publish((symbol_short!("qv"), symbol_short!("created")), round_id);

        Ok(round_id)
    }

    /// Close a round, preventing any further votes.
    ///
    /// Only the round's admin (the address that called `create_round`) may close it.
    ///
    /// # Errors
    /// * [`QVError::RoundNotFound`] – No round with `round_id` exists.
    /// * [`QVError::RoundNotOpen`]  – Round is already closed.
    /// * [`QVError::Unauthorized`]  – Caller is not the round admin.
    pub fn close_round(env: Env, admin: Address, round_id: u64) -> Result<(), QVError> {
        admin.require_auth();

        let mut round = get_round(&env, round_id).ok_or(QVError::RoundNotFound)?;

        if round.state != RoundState::Open {
            return Err(QVError::RoundNotOpen);
        }
        if round.admin != admin {
            return Err(QVError::Unauthorized);
        }

        round.state = RoundState::Closed;
        save_round(&env, round_id, &round);

        env.events()
            .publish((symbol_short!("qv"), symbol_short!("closed")), round_id);

        Ok(())
    }

    // ── Voting ────────────────────────────────────────────────────────────────

    /// Cast votes on an option within a round.
    ///
    /// ## Accumulation behaviour (important — read before integrating)
    ///
    /// Repeated calls by the same voter on the same option **accumulate**.
    /// The cost charged is the *marginal* quadratic increment:
    ///
    /// ```text
    /// prev_votes = votes already cast by voter on this option
    /// new_total  = prev_votes + num_votes
    /// cost       = new_total² − prev_votes²
    /// ```
    ///
    /// Calling `cast_vote(option=A, 3)` then `cast_vote(option=A, 2)` costs
    /// `9 + (25−9) = 25` total — the same as `cast_vote(option=A, 5)`.
    /// There is no way to obtain a discount by splitting calls.
    ///
    /// # Arguments
    /// * `voter`     – Voter address. Must sign.
    /// * `round_id`  – Target round (must be Open).
    /// * `option_id` – Target option (must exist in the round).
    /// * `num_votes` – Additional votes to cast (must be ≥ 1).
    ///
    /// # Errors
    /// * [`QVError::RoundNotFound`]       – No round with `round_id`.
    /// * [`QVError::RoundNotOpen`]        – Round is closed.
    /// * [`QVError::InvalidOption`]       – `option_id` not in this round.
    /// * [`QVError::ZeroVotes`]           – `num_votes` is 0.
    /// * [`QVError::VoteCountOverflow`]   – Accumulated votes would overflow `u32`.
    /// * [`QVError::CostOverflow`]        – Quadratic cost overflows `u64`.
    /// * [`QVError::InsufficientCredits`] – Voter lacks enough voice credits.
    pub fn cast_vote(
        env: Env,
        voter: Address,
        round_id: u64,
        option_id: u32,
        num_votes: u32,
    ) -> Result<(), QVError> {
        voter.require_auth();

        if num_votes == 0 {
            return Err(QVError::ZeroVotes);
        }

        let round = get_round(&env, round_id).ok_or(QVError::RoundNotFound)?;

        if round.state != RoundState::Open {
            return Err(QVError::RoundNotOpen);
        }

        // Validate option ID exists in this round.
        let valid_option = round.option_ids.iter().any(|id| id == option_id);
        if !valid_option {
            return Err(QVError::InvalidOption);
        }

        // Initialise voice credits for first-time voters in this round.
        let credits_remaining = if has_voted_on_option(&env, round_id, &voter) {
            get_voter_credits_remaining(&env, round_id, &voter)
        } else {
            round.credits_per_voter
        };

        // Existing votes this voter has cast on this specific option.
        let prev_votes: u32 = get_voter_option_votes(&env, round_id, &voter, option_id);

        // new_total = prev_votes + num_votes  (checked for u32 overflow).
        let new_total: u32 = prev_votes
            .checked_add(num_votes)
            .ok_or(QVError::VoteCountOverflow)?;

        // cost = new_total² − prev_votes²  (computed in u64 to prevent overflow).
        //
        // Maximum safe new_total for u64 cost: sqrt(u64::MAX) ≈ 4.29×10⁹, which
        // exceeds u32::MAX (≈ 4.29×10⁹).  Since new_total is a u32, new_total²
        // fits in a u64 as long as new_total ≤ 65535 without risk; for values up
        // to u32::MAX the product can reach ~1.8×10¹⁹ which fits in u64::MAX
        // (~1.8×10¹⁹).  We use checked_mul regardless to be explicit.
        let new_sq: u64 = (new_total as u64)
            .checked_mul(new_total as u64)
            .ok_or(QVError::CostOverflow)?;
        let old_sq: u64 = (prev_votes as u64)
            .checked_mul(prev_votes as u64)
            .ok_or(QVError::CostOverflow)?;
        // new_sq ≥ old_sq always (new_total ≥ prev_votes), so this cannot underflow.
        let marginal_cost: u64 = new_sq - old_sq;

        if marginal_cost > credits_remaining {
            return Err(QVError::InsufficientCredits);
        }

        let new_credits = credits_remaining - marginal_cost;

        // Persist updated voter state.
        set_voter_option_votes(&env, round_id, &voter, option_id, new_total);
        set_voter_credits_remaining(&env, round_id, &voter, new_credits);

        // Update the option's running total (delta = num_votes, not new_total).
        add_option_votes(&env, round_id, option_id, num_votes);

        env.events().publish(
            (symbol_short!("qv"), symbol_short!("vote")),
            (round_id, option_id, voter, num_votes),
        );

        Ok(())
    }

    // ── Results ───────────────────────────────────────────────────────────────

    /// Return the vote tally for every option in a round.
    ///
    /// Returns `Vec<(option_id, total_votes)>` — the **raw vote count**, not the
    /// credits spent.  Credits spent are the quadratic cost; the vote count is
    /// what determines the outcome.
    ///
    /// Can be called at any time (during or after a round).
    ///
    /// # Errors
    /// * [`QVError::RoundNotFound`] – No round with `round_id`.
    pub fn get_results(env: Env, round_id: u64) -> Result<Vec<(u32, u32)>, QVError> {
        let round = get_round(&env, round_id).ok_or(QVError::RoundNotFound)?;
        Ok(storage::tally_votes(&env, round_id, &round.option_ids))
    }

    /// Return metadata about a round (state, admin, options, credits per voter).
    ///
    /// # Errors
    /// * [`QVError::RoundNotFound`] – No round with `round_id`.
    pub fn get_round_info(env: Env, round_id: u64) -> Result<RoundInfo, QVError> {
        get_round(&env, round_id).ok_or(QVError::RoundNotFound)
    }

    /// Return the remaining voice credits for a voter in a round.
    ///
    /// Returns the full `credits_per_voter` allocation if the voter has not yet
    /// cast any votes in this round.
    ///
    /// # Errors
    /// * [`QVError::RoundNotFound`] – No round with `round_id`.
    pub fn get_voter_credits(env: Env, round_id: u64, voter: Address) -> Result<u64, QVError> {
        let round = get_round(&env, round_id).ok_or(QVError::RoundNotFound)?;
        if has_voted_on_option(&env, round_id, &voter) {
            Ok(get_voter_credits_remaining(&env, round_id, &voter))
        } else {
            Ok(round.credits_per_voter)
        }
    }
}
