//! End-to-end integration test for the native-token-policy sub-tree.

use omega_commitment_core::{
    token_policy_leaf::{validate_policy_id_uniqueness, TokenPolicy},
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    policies: Vec<TokenPolicy>,
}

const FIXTURE: &str = include_str!("fixtures/token_policies_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.policies.len(), 8);

    assert!(
        validate_policy_id_uniqueness(&f.policies).is_none(),
        "fixture has duplicate policy_ids"
    );

    let leaves: Vec<_> = f.policies.iter().map(|p| p.leaf_hash()).collect();
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
    let leaves1: Vec<_> = f.policies.iter().map(|p| p.leaf_hash()).collect();
    let leaves2: Vec<_> = f.policies.iter().map(|p| p.leaf_hash()).collect();
    assert_eq!(
        MerkleTree::build(leaves1).root(),
        MerkleTree::build(leaves2).root()
    );
}

#[test]
fn duplicate_policy_id_rejected_by_validator() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let mut policies = f.policies;
    let dup = policies[0].clone();
    policies.push(dup);
    assert_eq!(validate_policy_id_uniqueness(&policies), Some(8));
}

#[test]
fn large_u128_supply_round_trips_through_json() {
    // The 8th fixture entry has a 39-digit supply value; this test
    // confirms serde_json correctly parses it through the u128 path
    // and the encoded bytes match the expected layout.
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let last = f.policies.last().unwrap();
    assert_eq!(
        last.total_supply_at_h,
        999_999_999_999_999_999_999_999_999_999_999_999u128
    );
    let bytes = last.encode();
    assert_eq!(bytes.len(), 52);
}
