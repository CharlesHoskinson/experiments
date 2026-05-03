//! End-to-end integration test for the governance-state sub-tree.

use omega_commitment_core::{
    governance_state_leaf::{validate_governance_keys_unique_per_kind, GovernanceFact},
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    facts: Vec<GovernanceFact>,
}

const FIXTURE: &str = include_str!("fixtures/governance_state_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.facts.len(), 8);

    assert!(
        validate_governance_keys_unique_per_kind(&f.facts).is_none(),
        "fixture has duplicate (kind, key) pairs"
    );

    let leaves: Vec<_> = f.facts.iter().map(|fact| fact.leaf_hash()).collect();
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
    let leaves1: Vec<_> = f.facts.iter().map(|fact| fact.leaf_hash()).collect();
    let leaves2: Vec<_> = f.facts.iter().map(|fact| fact.leaf_hash()).collect();
    assert_eq!(
        MerkleTree::build(leaves1).root(),
        MerkleTree::build(leaves2).root()
    );
}

#[test]
fn all_four_kinds_present_in_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let kinds: std::collections::HashSet<u8> = f.facts.iter().map(|x| x.kind).collect();
    assert_eq!(kinds.len(), 4, "expected all 4 kinds");
    for expected in 0..=3u8 {
        assert!(kinds.contains(&expected), "missing kind={expected}");
    }
}

#[test]
fn large_u128_value_round_trips() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let last = f.facts.last().unwrap();
    assert_eq!(last.value, u128::MAX - 1);
}

#[test]
fn duplicate_kind_key_pair_rejected_by_validator() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let mut facts = f.facts;
    let dup = facts[0].clone();
    facts.push(dup);
    assert_eq!(validate_governance_keys_unique_per_kind(&facts), Some(8));
}
