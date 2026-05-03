//! End-to-end integration test for the transaction-index sub-tree.

use omega_commitment_core::{
    tree::MerkleTree,
    tx_index_leaf::{validate_tx_uniqueness, TxIndexEntry},
    witness::InclusionWitness,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    entries: Vec<TxIndexEntry>,
}

const FIXTURE: &str = include_str!("fixtures/tx_index_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.entries.len(), 8);

    assert!(
        validate_tx_uniqueness(&f.entries).is_none(),
        "fixture has duplicate tx_ids"
    );

    let leaves: Vec<_> = f.entries.iter().map(|e| e.leaf_hash()).collect();
    let tree = MerkleTree::build(leaves.clone());
    assert_eq!(tree.leaf_count(), 8);
    assert_eq!(tree.depth(), 3);
    let root = tree.root();
    assert_ne!(root, [0u8; 32]);

    for leaf in leaves {
        let w = InclusionWitness::build(&tree, leaf).expect("leaf is in tree");
        assert!(w.verify(root), "witness verification failed");
    }
}

#[test]
fn root_is_stable_across_runs() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let leaves1: Vec<_> = f.entries.iter().map(|e| e.leaf_hash()).collect();
    let leaves2: Vec<_> = f.entries.iter().map(|e| e.leaf_hash()).collect();
    assert_eq!(
        MerkleTree::build(leaves1).root(),
        MerkleTree::build(leaves2).root()
    );
}

#[test]
fn same_block_txs_get_distinct_leaves() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let slot5: Vec<_> = f
        .entries
        .iter()
        .filter(|e| e.slot == 5)
        .map(|e| e.leaf_hash())
        .collect();
    assert_eq!(slot5.len(), 3);
    assert_ne!(slot5[0], slot5[1]);
    assert_ne!(slot5[1], slot5[2]);
    assert_ne!(slot5[0], slot5[2]);
}

#[test]
fn duplicate_tx_id_rejected_by_validator() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let mut entries = f.entries;
    let dup = entries[0].clone();
    entries.push(dup);
    assert_eq!(validate_tx_uniqueness(&entries), Some(8));
}
