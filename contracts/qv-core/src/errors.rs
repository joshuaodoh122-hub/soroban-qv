//! Error types for the QV core contract.

use soroban_sdk::contracterror;

/// All errors that the QV core contract can return.
///
/// Using `contracterror` makes these values stable across contract versions
/// and readable in SDK tooling / explorers.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum QVError {
    // ── Round creation errors ─────────────────────────────────────────────
    /// `option_ids` was empty — a round must have at least one option.
    NoOptions = 1,
    /// `option_ids` contained duplicate values.
    DuplicateOption = 2,
    /// `credits_per_voter` was zero.
    ZeroCredits = 3,

    // ── Round lookup / state errors ───────────────────────────────────────
    /// No round exists with the given `round_id`.
    RoundNotFound = 4,
    /// The round is not in the `Open` state (e.g. already closed).
    RoundNotOpen = 5,

    // ── Authorisation errors ──────────────────────────────────────────────
    /// The caller is not the admin of this round.
    Unauthorized = 6,

    // ── Vote validation errors ────────────────────────────────────────────
    /// `num_votes` was zero.
    ZeroVotes = 7,
    /// `option_id` is not registered for this round.
    InvalidOption = 8,
    /// Accumulated vote count would overflow `u32`.
    VoteCountOverflow = 9,
    /// Quadratic cost computation overflowed `u64`.
    CostOverflow = 10,
    /// Voter does not have enough voice credits to cover the quadratic cost.
    InsufficientCredits = 11,
}
