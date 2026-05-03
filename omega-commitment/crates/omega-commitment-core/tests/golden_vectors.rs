//! Golden vectors for per-sub-tree canonical roots against the seven
//! shipped synthetic fixtures.
//!
//! These hashes are pinned constants. If a code change causes any of
//! them to drift, the test fails — and that's the point. A failure
//! here means either:
//!   - a bug was introduced in encoding logic (revert), or
//!   - encoding logic was deliberately changed (regenerate vectors as
//!     a SemVer-major change with a recorded decision).

use omega_commitment_core::{
    governance_state_leaf::GovernanceFact, header_leaf::BlockHeader,
    script_registry_leaf::ScriptEntry, stake_state_leaf::StakeEntry,
    token_policy_leaf::TokenPolicy, tree::MerkleTree, tx_index_leaf::TxIndexEntry, utxo_leaf::Utxo,
};
use serde::Deserialize;

const FIXTURES: &str = "tests/fixtures";

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURES}/{name}")).unwrap()
}

#[derive(Deserialize)]
struct UtxoIn {
    utxos: Vec<Utxo>,
}
#[derive(Deserialize)]
struct HeaderIn {
    headers: Vec<BlockHeader>,
}
#[derive(Deserialize)]
struct TxIn {
    entries: Vec<TxIndexEntry>,
}
#[derive(Deserialize)]
struct PolIn {
    policies: Vec<TokenPolicy>,
}
#[derive(Deserialize)]
struct ScriptIn {
    scripts: Vec<ScriptEntry>,
}
#[derive(Deserialize)]
struct StakeIn {
    stake_entries: Vec<StakeEntry>,
}
#[derive(Deserialize)]
struct GovIn {
    facts: Vec<GovernanceFact>,
}

#[test]
fn golden_utxo_root() {
    let f: UtxoIn = serde_json::from_str(&read_fixture("utxo_set_small.json")).unwrap();
    let leaves: Vec<_> = f.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    let root = MerkleTree::build(leaves).root();
    // GOLDEN: regenerate via Step 1 if encoding semantics change.
    assert_eq!(
        hex::encode(root),
        "74be699a17928cfae6a9301b96e033c5b75ccc841b2eeb4d3e9ab4484694c044",
        "UTXO sub-tree root drifted"
    );
}

#[test]
fn golden_header_root() {
    let f: HeaderIn = serde_json::from_str(&read_fixture("header_chain_small.json")).unwrap();
    let leaves: Vec<_> = f.headers.iter().map(|h| h.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "ed2eaedffc3833afbe0d7727f66c1b824bec77139f9e0a965b81e30dd349f1de",
        "Header sub-tree root drifted"
    );
}

#[test]
fn golden_tx_index_root() {
    let f: TxIn = serde_json::from_str(&read_fixture("tx_index_small.json")).unwrap();
    let leaves: Vec<_> = f.entries.iter().map(|e| e.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "76fc602782a80bb5e425bf22d32cdcf0ababa46e9129c76d470b990fb62fe6c1",
        "Tx-index sub-tree root drifted"
    );
}

#[test]
fn golden_token_policy_root() {
    let f: PolIn = serde_json::from_str(&read_fixture("token_policies_small.json")).unwrap();
    let leaves: Vec<_> = f.policies.iter().map(|p| p.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "c8d27987a53df992eebc37a6b1ad4549009cf5916618d869161b7e659a3a3c2a",
        "Token-policy sub-tree root drifted"
    );
}

#[test]
fn golden_script_root() {
    let f: ScriptIn = serde_json::from_str(&read_fixture("script_registry_small.json")).unwrap();
    let leaves: Vec<_> = f.scripts.iter().map(|s| s.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "92cc8f368cf40d6d00ab1524d1d5715786f563b2cfb8756dc29ffab41fd74bab",
        "Script-registry sub-tree root drifted"
    );
}

#[test]
fn golden_stake_root() {
    let f: StakeIn = serde_json::from_str(&read_fixture("stake_state_small.json")).unwrap();
    let leaves: Vec<_> = f.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "b903889b884b4e33dfd3a2c7c3736cd16100cdcd0328d91874cfab473e196322",
        "Stake-state sub-tree root drifted"
    );
}

#[test]
fn golden_governance_root() {
    let f: GovIn = serde_json::from_str(&read_fixture("governance_state_small.json")).unwrap();
    let leaves: Vec<_> = f.facts.iter().map(|fact| fact.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "cee7d743ecd1367142aab991e55e67f0a40835eecc0661ccec6f9617b99734b4",
        "Governance-state sub-tree root drifted"
    );
}

#[test]
fn golden_utxo_witness_round_trip() {
    use omega_commitment_core::witness::InclusionWitness;

    let f: UtxoIn = serde_json::from_str(&read_fixture("utxo_set_small.json")).unwrap();
    let leaves: Vec<_> = f.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    let tree = MerkleTree::build(leaves.clone());
    let root = tree.root();

    // For each leaf, build a witness and verify it.
    for leaf in &leaves {
        let w = InclusionWitness::build(&tree, *leaf).expect("leaf in tree");
        assert!(
            w.verify(root),
            "witness must verify against pinned golden root"
        );
    }

    // Serialize the first leaf's witness; confirm it round-trips.
    let w = InclusionWitness::build(&tree, leaves[0]).unwrap();
    let json = serde_json::to_string(&w).unwrap();
    let w2: InclusionWitness = serde_json::from_str(&json).unwrap();
    assert_eq!(w, w2, "witness JSON round-trip diverged");

    // Witness shape: sibling count must equal tree depth.
    assert_eq!(
        w.siblings.len() as u32,
        tree.depth() as u32,
        "witness siblings count != tree depth"
    );
}
