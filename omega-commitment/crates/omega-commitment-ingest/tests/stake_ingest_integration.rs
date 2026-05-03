//! Integration test for stake ingestion against stake_snapshot.cbor.

use omega_commitment_core::tree::MerkleTree;
use omega_commitment_ingest::stake::ingest_stake;
use std::{fs, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stake_snapshot.cbor")
}

#[test]
fn stake_fixture_yields_four_entries_via_integration() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_stake(&cbor).unwrap();
    assert_eq!(out.stake_entries.len(), 4);
}

#[test]
fn stake_root_is_nonzero() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_stake(&cbor).unwrap();
    let leaves: Vec<_> = out.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_ne!(root, [0u8; 32]);
}
