//! Ingestion-layer golden vectors: per-sub-tree roots from CBOR
//! fixtures, plus the canonical v0.9.0 hybrid bundle root tuple
//! (5 CBOR-derived sub-trees + 2 existing JSON fixtures).
//!
//! These are the canonical ingestion regression net under the v1
//! domain-separated Merkle construction (Batch 1 of the 2026-05-03
//! audit-resolution plan). If any of these drift, encoding or
//! aggregation logic changed — investigate before re-pinning.

use omega_commitment_bundle::bundle::assemble;
use omega_commitment_core::{
    tree::MerkleTree, SUB_TREE_ID_GOVERNANCE, SUB_TREE_ID_SCRIPT, SUB_TREE_ID_STAKE,
    SUB_TREE_ID_TOKEN_POLICY, SUB_TREE_ID_UTXO,
};
use omega_commitment_ingest::{
    governance::ingest_governance, script::ingest_scripts, stake::ingest_stake,
    token_policy::ingest_token_policies, utxo::ingest_utxos,
};
use std::{fs, path::PathBuf};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn extended_cbor() -> Vec<u8> {
    fs::read(manifest().join("tests/fixtures/ledger_state_extended.cbor")).unwrap()
}

fn stake_cbor() -> Vec<u8> {
    fs::read(manifest().join("tests/fixtures/stake_snapshot.cbor")).unwrap()
}

fn governance_cbor() -> Vec<u8> {
    fs::read(manifest().join("tests/fixtures/governance_snapshot.cbor")).unwrap()
}

#[test]
fn golden_utxo_root_from_extended_cbor() {
    let out = ingest_utxos(&extended_cbor()).unwrap();
    let payloads: Vec<Vec<u8>> = out
        .utxos
        .iter()
        .map(|u| u.commit_to_subtree().unwrap())
        .collect();
    let root = MerkleTree::build_v1(SUB_TREE_ID_UTXO, payloads)
        .unwrap()
        .root();
    assert_eq!(
        hex::encode(root),
        // re-pinned 2026-05-03: Batch 2 Cardano semantic fidelity (A2/F002, A3/F001-F005)
        "2ae8632b3947b820e4cd3a91a0d05db89228c7b9d4e53d3958b43c1709e5a578",
        "ingestion-layer UTXO root drifted"
    );
}

#[test]
fn golden_token_policy_root_from_extended_cbor() {
    let out = ingest_token_policies(&extended_cbor()).unwrap();
    let payloads: Vec<Vec<u8>> = out.policies.iter().map(|p| p.commit_to_subtree()).collect();
    let root = MerkleTree::build_v1(SUB_TREE_ID_TOKEN_POLICY, payloads)
        .unwrap()
        .root();
    assert_eq!(
        hex::encode(root),
        // re-pinned 2026-05-03: Batch 1 crypto soundness (A1/F001-F005)
        "1aecac9810c93107e4a932f76eb1eee3b625f2c8b9355ff4f20a712c3a2a51ff",
        "ingestion-layer token-policy root drifted"
    );
}

#[test]
fn golden_script_root_from_extended_cbor() {
    let out = ingest_scripts(&extended_cbor()).unwrap();
    let payloads: Vec<Vec<u8>> = out.scripts.iter().map(|s| s.commit_to_subtree()).collect();
    let root = MerkleTree::build_v1(SUB_TREE_ID_SCRIPT, payloads)
        .unwrap()
        .root();
    assert_eq!(
        hex::encode(root),
        // re-pinned 2026-05-03: Batch 1 crypto soundness (A1/F001-F005)
        "8eed66a730ed1147e6c022e03b20bd9a1114ed3ccea11b8078c2cbdb7ed7ec31",
        "ingestion-layer script root drifted"
    );
}

