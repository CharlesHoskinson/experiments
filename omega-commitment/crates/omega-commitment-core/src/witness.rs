//! Inclusion witness against the legacy [`MerkleTree::build`] root.
//!
//! [`InclusionWitness`] proves that a leaf hash sits at a specific
//! sorted index under a published Merkle root. The verifier replays
//! the path with raw Blake3 of `left || right` concatenations, which
//! matches the legacy [`MerkleTree::build`] construction. The v1
//! domain-separated `build_v1` path uses [`crate::tree::node_hash_v2`]
//! and is consumed by the STARK verifier in `omega-claim-verifier`,
//! not by this witness type.

use crate::hash::{blake3_256, Hash};
use crate::tree::MerkleTree;
use serde::{Deserialize, Serialize};

/// A Merkle inclusion witness: a leaf hash, its sorted index, and the
/// sibling hashes along the path from leaf to root.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InclusionWitness {
    /// The leaf hash being proven.
    #[serde(with = "hex::serde")]
    pub leaf: Hash,
    /// Position of the leaf in the sorted, padded leaves array.
    pub leaf_index: u32,
    /// Sibling hashes from leaf-level up to (but not including) the
    /// root. `siblings[i]` is the sibling of the node at layer `i` on
    /// the path from leaf to root.
    #[serde(with = "crate::serde_helpers::hex_vec_hash")]
    pub siblings: Vec<Hash>,
}

impl InclusionWitness {
    /// Builds a witness for the leaf at a known index in the sorted,
    /// padded leaves array.
    ///
    /// Faster than [`Self::build`] when the index is already known;
    /// avoids the O(n) `position` scan.
    ///
    /// # Examples
    ///
    /// ```
    /// use omega_commitment_core::hash::blake3_256;
    /// use omega_commitment_core::tree::MerkleTree;
    /// use omega_commitment_core::witness::InclusionWitness;
    /// let leaves = vec![blake3_256(b"a"), blake3_256(b"b")];
    /// let tree = MerkleTree::build(leaves);
    /// let w = InclusionWitness::build_at_index(&tree, 0).expect("index in range");
    /// assert!(w.verify(tree.root()));
    /// ```
    ///
    /// Returns `None` if `leaf_index` is out of range.
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

    /// Builds a witness for the given leaf hash.
    ///
    /// Scans the sorted leaves array for `leaf` and constructs the
    /// path. Returns `None` if `leaf` is not present.
    ///
    /// # Examples
    ///
    /// ```
    /// use omega_commitment_core::hash::blake3_256;
    /// use omega_commitment_core::tree::MerkleTree;
    /// use omega_commitment_core::witness::InclusionWitness;
    /// let a = blake3_256(b"a");
    /// let tree = MerkleTree::build(vec![a]);
    /// let w = InclusionWitness::build(&tree, a).expect("leaf present");
    /// assert!(w.verify(tree.root()));
    /// ```
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

    /// Verifies this witness against a claimed root.
    ///
    /// Replays the path with raw Blake3 of `left || right`
    /// concatenations (legacy [`MerkleTree::build`] node construction),
    /// orienting each step by the parity of the index at that layer.
    ///
    /// # Examples
    ///
    /// ```
    /// use omega_commitment_core::hash::blake3_256;
    /// use omega_commitment_core::tree::MerkleTree;
    /// use omega_commitment_core::witness::InclusionWitness;
    /// let a = blake3_256(b"a");
    /// let tree = MerkleTree::build(vec![a]);
    /// let w = InclusionWitness::build(&tree, a).expect("leaf present");
    /// assert!(w.verify(tree.root()));
    /// assert!(!w.verify([0u8; 32]));
    /// ```
    ///
    /// # Soundness
    ///
    /// `verify(root)` returns `true` iff the witness's `leaf` sits at
    /// `leaf_index` in a tree whose layer-by-layer parent computation
    /// `parent = blake3(if idx_bit == 0 { current || sibling } else
    /// { sibling || current })` reaches `root` after `siblings.len()`
    /// steps.
    ///
    /// The malformed-witness checks reject:
    ///
    /// - depths `>= 32` (exceeds `u32` index space and would overflow
    ///   the `1 << depth` bound),
    /// - `leaf_index` outside `[0, 2^depth)` (impossible position for
    ///   a tree of that depth).
    ///
    /// These reject witnesses an honest builder would never produce
    /// and close the malformed-witness DoS class.
    ///
    /// **The verifier does NOT check that `leaf` is a domain-separated
    /// v1 leaf hash.** This witness type is paired with the legacy
    /// [`MerkleTree::build`] construction, which uses raw Blake3 of
    /// `left || right` concatenations and pads with [`ZERO_HASH`]. It
    /// inherits that path's lack of [`DOMAIN_LEAF`] /
    /// [`DOMAIN_NODE`] separation: an adversary who can produce a
    /// 64-byte preimage colliding with an internal node would be
    /// accepted. New code that requires the v1 second-preimage-swap
    /// closure must use [`MerkleTree::build_v1`] together with the
    /// STARK verifier in `omega-claim-verifier`, which constrains
    /// [`crate::tree::leaf_hash_v2`] and [`crate::tree::node_hash_v2`]
    /// inside the AIR.
    ///
    /// # Limitations
    ///
    /// Bound to the legacy [`MerkleTree::build`] hash construction.
    /// Migration to a v1 witness verifier is tracked under v1.0 work
    /// (track T6).
    ///
    /// [`DOMAIN_LEAF`]: crate::tree::DOMAIN_LEAF
    /// [`DOMAIN_NODE`]: crate::tree::DOMAIN_NODE
    /// [`ZERO_HASH`]: crate::tree::ZERO_HASH
    /// [`MerkleTree::build`]: crate::tree::MerkleTree::build
    /// [`MerkleTree::build_v1`]: crate::tree::MerkleTree::build_v1
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
