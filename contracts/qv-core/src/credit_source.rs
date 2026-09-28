//! Voice credit sourcing abstraction.
//!
//! ## Current implementation
//! The contract uses a **fixed per-round allocation**: every voter receives the
//! same number of voice credits when they first interact with a round.  This is
//! configured at round creation via `credits_per_voter`.
//!
//! ## Future extension (v2 roadmap)
//! Token-weighted allocation — where each voter's credit budget is proportional
//! to their token balance at a snapshot block — can be added by implementing
//! the [`CreditSource`] trait below and wiring it into `cast_vote`.
//!
//! The trait is defined here as documentation and as a compile-time interface
//! contract; it is **not** called by the core contract in v0.1.  Integrators
//! who need token-weighted credits should fork this crate, implement the trait,
//! and replace the fixed-allocation logic in `cast_vote`.

use soroban_sdk::{Address, Env};

/// Interface for pluggable voice-credit sourcing strategies.
///
/// Implement this to supply token-weighted, reputation-weighted, or any other
/// allocation strategy.  The core contract currently uses a fixed allocation
/// directly, but this trait describes the interface a v2 implementation would
/// satisfy.
///
/// # Note on Soroban trait constraints
/// Soroban's `#[contractimpl]` does not support Rust trait objects at runtime
/// (no vtable dispatch inside a WASM contract).  In practice, a v2 implementation
/// would be a concrete type that implements this trait, and `cast_vote` would call
/// it directly.  The trait here serves as an *interface specification* for
/// integrators, not a runtime dispatch mechanism.
pub trait CreditSource {
    /// Return the number of voice credits to allocate to `voter` for `round_id`.
    ///
    /// Called exactly once per voter per round, on their first `cast_vote` call.
    /// Subsequent calls within the same round must use the stored remaining balance.
    fn credits_for_voter(env: &Env, round_id: u64, voter: &Address) -> u64;
}

/// Fixed allocation strategy: every voter receives the same credit budget.
///
/// This is the strategy used by the core contract in v0.1.
pub struct FixedAllocation;

impl CreditSource for FixedAllocation {
    fn credits_for_voter(_env: &Env, _round_id: u64, _voter: &Address) -> u64 {
        // The actual value comes from `RoundInfo::credits_per_voter`, stored at
        // round creation.  This stub exists to document the interface contract.
        unimplemented!(
            "FixedAllocation reads from RoundInfo::credits_per_voter directly in cast_vote"
        )
    }
}
