//! UTXO ingestion end-to-end against the hand-crafted CBOR fixture.

use omega_commitment_core::tree::MerkleTree;
use omega_commitment_ingest::utxo::ingest_utxos;
use std::{fs, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ledger_state_minimal.cbor")
}

#[test]
fn ingest_minimal_cbor_fixture() {
    let cbor = fs::read(fixture_path()).expect("fixture readable");
    let out = ingest_utxos(&cbor).unwrap();
    assert_eq!(out.utxos.len(), 3);
    assert_eq!(out.utxos[0].tx_id, [0x11; 32]);
    assert_eq!(out.utxos[1].tx_id, [0x22; 32]);
    assert_eq!(out.utxos[2].tx_id, [0x33; 32]);
}

#[test]
fn ingest_then_build_tree_succeeds() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_utxos(&cbor).unwrap();
    let leaves: Vec<_> = out.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    let tree = MerkleTree::build(leaves);
    // 3 utxos pads to 4 leaves at depth 2.
    assert_eq!(tree.leaf_count(), 4);
    assert_eq!(tree.depth(), 2);
    assert_ne!(tree.root(), [0u8; 32]);
}

#[test]
fn ingest_to_json_matches_per_sub_tree_cli_format() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_utxos(&cbor).unwrap();
    let json = serde_json::to_string(&out).unwrap();
    // The JSON shape must match what `omega-commitment commit --sub-tree utxo`
    // accepts (namely `{"utxos": [...]}`).
    assert!(
        json.contains("\"utxos\":"),
        "JSON should have utxos field: {json}"
    );
    // Round-trip back through serde to confirm the format is stable.
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["utxos"].is_array());
}
