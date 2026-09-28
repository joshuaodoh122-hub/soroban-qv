//! Shared data types for the QV core contract.

use soroban_sdk::{contracttype, Address, Vec};

/// The lifecycle state of a voting round.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum RoundState {
    /// Votes may be cast.
    Open,
    /// The round has been closed by its admin; no further votes are accepted.
    Closed,
}

/// Metadata stored for each round.
#[derive(Clone, Debug)]
#[contracttype]
pub struct RoundInfo {
    /// Address authorised to close this round.
    pub admin: Address,
    /// The set of valid option IDs for this round.
    pub option_ids: Vec<u32>,
    /// Voice credits allocated to each voter upon their first vote in this round.
    pub credits_per_voter: u64,
    /// Current lifecycle state.
    pub state: RoundState,
}

/// Per-(voter, option) vote record stored in contract state.
///
/// We only store the cumulative vote count; credits remaining is stored
/// separately per-voter so the contract never needs to recompute history.
#[derive(Clone, Debug)]
#[contracttype]
pub struct VoteRecord {
    /// Total votes this voter has cast on this option (cumulative across calls).
    pub total_votes: u32,
}
