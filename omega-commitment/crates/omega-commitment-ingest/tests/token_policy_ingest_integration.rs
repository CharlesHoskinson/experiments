//! Integration test for token-policy ingestion against the extended fixture.

use omega_commitment_core::tree::MerkleTree;
use omega_commitment_ingest::token_policy::ingest_token_policies;
use std::{fs, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ledger_state_extended.cbor")
}

#[test]
fn extended_fixture_yields_two_policies_via_integration() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_token_policies(&cbor).unwrap();
    assert_eq!(out.policies.len(), 2);
}

#[test]
fn token_policy_root_is_nonzero() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_token_policies(&cbor).unwrap();
    let leaves: Vec<_> = out.policies.iter().map(|p| p.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_ne!(root, [0u8; 32]);
}
