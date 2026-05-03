//! Golden vectors for per-sub-tree canonical roots against the seven
//! shipped synthetic fixtures.
//!
//! These hashes are pinned constants under the v1 domain-separated
//! Merkle construction (`MerkleTree::build_v1` with the
//! `omega:v2:leaf` / `omega:v2:node` tags). If a code change causes
//! any of them to drift, the test fails — and that's the point. A
//! failure here means either:
//!   - a bug was introduced in encoding logic (revert), or
//!   - encoding logic was deliberately changed (regenerate vectors as
//!     a SemVer-major change with a recorded decision).
//!
//! The values were re-pinned 2026-05-03 as part of Batch 1 of the
//! audit-resolution plan (A1/F001-F005, A4/F001, A7/F001-F002).

use omega_commitment_core::{
    governance_state_leaf::GovernanceFact, header_leaf::BlockHeader,
    script_registry_leaf::ScriptEntry, stake_state_leaf::StakeEntry,
    token_policy_leaf::TokenPolicy, tree::MerkleTree, tx_index_leaf::TxIndexEntry, utxo_leaf::Utxo,
    SUB_TREE_ID_GOVERNANCE, SUB_TREE_ID_HEADER, SUB_TREE_ID_SCRIPT, SUB_TREE_ID_STAKE,
    SUB_TREE_ID_TOKEN_POLICY, SUB_TREE_ID_TX_INDEX, SUB_TREE_ID_UTXO,
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
    let payloads: Vec<Vec<u8>> = f
        .utxos
        .iter()
        .map(|u| u.commit_to_subtree().unwrap())
        .collect();
    let root = MerkleTree::build_v1(SUB_TREE_ID_UTXO, payloads)
        .unwrap()
        .root();
    // re-pinned 2026-05-03: Batch 2 Cardano semantic fidelity (A2/F002, A3/F001-F005)
    assert_eq!(
        hex::encode(root),
        "8cb453aac940d54c315a29f8770fe3d1b82d9092b5215c16473b0fee63595244",
        "UTXO sub-tree root drifted"
    );
}

#[test]
fn golden_header_root() {
    let f: HeaderIn = serde_json::from_str(&read_fixture("header_chain_small.json")).unwrap();
    let payloads: Vec<Vec<u8>> = f.headers.iter().map(|h| h.commit_to_subtree()).collect();
    let root = MerkleTree::build_v1(SUB_TREE_ID_HEADER, payloads)
        .unwrap()
        .root();
    // re-pinned 2026-05-03: Batch 1 crypto soundness (A1/F001-F005)
    assert_eq!(
        hex::encode(root),
        "35a1da1aee850d823799c757e5636f1403202d4656239d9b270c698a4e604879",
        "Header sub-tree root drifted"
    );
}

#[test]
fn golden_tx_index_root() {
    let f: TxIn = serde_json::from_str(&read_fixture("tx_index_small.json")).unwrap();
    let payloads: Vec<Vec<u8>> = f.entries.iter().map(|e| e.commit_to_subtree()).collect();
    let root = MerkleTree::build_v1(SUB_TREE_ID_TX_INDEX, payloads)
        .unwrap()
        .root();
    // re-pinned 2026-05-03: Batch 1 crypto soundness (A1/F001-F005)
    assert_eq!(
        hex::encode(root),
        "ac65e33029d4ed81cd6ddf5cd401fd66c9366e2c5d702d044cc92b7cef5cc7cd",
        "Tx-index sub-tree root drifted"
    );
}

#[test]
fn golden_token_policy_root() {
    let f: PolIn = serde_json::from_str(&read_fixture("token_policies_small.json")).unwrap();
    let payloads: Vec<Vec<u8>> = f.policies.iter().map(|p| p.commit_to_subtree()).collect();
    let root = MerkleTree::build_v1(SUB_TREE_ID_TOKEN_POLICY, payloads)
        .unwrap()
        .root();
    // re-pinned 2026-05-03: Batch 1 crypto soundness (A1/F001-F005)
    assert_eq!(
        hex::encode(root),
        "4d85aab3ce95d7d98e6d71e89eea90f5ff65944b5394b8aa191454db45fcc6a5",
        "Token-policy sub-tree root drifted"
    );
}

#[test]
fn golden_script_root() {
    let f: ScriptIn = serde_json::from_str(&read_fixture("script_registry_small.json")).unwrap();
    let payloads: Vec<Vec<u8>> = f.scripts.iter().map(|s| s.commit_to_subtree()).collect();
    let root = MerkleTree::build_v1(SUB_TREE_ID_SCRIPT, payloads)
        .unwrap()
        .root();
    // re-pinned 2026-05-03: Batch 1 crypto soundness (A1/F001-F005)
    assert_eq!(
        hex::encode(root),
        "ef313c61da9a406b22ac4e7afd70d701ce853498503bd1755eafca0581f9369f",
        "Script-registry sub-tree root drifted"
    );
}

#[test]
fn golden_stake_root() {
    let f: StakeIn = serde_json::from_str(&read_fixture("stake_state_small.json")).unwrap();
    let payloads: Vec<Vec<u8>> = f
        .stake_entries
        .iter()
        .map(|s| s.commit_to_subtree())
        .collect();
    let root = MerkleTree::build_v1(SUB_TREE_ID_STAKE, payloads)
        .unwrap()
        .root();
    // re-pinned 2026-05-03: Batch 2 Cardano semantic fidelity (A2/F002, A3/F001-F005)
    assert_eq!(
        hex::encode(root),
        "80cb39bd34a9c2e7adc79cc17124223b34b901cf2ee2ed68a86f5a0acdecb044",
        "Stake-state sub-tree root drifted"
    );
}

#[test]
fn golden_governance_root() {
    let f: GovIn = serde_json::from_str(&read_fixture("governance_state_small.json")).unwrap();
    let payloads: Vec<Vec<u8>> = f
        .facts
        .iter()
        .map(|fact| fact.commit_to_subtree())
        .collect();
    let root = MerkleTree::build_v1(SUB_TREE_ID_GOVERNANCE, payloads)
        .unwrap()
        .root();
    // re-pinned 2026-05-03: Batch 2 Cardano semantic fidelity (A2/F002, A3/F001-F005)
    assert_eq!(
        hex::encode(root),
        "40d345ddc569a6009825e9517696cfd5c6b8c532c562d61113446f53cf3bb1cd",
        "Governance-state sub-tree root drifted"
    );
}

#[test]
fn golden_utxo_witness_round_trip() {
    // Witnesses are still built against the legacy MerkleTree::build
    // path because the v1 inclusion-witness verifier is part of the
    // v1.0 verifier-circuit work (track T6) and is not yet shipped.
    // Once `witness::InclusionWitness::verify` is migrated to use
    // `node_hash_v2`, this test should switch to the v1 builder.
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
