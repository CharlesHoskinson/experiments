//! End-to-end integration test for the script-registry sub-tree.

use omega_commitment_core::{
    script_registry_leaf::{validate_script_hash_uniqueness, ScriptEntry},
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    scripts: Vec<ScriptEntry>,
}

const FIXTURE: &str = include_str!("fixtures/script_registry_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.scripts.len(), 8);

    assert!(
        validate_script_hash_uniqueness(&f.scripts).is_none(),
        "fixture has duplicate script_hashes"
    );

    let leaves: Vec<_> = f.scripts.iter().map(|s| s.leaf_hash()).collect();
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
    let leaves1: Vec<_> = f.scripts.iter().map(|s| s.leaf_hash()).collect();
    let leaves2: Vec<_> = f.scripts.iter().map(|s| s.leaf_hash()).collect();
    assert_eq!(
        MerkleTree::build(leaves1).root(),
        MerkleTree::build(leaves2).root()
    );
}

#[test]
fn all_four_languages_present_in_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let langs: std::collections::HashSet<u8> = f.scripts.iter().map(|s| s.language).collect();
    assert_eq!(langs.len(), 4, "expected all 4 language values");
    for expected in 0..=3u8 {
        assert!(langs.contains(&expected), "missing language={expected}");
    }
}

#[test]
fn duplicate_script_hash_rejected_by_validator() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let mut scripts = f.scripts;
    let dup = scripts[0].clone();
    scripts.push(dup);
    assert_eq!(validate_script_hash_uniqueness(&scripts), Some(8));
}
