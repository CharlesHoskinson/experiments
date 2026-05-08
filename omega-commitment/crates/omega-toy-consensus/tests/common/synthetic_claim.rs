//! Synthetic accepted claims for consensus integration tests.

/// Builds a synthetic claim accepted by the verifier and mock ledger.
#[allow(dead_code)]
pub fn synthetic_accepted_claim_for_leaf(leaf_index: u64) -> omega_claim_tx::ClaimTx {
    omega_claim_prover::test_fixtures::build_synthetic_accepted_claim(leaf_index)
}
