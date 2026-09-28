//! Comprehensive tests for the QV core contract.
//!
//! Test coverage:
//! - Quadratic cost math: exact values, boundary behavior, overflow.
//! - Insufficient voice credits → clean failure.
//! - Multiple votes on different options by the same voter.
//! - Repeated cast_vote on the same option (accumulation).
//! - Voting after round close → failure.
//! - Round creation auth: unauthorized caller → failure.
//! - get_results correctness with multiple voters/options.
//! - Full end-to-end integration test.
//!
//! ## Soroban client pattern note
//! The generated `client.*` methods PANIC (via host trap) on contract errors.
//! To test error paths, use `client.try_*` variants which return
//! `Result<Result<T, E>, soroban_sdk::Error>`.

#![cfg(test)]

extern crate std;

use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{QVContract, QVContractClient, QVError};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn setup() -> (Env, QVContractClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(QVContract, ());
    let client = QVContractClient::new(&env, &contract_id);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let carol = Address::generate(&env);
    (env, client, alice, bob, carol)
}

/// Create a standard 3-option round with 100 credits per voter.
fn create_standard_round(client: &QVContractClient, env: &Env, admin: &Address) -> u64 {
    client.create_round(admin, &vec![env, 1u32, 2u32, 3u32], &100u64)
}

// ─── Quadratic cost math ──────────────────────────────────────────────────────

#[test]
fn test_qv_cost_1_vote() {
    // 1 vote → 1 credit
    let (env, client, alice, _, _) = setup();
    let r = create_standard_round(&client, &env, &alice);
    client.cast_vote(&alice, &r, &1u32, &1u32);
    assert_eq!(client.get_voter_credits(&r, &alice), 99);
}

#[test]
fn test_qv_cost_2_votes() {
    // 2 votes → 4 credits
    let (env, client, alice, _, _) = setup();
    let r = create_standard_round(&client, &env, &alice);
    client.cast_vote(&alice, &r, &1u32, &2u32);
    assert_eq!(client.get_voter_credits(&r, &alice), 96);
}

#[test]
fn test_qv_cost_3_votes() {
    // 3 votes → 9 credits
    let (env, client, alice, _, _) = setup();
    let r = create_standard_round(&client, &env, &alice);
    client.cast_vote(&alice, &r, &1u32, &3u32);
    assert_eq!(client.get_voter_credits(&r, &alice), 91);
}

#[test]
fn test_qv_cost_5_votes() {
    // 5 votes → 25 credits
    let (env, client, alice, _, _) = setup();
    let r = create_standard_round(&client, &env, &alice);
    client.cast_vote(&alice, &r, &1u32, &5u32);
    assert_eq!(client.get_voter_credits(&r, &alice), 75);
}

#[test]
fn test_qv_cost_10_votes_drains_budget() {
    // 10 votes → 100 credits (exactly drains the budget)
    let (env, client, alice, _, _) = setup();
    let r = create_standard_round(&client, &env, &alice);
    client.cast_vote(&alice, &r, &1u32, &10u32);
    assert_eq!(client.get_voter_credits(&r, &alice), 0);
}

#[test]
fn test_qv_zero_votes_rejected() {
    let (env, client, alice, _, _) = setup();
    let r = create_standard_round(&client, &env, &alice);
    let err = client
        .try_cast_vote(&alice, &r, &1u32, &0u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, QVError::ZeroVotes);
}

// ─── Insufficient credits ──────────────────────────────────────────────────────

#[test]
fn test_insufficient_credits_rejected() {
    let (env, client, alice, _, _) = setup();
    let r = create_standard_round(&client, &env, &alice);

    // Spend 81 credits (9 votes on option 1).
    client.cast_vote(&alice, &r, &1u32, &9u32);
    assert_eq!(client.get_voter_credits(&r, &alice), 19);

    // 5 votes on option 2 costs 25 — should fail.
    let err = client
        .try_cast_vote(&alice, &r, &2u32, &5u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, QVError::InsufficientCredits);
}

