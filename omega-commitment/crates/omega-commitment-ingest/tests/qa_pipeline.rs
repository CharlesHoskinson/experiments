//! End-to-end QA pipeline: hand-crafted CBOR → omega-ingest → JSON →
//! omega-commitment commit (in-process) → leaf hashes + root.
//!
//! Proves the entire ingestion → commitment pipeline works on a
//! CBOR-shaped input. Validates that the CBOR fixture, the ingestion
//! library, and the commitment-core library all agree.

use omega_commitment_core::tree::MerkleTree;
use omega_commitment_ingest::utxo::ingest_utxos;
use std::{fs, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ledger_state_minimal.cbor")
}

#[test]
fn full_pipeline_cbor_to_root() {
    // 1) Read CBOR fixture.
    let cbor = fs::read(fixture_path()).unwrap();

    // 2) Ingest into the per-sub-tree JSON shape.
    let out = ingest_utxos(&cbor).unwrap();
    assert_eq!(out.utxos.len(), 3);

    // 3) Compute leaf hashes (what `omega-commitment commit` would do).
    let leaves: Vec<_> = out.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();

    // 4) Build the Merkle tree.
    let tree = MerkleTree::build(leaves.clone());
    let root = tree.root();
    assert_ne!(root, [0u8; 32]);

    // 5) Round-trip via JSON: write to a temp dir, then parse back, then verify
    //    the round-tripped UTXOs hash to the same leaves.
    use omega_commitment_core::utxo_leaf::Utxo;
    use serde::Deserialize;
    #[derive(Deserialize)]
    struct Roundtrip {
        utxos: Vec<Utxo>,
    }
    let json = serde_json::to_string(&out).unwrap();
    let parsed: Roundtrip = serde_json::from_str(&json).unwrap();
    let leaves_after: Vec<_> = parsed
        .utxos
        .iter()
        .map(|u| u.leaf_hash().unwrap())
        .collect();
    assert_eq!(
        leaves, leaves_after,
        "JSON round-trip must preserve leaf hashes"
    );

    // 6) Tree built from the round-tripped UTXOs has the same root.
    let tree_after = MerkleTree::build(leaves_after);
    assert_eq!(tree.root(), tree_after.root());
}

#[test]
fn full_pipeline_five_sub_trees_from_cbor() {
    use omega_commitment_ingest::{
        governance::ingest_governance, script::ingest_scripts, stake::ingest_stake,
        token_policy::ingest_token_policies,
    };

    let extended_cbor = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ledger_state_extended.cbor"),
    )
    .unwrap();
    let stake_cbor = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stake_snapshot.cbor"),
    )
    .unwrap();
    let gov_cbor = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/governance_snapshot.cbor"),
    )
    .unwrap();

    // 1) UTXO sub-tree from extended fixture.
    let utxo_out = omega_commitment_ingest::utxo::ingest_utxos(&extended_cbor).unwrap();
    assert_eq!(utxo_out.utxos.len(), 4);
    let utxo_leaves: Vec<_> = utxo_out
        .utxos
        .iter()
        .map(|u| u.leaf_hash().unwrap())
        .collect();
    let utxo_root = omega_commitment_core::tree::MerkleTree::build(utxo_leaves).root();
    assert_ne!(utxo_root, [0u8; 32]);

    // 2) Token-policy sub-tree derived from extended fixture.
    let tp_out = ingest_token_policies(&extended_cbor).unwrap();
    assert_eq!(tp_out.policies.len(), 2);
    let tp_leaves: Vec<_> = tp_out.policies.iter().map(|p| p.leaf_hash()).collect();
    let tp_root = omega_commitment_core::tree::MerkleTree::build(tp_leaves).root();
    assert_ne!(tp_root, [0u8; 32]);

    // 3) Script sub-tree derived from extended fixture.
    let s_out = ingest_scripts(&extended_cbor).unwrap();
    assert_eq!(s_out.scripts.len(), 2);
    let s_leaves: Vec<_> = s_out.scripts.iter().map(|s| s.leaf_hash()).collect();
    let s_root = omega_commitment_core::tree::MerkleTree::build(s_leaves).root();
    assert_ne!(s_root, [0u8; 32]);

    // 4) Stake sub-tree from stake_snapshot.cbor.
    let st_out = ingest_stake(&stake_cbor).unwrap();
    assert_eq!(st_out.stake_entries.len(), 4);
    let st_leaves: Vec<_> = st_out.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    let st_root = omega_commitment_core::tree::MerkleTree::build(st_leaves).root();
    assert_ne!(st_root, [0u8; 32]);

    // 5) Governance sub-tree from governance_snapshot.cbor.
    let g_out = ingest_governance(&gov_cbor).unwrap();
    assert_eq!(g_out.facts.len(), 5);
    let g_leaves: Vec<_> = g_out.facts.iter().map(|f| f.leaf_hash()).collect();
    let g_root = omega_commitment_core::tree::MerkleTree::build(g_leaves).root();
    assert_ne!(g_root, [0u8; 32]);

    // Sanity: each sub-tree root is distinct.
    let roots = [utxo_root, tp_root, s_root, st_root, g_root];
    for i in 0..roots.len() {
        for j in (i + 1)..roots.len() {
            assert_ne!(
                roots[i], roots[j],
                "sub-tree {} and {} produced same root",
                i, j
            );
        }
    }
}
