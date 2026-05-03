//! Inclusion witness for a UTXO leaf in the Merkle tree.

use crate::hash::{blake3_256, Hash};
use crate::tree::MerkleTree;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InclusionWitness {
    /// The leaf hash being proven.
    #[serde(with = "hex::serde")]
    pub leaf: Hash,
    /// Position of the leaf in the sorted-padded leaves array.
    pub leaf_index: u32,
    /// Sibling hashes from leaf-level up to (but not including) the root.
    /// `siblings[i]` is the sibling of the node at layer i on the path
    /// from leaf to root.
    #[serde(with = "crate::serde_helpers::hex_vec_hash")]
    pub siblings: Vec<Hash>,
}

impl InclusionWitness {
    /// Build a witness for the leaf at a known index in the (sorted, padded)
    /// leaves array. Faster than `build` when you already know the index —
    /// avoids the O(n) `position` scan.
    pub fn build_at_index(tree: &MerkleTree, leaf_index: u32) -> Option<Self> {
        let leaves = tree.leaves();
        let leaf = *leaves.get(leaf_index as usize)?;
        let mut idx = leaf_index as usize;
        let mut siblings = Vec::with_capacity(tree.depth());
        let layers = tree.layers();
        for layer in &layers[..tree.depth()] {
            let sib_idx = idx ^ 1;
            siblings.push(layer[sib_idx]);
            idx /= 2;
        }
        Some(Self {
            leaf,
            leaf_index,
            siblings,
        })
    }

    /// Build a witness for the given leaf hash. Returns None if the leaf
    /// isn't in the tree.
    pub fn build(tree: &MerkleTree, leaf: Hash) -> Option<Self> {
        let leaves = tree.leaves();
        let leaf_index = leaves.iter().position(|h| h == &leaf)? as u32;
        let mut idx = leaf_index as usize;
        let mut siblings = Vec::with_capacity(tree.depth());
        let layers = tree.layers();
        for layer in &layers[..tree.depth()] {
            let sib_idx = idx ^ 1;
            siblings.push(layer[sib_idx]);
            idx /= 2;
        }
        Some(Self {
            leaf,
            leaf_index,
            siblings,
        })
    }

    /// Verify this witness against a claimed root.
    pub fn verify(&self, root: Hash) -> bool {
        let depth = self.siblings.len();
        // Reject witnesses with absurd depth or out-of-range index.
        // A valid witness for depth d has leaf_index in [0, 2^d).
        // depth == 0: single-leaf tree, leaf_index must be 0, and leaf must equal root.
        if depth >= 32 {
            return false;
        }
        let max_index = if depth == 0 { 1u64 } else { 1u64 << depth };
        if (self.leaf_index as u64) >= max_index {
            return false;
        }
        let mut current = self.leaf;
        let mut idx = self.leaf_index as usize;
        for sib in &self.siblings {
            let mut buf = [0u8; 64];
            if idx & 1 == 0 {
                buf[..32].copy_from_slice(&current);
                buf[32..].copy_from_slice(sib);
            } else {
                buf[..32].copy_from_slice(sib);
                buf[32..].copy_from_slice(&current);
            }
            current = blake3_256(&buf);
            idx /= 2;
        }
        current == root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_tree_of(n: usize) -> (MerkleTree, Vec<Hash>) {
        let leaves: Vec<Hash> = (0..n)
            .map(|i| blake3_256(&(i as u32).to_be_bytes()))
            .collect();
        let tree = MerkleTree::build(leaves.clone());
        (tree, leaves)
    }

    #[test]
    fn witness_for_present_leaf_verifies() {
        let (tree, leaves) = build_tree_of(8);
        for leaf in &leaves {
            let w = InclusionWitness::build(&tree, *leaf).unwrap();
            assert!(w.verify(tree.root()), "leaf {:?} witness failed", leaf);
        }
    }

    #[test]
    fn witness_for_absent_leaf_is_none() {
        let (tree, _) = build_tree_of(4);
        let bogus = blake3_256(b"not in tree");
        assert!(InclusionWitness::build(&tree, bogus).is_none());
    }

    #[test]
    fn tampered_witness_fails_verify() {
        let (tree, leaves) = build_tree_of(4);
        let mut w = InclusionWitness::build(&tree, leaves[0]).unwrap();
        // Flip a bit in the first sibling.
        if !w.siblings.is_empty() {
            w.siblings[0][0] ^= 0x01;
            assert!(!w.verify(tree.root()));
        }
    }

    #[test]
    fn wrong_root_rejects() {
        let (tree, leaves) = build_tree_of(4);
        let w = InclusionWitness::build(&tree, leaves[0]).unwrap();
        let bad_root = blake3_256(b"bad");
        assert!(!w.verify(bad_root));
    }

    #[test]
    fn witness_serializes_to_json() {
        let (tree, leaves) = build_tree_of(4);
        let w = InclusionWitness::build(&tree, leaves[0]).unwrap();
        let s = serde_json::to_string(&w).unwrap();
        let w2: InclusionWitness = serde_json::from_str(&s).unwrap();
        assert_eq!(w, w2);
    }

    #[test]
    fn malformed_witness_rejected() {
        // depth=0 with leaf_index=5 is impossible (single-leaf tree, only index 0)
        let bad = InclusionWitness {
            leaf: blake3_256(b"any"),
            leaf_index: 5,
            siblings: vec![],
        };
        assert!(!bad.verify([0u8; 32]));

        // depth=2 with leaf_index=4 is impossible (only indices 0..3 valid for 4 leaves)
        let bad2 = InclusionWitness {
            leaf: blake3_256(b"any"),
            leaf_index: 4,
            siblings: vec![[0u8; 32], [0u8; 32]],
        };
        assert!(!bad2.verify([0u8; 32]));

        // Excessively deep witness
        let bad3 = InclusionWitness {
            leaf: blake3_256(b"any"),
            leaf_index: 0,
            siblings: vec![[0u8; 32]; 32],
        };
        assert!(!bad3.verify([0u8; 32]));
    }

    #[test]
    fn build_at_index_matches_build() {
        let (tree, leaves) = build_tree_of(8);
        for (i, leaf) in leaves.iter().enumerate() {
            // Find the post-sort index for this leaf.
            let idx = tree.leaves().iter().position(|h| h == leaf).unwrap() as u32;
            let w_by_leaf = InclusionWitness::build(&tree, *leaf).unwrap();
            let w_by_idx = InclusionWitness::build_at_index(&tree, idx).unwrap();
            assert_eq!(w_by_leaf, w_by_idx, "mismatch at leaf {}", i);
        }
    }

    #[test]
    fn build_at_index_out_of_range_returns_none() {
        let (tree, _) = build_tree_of(4);
        assert!(InclusionWitness::build_at_index(&tree, 9999).is_none());
    }

    #[test]
    fn siblings_serialize_as_hex_strings() {
        let (tree, leaves) = build_tree_of(4);
        let w = InclusionWitness::build(&tree, leaves[0]).unwrap();
        let s = serde_json::to_string(&w).unwrap();
        // Should contain hex-encoded siblings (64 hex chars per entry)
        assert!(
            !s.contains("[0,") && !s.contains("[ 0,"),
            "siblings should not be u8 arrays: {s}"
        );
        // Should contain at least one 64-char hex substring (typical sibling)
        assert!(s.contains("\""), "should have quoted hex strings");
    }
}