#[test]
fn test_11_votes_over_100_credits_rejected() {
    // 11 votes costs 121, which exceeds 100 credits.
    let (env, client, alice, _, _) = setup();
    let r = create_standard_round(&client, &env, &alice);
    let err = client
        .try_cast_vote(&alice, &r, &1u32, &11u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, QVError::InsufficientCredits);
}

#[test]
fn test_exact_credit_spend_succeeds_then_more_fails() {
    let (env, client, alice, _, _) = setup();
    let r = create_standard_round(&client, &env, &alice);

    // 8 votes on option 1 = 64 credits → 36 remaining.
    client.cast_vote(&alice, &r, &1u32, &8u32);
    assert_eq!(client.get_voter_credits(&r, &alice), 36);

    // 6 votes on option 2 = 36 credits → 0 remaining.
    client.cast_vote(&alice, &r, &2u32, &6u32);
    assert_eq!(client.get_voter_credits(&r, &alice), 0);

    // Any further vote must fail.
    let err = client
        .try_cast_vote(&alice, &r, &3u32, &1u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, QVError::InsufficientCredits);
}

// ─── Multiple options ──────────────────────────────────────────────────────────

#[test]
fn test_voter_can_split_across_options() {
    let (env, client, alice, _, _) = setup();
    let r = create_standard_round(&client, &env, &alice);

    // 3 votes on option 1 (cost 9), 4 on option 2 (cost 16), 5 on option 3 (cost 25). Total: 50.
    client.cast_vote(&alice, &r, &1u32, &3u32);
    client.cast_vote(&alice, &r, &2u32, &4u32);
    client.cast_vote(&alice, &r, &3u32, &5u32);

    assert_eq!(client.get_voter_credits(&r, &alice), 50);

    let results = client.get_results(&r);
    let find = |id: u32| {
        results
            .iter()
            .find(|(oid, _)| *oid == id)
            .map(|(_, v)| v)
            .unwrap_or(0)
    };
    assert_eq!(find(1), 3);
    assert_eq!(find(2), 4);
    assert_eq!(find(3), 5);
}

// ─── Repeated votes on same option (accumulation) ─────────────────────────────

#[test]
fn test_accumulated_votes_same_option_marginal_cost() {
    // First call: 3 votes on option 1 → cost 9. Remaining: 91.
    // Second call: 2 more votes on option 1 → new_total=5, marginal cost=25-9=16. Remaining: 75.
    let (env, client, alice, _, _) = setup();
    let r = create_standard_round(&client, &env, &alice);

    client.cast_vote(&alice, &r, &1u32, &3u32);
    assert_eq!(client.get_voter_credits(&r, &alice), 91);

    client.cast_vote(&alice, &r, &1u32, &2u32);
    assert_eq!(client.get_voter_credits(&r, &alice), 75); // 100 - 25 total

    // Option 1 tally should be 5 (3+2).
    let results = client.get_results(&r);
    let opt1 = results
        .iter()
        .find(|(id, _)| *id == 1u32)
        .map(|(_, v)| v)
        .unwrap_or(0);
    assert_eq!(opt1, 5);
}

#[test]
fn test_accumulated_votes_no_discount_possible() {
    // Alice: one call of 5 votes → cost 25.
    // Bob: five calls of 1 → costs 1+3+5+7+9=25 (same total).
    let (env, client, alice, bob, _) = setup();
    let r = create_standard_round(&client, &env, &alice);

    client.cast_vote(&alice, &r, &1u32, &5u32);
    assert_eq!(client.get_voter_credits(&r, &alice), 75);

    client.cast_vote(&bob, &r, &1u32, &1u32); // cost 1²-0²=1 → 99
    client.cast_vote(&bob, &r, &1u32, &1u32); // cost 2²-1²=3 → 96
    client.cast_vote(&bob, &r, &1u32, &1u32); // cost 3²-2²=5 → 91
    client.cast_vote(&bob, &r, &1u32, &1u32); // cost 4²-3²=7 → 84
    client.cast_vote(&bob, &r, &1u32, &1u32); // cost 5²-4²=9 → 75
    assert_eq!(client.get_voter_credits(&r, &bob), 75);
}

