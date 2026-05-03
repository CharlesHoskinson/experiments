//! End-to-end integration test for the stake-state sub-tree.

use omega_commitment_core::{
    stake_state_leaf::{validate_stake_credential_uniqueness, StakeEntry},
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    stake_entries: Vec<StakeEntry>,
}

const FIXTURE: &str = include_str!("fixtures/stake_state_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.stake_entries.len(), 8);

    assert!(
        validate_stake_credential_uniqueness(&f.stake_entries).is_none(),
        "fixture has duplicate stake_credential_hashes"
    );

    let leaves: Vec<_> = f.stake_entries.iter().map(|s| s.leaf_hash()).collect();
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
    let leaves1: Vec<_> = f.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    let leaves2: Vec<_> = f.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    assert_eq!(
        MerkleTree::build(leaves1).root(),
        MerkleTree::build(leaves2).root()
    );
}

#[test]
fn pool_operator_flag_changes_leaf() {
    // Sanity: verify a pool-operator entry hashes differently than the
    // same entry with the flag flipped (consistency with leaf-encoding tests).
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let op_entries: Vec<_> = f
        .stake_entries
        .iter()
        .filter(|s| s.is_pool_operator == 1)
        .collect();
    assert!(
        !op_entries.is_empty(),
        "fixture should include pool operators"
    );
    for entry in op_entries {
        let mut flipped = entry.clone();
        flipped.is_pool_operator = 0;
        assert_ne!(entry.leaf_hash(), flipped.leaf_hash());
    }
}

#[test]
fn duplicate_stake_credential_rejected_by_validator() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let mut entries = f.stake_entries;
    let dup = entries[0].clone();
    entries.push(dup);
    assert_eq!(validate_stake_credential_uniqueness(&entries), Some(8));
}