#[test]
fn golden_stake_root_from_cbor() {
    let out = ingest_stake(&stake_cbor()).unwrap();
    let payloads: Vec<Vec<u8>> = out
        .stake_entries
        .iter()
        .map(|s| s.commit_to_subtree())
        .collect();
    let root = MerkleTree::build_v1(SUB_TREE_ID_STAKE, payloads)
        .unwrap()
        .root();
    assert_eq!(
        hex::encode(root),
        // re-pinned 2026-05-03: Batch 2 Cardano semantic fidelity (A2/F002, A3/F001-F005)
        "65be9aa4799346da0dc8cd04c142a1282007fc935ee90396ad4201a1970ac004",
        "ingestion-layer stake root drifted"
    );
}

#[test]
fn golden_governance_root_from_cbor() {
    let out = ingest_governance(&governance_cbor()).unwrap();
    let payloads: Vec<Vec<u8>> = out.facts.iter().map(|f| f.commit_to_subtree()).collect();
    let root = MerkleTree::build_v1(SUB_TREE_ID_GOVERNANCE, payloads)
        .unwrap()
        .root();
    assert_eq!(
        hex::encode(root),
        // re-pinned 2026-05-03: Batch 2 Cardano semantic fidelity (A2/F002, A3/F001-F005)
        "c34315048667ee3510f415e1438d0a549f04d48c9dee26ed933f2c3bd66725d8",
        "ingestion-layer governance root drifted"
    );
}

/// Run all 5 ingestion paths, write per-sub-tree JSON to a tempdir,
/// copy the 2 existing JSON fixtures (header, tx_index) over, then
/// run `omega-commitment-bundle::assemble` and pin the resulting
/// dual-track bundle root tuple.
#[test]
fn golden_hybrid_bundle_roots() {
    let dir = tempfile::tempdir().unwrap();

    // 5 sub-trees from CBOR ingestion.
    let utxo = ingest_utxos(&extended_cbor()).unwrap();
    fs::write(
        dir.path().join("utxo.json"),
        serde_json::to_string(&utxo).unwrap(),
    )
    .unwrap();
    let tp = ingest_token_policies(&extended_cbor()).unwrap();
    fs::write(
        dir.path().join("token_policy.json"),
        serde_json::to_string(&tp).unwrap(),
    )
    .unwrap();
    let s = ingest_scripts(&extended_cbor()).unwrap();
    fs::write(
        dir.path().join("script.json"),
        serde_json::to_string(&s).unwrap(),
    )
    .unwrap();
    let st = ingest_stake(&stake_cbor()).unwrap();
    fs::write(
        dir.path().join("stake.json"),
        serde_json::to_string(&st).unwrap(),
    )
    .unwrap();
    let g = ingest_governance(&governance_cbor()).unwrap();
    fs::write(
        dir.path().join("governance.json"),
        serde_json::to_string(&g).unwrap(),
    )
    .unwrap();

    // 2 sub-trees from existing JSON fixtures (header + tx-index).
    let core_fixtures = manifest()
        .parent()
        .unwrap()
        .join("omega-commitment-core/tests/fixtures");
    fs::copy(
        core_fixtures.join("header_chain_small.json"),
        dir.path().join("header.json"),
    )
    .unwrap();
    fs::copy(
        core_fixtures.join("tx_index_small.json"),
        dir.path().join("tx_index.json"),
    )
    .unwrap();

    let bundle = assemble(dir.path()).unwrap();
    assert_eq!(
        hex::encode(bundle.blake2b_bundle_root),
        // re-pinned 2026-05-03: Batch 2 Cardano semantic fidelity (A2/F002, A3/F001-F005)
        "a035704932850d18ed8293b036387d9c3b14fc7ff047fe3eb88fee490b2b8e24",
        "hybrid blake2b_bundle_root drifted from Batch 2 v1 pin"
    );
    assert_eq!(
        hex::encode(bundle.sha3_bundle_root),
        // re-pinned 2026-05-03: Batch 2 Cardano semantic fidelity (A2/F002, A3/F001-F005)
        "b2f739a441541eb9cb7dc543dd71c5ffd531ba34003744b1ac2274885945de7b",
        "hybrid sha3_bundle_root drifted from Batch 2 v1 pin"
    );
}
