//! Integration test for script ingestion against the extended fixture.

use omega_commitment_core::tree::MerkleTree;
use omega_commitment_ingest::script::ingest_scripts;
use std::{fs, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ledger_state_extended.cbor")
}

#[test]
fn extended_fixture_yields_two_scripts_via_integration() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_scripts(&cbor).unwrap();
    assert_eq!(out.scripts.len(), 2);
}

#[test]
fn script_root_is_nonzero() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_scripts(&cbor).unwrap();
    let leaves: Vec<_> = out.scripts.iter().map(|s| s.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_ne!(root, [0u8; 32]);
}
