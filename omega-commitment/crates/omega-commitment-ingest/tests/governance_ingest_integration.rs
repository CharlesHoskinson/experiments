//! Integration test for governance ingestion against governance_snapshot.cbor.

use omega_commitment_core::tree::MerkleTree;
use omega_commitment_ingest::governance::ingest_governance;
use std::{fs, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/governance_snapshot.cbor")
}

#[test]
fn governance_fixture_yields_four_facts_via_integration() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_governance(&cbor).unwrap();
    assert_eq!(out.facts.len(), 4);
}

#[test]
fn governance_root_is_nonzero() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_governance(&cbor).unwrap();
    let leaves: Vec<_> = out.facts.iter().map(|f| f.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_ne!(root, [0u8; 32]);
}
