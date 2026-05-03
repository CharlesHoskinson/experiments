//! End-to-end integration test for the UTXO sub-tree commitment.

use omega_commitment_core::{tree::MerkleTree, utxo_leaf::Utxo, witness::InclusionWitness};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    utxos: Vec<Utxo>,
}

const FIXTURE: &str = include_str!("fixtures/utxo_set_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.utxos.len(), 3);

    // Encode each UTXO into a leaf hash.
    let leaves: Vec<_> = f.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();

    // Build the tree.
    let tree = MerkleTree::build(leaves.clone());
    assert_eq!(tree.leaf_count(), 4); // padded from 3
    let root = tree.root();
    assert_ne!(root, [0u8; 32]);

    // Witness for every UTXO verifies against the root.
    for leaf in leaves {
        let w = InclusionWitness::build(&tree, leaf).expect("leaf is in tree");
        assert!(w.verify(root), "witness verification failed");
    }
}

#[test]
fn root_is_stable_across_runs() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let leaves1: Vec<_> = f.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    let leaves2: Vec<_> = f.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    assert_eq!(
        MerkleTree::build(leaves1).root(),
        MerkleTree::build(leaves2).root()
    );
}
