//! Golden vectors for per-sub-tree canonical roots against the seven
//! shipped synthetic fixtures.
//!
//! These hashes are pinned constants under the v1 domain-separated
//! Merkle construction (`MerkleTree::build_v1` with the
//! `omega:v1:leaf` / `omega:v1:node` tags). If a code change causes
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
    // re-pinned 2026-05-03: Batch 1 crypto soundness (A1/F001-F005)
    assert_eq!(
        hex::encode(root),
        "93141ad316c9ad53b27f8b5e5dad40aea0f8c6fa98522f64f6efb6886d3ee10c",
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
        "59e41dbb590b0dc23106b784f635e521a0d50c207063ec2a36c9de4c9729315e",
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
        "c1804b722ce6b0f8e77a391ae401c239f9eaef56812f2840a10b86fc75b86b39",
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
        "620ed0033e184da339bc51cad03cdb781735106992ffacf74cef1d03b58add27",
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
        "0b74abb2c4e04e79906172e2919dd92370f3cf614d25b1a85ed313791ff758bb",
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
    // re-pinned 2026-05-03: Batch 1 crypto soundness (A1/F001-F005)
    assert_eq!(
        hex::encode(root),
        "1beaf83f7cd4ae62eca2b3d36decee1d5894648587b9bc671f026a14434f522f",
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
    // re-pinned 2026-05-03: Batch 1 crypto soundness (A1/F001-F005)
    assert_eq!(
        hex::encode(root),
        "50f021b3dab7410b770cf648524417469d26435b530fb9ceccc4f50222d6dc8f",
        "Governance-state sub-tree root drifted"
    );
}

#[test]
fn golden_utxo_witness_round_trip() {
    // Witnesses are still built against the legacy MerkleTree::build
    // path because the v1 inclusion-witness verifier is part of the
    // v1.0 verifier-circuit work (track T6) and is not yet shipped.
    // Once `witness::InclusionWitness::verify` is migrated to use
    // `node_hash_v1`, this test should switch to the v1 builder.
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