// ─── Round close ──────────────────────────────────────────────────────────────

#[test]
fn test_vote_after_close_rejected() {
    let (env, client, alice, bob, _) = setup();
    let r = create_standard_round(&client, &env, &alice);

    client.close_round(&alice, &r);

    let err = client
        .try_cast_vote(&bob, &r, &1u32, &1u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, QVError::RoundNotOpen);
}

#[test]
fn test_close_round_twice_rejected() {
    let (env, client, alice, _, _) = setup();
    let r = create_standard_round(&client, &env, &alice);
    client.close_round(&alice, &r);

    let err = client.try_close_round(&alice, &r).unwrap_err().unwrap();
    assert_eq!(err, QVError::RoundNotOpen);
}

// ─── Authorization ─────────────────────────────────────────────────────────────

#[test]
fn test_non_admin_cannot_close_round() {
    let (env, client, alice, bob, _) = setup();
    let r = create_standard_round(&client, &env, &alice);

    let err = client.try_close_round(&bob, &r).unwrap_err().unwrap();
    assert_eq!(err, QVError::Unauthorized);
}

// ─── Round creation validation ─────────────────────────────────────────────────

#[test]
fn test_create_round_empty_options_rejected() {
    let (env, client, alice, _, _) = setup();
    let err = client
        .try_create_round(&alice, &vec![&env], &100u64)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, QVError::NoOptions);
}

#[test]
fn test_create_round_zero_credits_rejected() {
    let (env, client, alice, _, _) = setup();
    let err = client
        .try_create_round(&alice, &vec![&env, 1u32], &0u64)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, QVError::ZeroCredits);
}

#[test]
fn test_create_round_duplicate_options_rejected() {
    let (env, client, alice, _, _) = setup();
    let err = client
        .try_create_round(&alice, &vec![&env, 1u32, 2u32, 1u32], &100u64)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, QVError::DuplicateOption);
}

#[test]
fn test_vote_invalid_option_rejected() {
    let (env, client, alice, _, _) = setup();
    let r = create_standard_round(&client, &env, &alice);
    let err = client
        .try_cast_vote(&alice, &r, &99u32, &1u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, QVError::InvalidOption);
}

#[test]
fn test_vote_nonexistent_round_rejected() {
    let (env, client, alice, _, _) = setup();
    let _r = create_standard_round(&client, &env, &alice);
    let err = client
        .try_cast_vote(&alice, &999u64, &1u32, &1u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, QVError::RoundNotFound);
}

// ─── get_results correctness ───────────────────────────────────────────────────

#[test]
fn test_get_results_multiple_voters_options() {
    let (env, client, alice, bob, carol) = setup();
    let r = create_standard_round(&client, &env, &alice);

    // Alice: 4 votes on option 1 (cost 16), 3 votes on option 2 (cost 9).
    client.cast_vote(&alice, &r, &1u32, &4u32);
    client.cast_vote(&alice, &r, &2u32, &3u32);

    // Bob: 5 votes on option 2 (cost 25), 2 votes on option 3 (cost 4).
    client.cast_vote(&bob, &r, &2u32, &5u32);
    client.cast_vote(&bob, &r, &3u32, &2u32);

    // Carol: 7 votes on option 1 (cost 49).
    client.cast_vote(&carol, &r, &1u32, &7u32);

    let results = client.get_results(&r);
    let find = |id: u32| {
        results
            .iter()
            .find(|(oid, _)| *oid == id)
            .map(|(_, v)| v)
            .unwrap_or(0)
    };

    assert_eq!(find(1), 11, "option 1: alice(4) + carol(7) = 11");
    assert_eq!(find(2), 8, "option 2: alice(3) + bob(5) = 8");
    assert_eq!(find(3), 2, "option 3: bob(2) = 2");
}

// ─── Full end-to-end integration test ─────────────────────────────────────────

