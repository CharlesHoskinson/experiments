//! Ingestion-layer golden vectors: per-sub-tree roots from CBOR
//! fixtures, plus the canonical v0.9.0 hybrid bundle root tuple
//! (5 CBOR-derived sub-trees + 2 existing JSON fixtures).
//!
//! These are the canonical ingestion regression net. If any of these
//! drift, encoding or aggregation logic changed — investigate before
//! re-pinning.

use omega_commitment_bundle::bundle::assemble;
use omega_commitment_core::tree::MerkleTree;
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
    let leaves: Vec<_> = out.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "3db453610cddde4f799a7bd5e5757fe7b66c71510c2f55d10d1a8c577b94f6f7",
        "ingestion-layer UTXO root drifted"
    );
}

#[test]
fn golden_token_policy_root_from_extended_cbor() {
    let out = ingest_token_policies(&extended_cbor()).unwrap();
    let leaves: Vec<_> = out.policies.iter().map(|p| p.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "2b093effe91ecb6d1dbae52e566914e629dd37bc3e1f76457087232790593157",
        "ingestion-layer token-policy root drifted"
    );
}

#[test]
fn golden_script_root_from_extended_cbor() {
    let out = ingest_scripts(&extended_cbor()).unwrap();
    let leaves: Vec<_> = out.scripts.iter().map(|s| s.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "d4362524462727386a3f6892e1cc07b813b97ad2e8b19d56c0c31e4c703df381",
        "ingestion-layer script root drifted"
    );
}

#[test]
fn golden_stake_root_from_cbor() {
    let out = ingest_stake(&stake_cbor()).unwrap();
    let leaves: Vec<_> = out.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "56d68a45319ec728ff99d8510f02d20c17c6d88335caf9f93fedeb4502997f85",
        "ingestion-layer stake root drifted"
    );
}

#[test]
fn golden_governance_root_from_cbor() {
    let out = ingest_governance(&governance_cbor()).unwrap();
    let leaves: Vec<_> = out.facts.iter().map(|f| f.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "bee53b24965867c9fb877eccb925695d65cf15485c8000cb08ee64218700317d",
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
        "18d6a6a299849d0c832f5f3094037099f2ad7997f05b1a471bb49b9cbb714a2c",
        "v0.9.0 hybrid blake2b_bundle_root drifted"
    );
    assert_eq!(
        hex::encode(bundle.sha3_bundle_root),
        "7831f4008d79f9211c89424d5c0ddfb16438b0a9f2c6a45c30d623a7dae2b3e3",
        "v0.9.0 hybrid sha3_bundle_root drifted"
    );
}
