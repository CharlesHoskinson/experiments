//! End-to-end integration test for the block header sub-tree commitment.

use omega_commitment_core::{
    header_leaf::{validate_chain_links, BlockHeader},
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    headers: Vec<BlockHeader>,
}

const FIXTURE: &str = include_str!("fixtures/header_chain_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.headers.len(), 8);

    // Sanity check the chain is well-linked (optional helper).
    assert!(
        validate_chain_links(&f.headers).is_none(),
        "fixture chain is not well-linked"
    );

    // Encode each header into a leaf hash.
    let leaves: Vec<_> = f.headers.iter().map(|h| h.leaf_hash()).collect();

    // Build the tree.
    let tree = MerkleTree::build(leaves.clone());
    assert_eq!(tree.leaf_count(), 8); // already a power of two
    assert_eq!(tree.depth(), 3);
    let root = tree.root();
    assert_ne!(root, [0u8; 32]);

    // Witness for every header verifies against the root.
    for leaf in leaves {
        let w = InclusionWitness::build(&tree, leaf).expect("leaf is in tree");
        assert!(w.verify(root), "witness verification failed");
    }
}

#[test]
fn root_is_stable_across_runs() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let leaves1: Vec<_> = f.headers.iter().map(|h| h.leaf_hash()).collect();
    let leaves2: Vec<_> = f.headers.iter().map(|h| h.leaf_hash()).collect();
    assert_eq!(
        MerkleTree::build(leaves1).root(),
        MerkleTree::build(leaves2).root()
    );
}

#[test]
fn header_witness_independent_of_chain_validity() {
    // Even if the chain is malformed, the commitment is still well-defined
    // (commitment generation does NOT depend on chain validity).
    let mut headers: Vec<BlockHeader> = serde_json::from_str::<Fixture>(FIXTURE).unwrap().headers;
    // Tamper with the last header's prev_hash.
    headers.last_mut().unwrap().prev_hash = [0xFFu8; 32];
    assert!(validate_chain_links(&headers).is_some(), "tamper detected");

    let leaves: Vec<_> = headers.iter().map(|h| h.leaf_hash()).collect();
    let tree = MerkleTree::build(leaves.clone());
    for leaf in leaves {
        let w = InclusionWitness::build(&tree, leaf).unwrap();
        assert!(w.verify(tree.root()));
    }
}