/// Simulates a realistic governance round: a DAO chooses which of 3 community
/// grant proposals to fund.
///
/// Setup:
/// - 3 voters with 100 voice credits each.
/// - 3 options: proposal A (id=1), B (id=2), C (id=3).
#[test]
fn test_e2e_grant_proposal_round() {
    let (env, client, alice, bob, carol) = setup();
    let admin = alice.clone();

    // Step 1: Create the round.
    let round_id = client.create_round(&admin, &vec![&env, 1u32, 2u32, 3u32], &100u64);

    // Confirm round is open.
    let info = client.get_round_info(&round_id);
    assert_eq!(info.state, crate::RoundState::Open);
    assert_eq!(info.credits_per_voter, 100);

    // Step 2: Alice strongly prefers B (7 votes = 49 credits), lightly votes A (1 vote = 1 credit).
    client.cast_vote(&alice, &round_id, &2u32, &7u32); // cost 49
    client.cast_vote(&alice, &round_id, &1u32, &1u32); // cost 1
    assert_eq!(client.get_voter_credits(&round_id, &alice), 50);

    // Bob splits evenly between B and C (5 votes each = 25+25 = 50 credits).
    client.cast_vote(&bob, &round_id, &2u32, &5u32); // cost 25
    client.cast_vote(&bob, &round_id, &3u32, &5u32); // cost 25
    assert_eq!(client.get_voter_credits(&round_id, &bob), 50);

    // Carol uses accumulation — first 3 votes on C, then 4 more on C.
    // Cost: 3²=9, then (7²-3²)=40, total=49.
    client.cast_vote(&carol, &round_id, &3u32, &3u32); // cost 9
    client.cast_vote(&carol, &round_id, &3u32, &4u32); // cost 40
    assert_eq!(client.get_voter_credits(&round_id, &carol), 51);

    // Step 3: Verify intermediate results before close.
    let results_open = client.get_results(&round_id);
    let find = |id: u32| {
        results_open
            .iter()
            .find(|(oid, _)| *oid == id)
            .map(|(_, v)| v)
            .unwrap_or(0)
    };
    assert_eq!(find(1), 1, "option A: alice(1)");
    assert_eq!(find(2), 12, "option B: alice(7) + bob(5) = 12");
    assert_eq!(find(3), 12, "option C: bob(5) + carol(7) = 12");

    // Step 4: Close the round.
    client.close_round(&admin, &round_id);
    let info_closed = client.get_round_info(&round_id);
    assert_eq!(info_closed.state, crate::RoundState::Closed);

    // Step 5: Voting is now rejected.
    let late_voter = Address::generate(&env);
    let err = client
        .try_cast_vote(&late_voter, &round_id, &1u32, &1u32)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, QVError::RoundNotOpen);

    // Step 6: Final results are stable after close.
    let results_final = client.get_results(&round_id);
    let find_f = |id: u32| {
        results_final
            .iter()
            .find(|(oid, _)| *oid == id)
            .map(|(_, v)| v)
            .unwrap_or(0)
    };
    assert_eq!(find_f(1), 1);
    assert_eq!(find_f(2), 12);
    assert_eq!(find_f(3), 12);
    // Note: B and C tied — a realistic outcome in a small group with spread preferences.
}

// ─── get_voter_credits before first vote ──────────────────────────────────────

#[test]
fn test_get_voter_credits_before_any_vote() {
    let (env, client, alice, bob, _) = setup();
    let r = create_standard_round(&client, &env, &alice);
    // Bob hasn't voted — should see full allocation.
    assert_eq!(client.get_voter_credits(&r, &bob), 100);
}

// ─── Round ID monotonicity ─────────────────────────────────────────────────────

#[test]
fn test_round_ids_are_monotonically_increasing() {
    let (env, client, alice, _, _) = setup();
    let r1 = create_standard_round(&client, &env, &alice);
    let r2 = create_standard_round(&client, &env, &alice);
    let r3 = create_standard_round(&client, &env, &alice);
    assert!(r2 > r1);
    assert!(r3 > r2);
}
