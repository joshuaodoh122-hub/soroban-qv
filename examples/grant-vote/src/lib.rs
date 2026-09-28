//! # grant-vote example
//!
//! A thin wrapper around `soroban-qv-core` demonstrating how to build a
//! concrete governance use-case: choosing which of three community grant
//! proposals to fund.
//!
//! ## What this shows
//! - How to call the QV core contract from another Soroban contract.
//! - Named proposal IDs (mapped to human-readable labels off-chain).
//! - A simple "announce winner" helper that reads results from the core contract.
//!
//! ## Option IDs used
//! | option_id | Proposal                |
//! |-----------|-------------------------|
//! | 1         | Community Dev Tools     |
//! | 2         | Public Goods Fund       |
//! | 3         | Education Grant         |

#![no_std]

use soroban_sdk::{contract, contractimpl, vec, Address, Env, Vec};

// Import core types for re-export and internal use.
use soroban_qv_core::QVContractClient;
pub use soroban_qv_core::{QVError, RoundInfo, RoundState};

/// The fixed option IDs used by this example.
pub const PROPOSAL_DEV_TOOLS: u32 = 1;
pub const PROPOSAL_PUBLIC_GOODS: u32 = 2;
pub const PROPOSAL_EDUCATION: u32 = 3;

/// Default voice credits per voter for grant rounds.
pub const GRANT_ROUND_CREDITS: u64 = 100;

#[contract]
pub struct GrantVoteContract;

#[contractimpl]
impl GrantVoteContract {
    /// Initialise a new grant proposal round, returning the `round_id`.
    ///
    /// The `admin` must authorise this call.
    /// Panics (via Soroban error trap) on contract error — see QVError for codes.
    pub fn start_grant_round(env: Env, admin: Address, qv_core: Address) -> u64 {
        admin.require_auth();
        let client = QVContractClient::new(&env, &qv_core);
        let options = vec![
            &env,
            PROPOSAL_DEV_TOOLS,
            PROPOSAL_PUBLIC_GOODS,
            PROPOSAL_EDUCATION,
        ];
        client.create_round(&admin, &options, &GRANT_ROUND_CREDITS)
    }

    /// Cast a vote on a grant proposal. `proposal_id` must be 1, 2, or 3.
    pub fn vote(
        env: Env,
        voter: Address,
        qv_core: Address,
        round_id: u64,
        proposal_id: u32,
        num_votes: u32,
    ) {
        voter.require_auth();
        let client = QVContractClient::new(&env, &qv_core);
        client.cast_vote(&voter, &round_id, &proposal_id, &num_votes);
    }

    /// Close the grant round after voting ends.
    pub fn close_grant_round(env: Env, admin: Address, qv_core: Address, round_id: u64) {
        admin.require_auth();
        let client = QVContractClient::new(&env, &qv_core);
        client.close_round(&admin, &round_id);
    }

    /// Return (winning_proposal_id, vote_count).
    ///
    /// In case of a tie, returns the first tied option (by iteration order).
    /// Callers should inspect the full results for tie-handling logic.
    pub fn winning_proposal(env: Env, qv_core: Address, round_id: u64) -> (u32, u32) {
        let client = QVContractClient::new(&env, &qv_core);
        let results: Vec<(u32, u32)> = client.get_results(&round_id);

        let mut best_id: u32 = 0;
        let mut best_votes: u32 = 0;

        for (option_id, votes) in results.iter() {
            if votes > best_votes {
                best_votes = votes;
                best_id = option_id;
            }
        }

        (best_id, best_votes)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    extern crate std;

    use soroban_sdk::{testutils::Address as _, Address, Env};

    use super::*;
    use soroban_qv_core::QVContract;

    fn setup() -> (Env, Address, Address, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let core_id = env.register(QVContract, ());
        let grant_id = env.register(GrantVoteContract, ());

        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        (env, core_id, grant_id, admin, alice, bob)
    }

    #[test]
    fn test_grant_vote_full_cycle() {
        let (env, core_id, grant_id, admin, alice, bob) = setup();
        let grant_client = GrantVoteContractClient::new(&env, &grant_id);

        // Start a grant round.
        let round_id = grant_client.start_grant_round(&admin, &core_id);

        // Alice strongly prefers Public Goods (6 votes = 36 credits).
        grant_client.vote(&alice, &core_id, &round_id, &PROPOSAL_PUBLIC_GOODS, &6u32);

        // Bob prefers Dev Tools (5 votes = 25 credits) and some for Education (3 votes = 9).
        grant_client.vote(&bob, &core_id, &round_id, &PROPOSAL_DEV_TOOLS, &5u32);
        grant_client.vote(&bob, &core_id, &round_id, &PROPOSAL_EDUCATION, &3u32);

        // Close the round.
        grant_client.close_grant_round(&admin, &core_id, &round_id);

        // Check winner: Public Goods (6) vs Dev Tools (5) vs Education (3).
        let (winner_id, winner_votes) = grant_client.winning_proposal(&core_id, &round_id);

        assert_eq!(winner_id, PROPOSAL_PUBLIC_GOODS);
        assert_eq!(winner_votes, 6);
    }
}
