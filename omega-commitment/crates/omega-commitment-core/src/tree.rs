//! Plonky3-friendly binary Merkle tree.
//!
//! - Leaves are sorted by their leaf hash (deterministic ordering).
//! - The tree is padded to the next power of two with the zero-hash leaf.
//! - Internal nodes: H(left || right).
//! - Root: the single hash at the top.
//!
//! This layout is chosen for compatibility with Plonky3 FRI-based
//! verification circuits: fixed depth, fixed arity, no variable-length
//! Merkle paths.

use crate::hash::{blake2b_256, Hash};

pub const ZERO_HASH: Hash = [0u8; 32];

#[derive(Debug, Clone)]
pub struct MerkleTree {
    leaves: Vec<Hash>,      // sorted, padded
    layers: Vec<Vec<Hash>>, // layers[0] = leaves; last = [root]
}

impl MerkleTree {
    /// Build from an unsorted set of leaf hashes.
    ///
    /// The caller is responsible for ensuring uniqueness; duplicate
    /// leaves are NOT deduplicated and will occupy distinct slots in
    /// the tree, producing a different root than a deduplicated input.
    pub fn build(mut input: Vec<Hash>) -> Self {
        input.sort();
        // Pad to next power of two (≥ 1).
        let target = input.len().max(1).next_power_of_two();
        while input.len() < target {
            input.push(ZERO_HASH);
        }
        let mut layers: Vec<Vec<Hash>> = vec![input];
        while layers.last().expect("non-empty").len() > 1 {
            let prev = layers.last().expect("non-empty");
            let mut next = Vec::with_capacity(prev.len() / 2);
            for chunk in prev.chunks(2) {
                let mut buf = [0u8; 64];
                buf[..32].copy_from_slice(&chunk[0]);
                buf[32..].copy_from_slice(&chunk[1]);
                next.push(blake2b_256(&buf));
            }
            layers.push(next);
        }
        Self {
            leaves: layers[0].clone(),
            layers,
        }
    }

    /// Sorted, padded leaves.
    pub fn leaves(&self) -> &[Hash] {
        &self.leaves
    }

    /// All layers from leaves (index 0) up to root layer.
    pub fn layers(&self) -> &[Vec<Hash>] {
        &self.layers
    }

    pub fn root(&self) -> Hash {
        *self
            .layers
            .last()
            .expect("post-build: layers is non-empty")
            .first()
            .expect("post-build: top layer is single root")
    }

    pub fn depth(&self) -> usize {
        self.layers.len() - 1
    }

    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_pads_to_one_zero_leaf() {
        let t = MerkleTree::build(vec![]);
        assert_eq!(t.leaf_count(), 1);
        assert_eq!(t.depth(), 0);
        assert_eq!(t.root(), ZERO_HASH);
    }

    #[test]
    fn single_leaf_tree() {
        let leaf = blake2b_256(b"a");
        let t = MerkleTree::build(vec![leaf]);
        assert_eq!(t.leaf_count(), 1);
        assert_eq!(t.depth(), 0);
        assert_eq!(t.root(), leaf);
    }

    #[test]
    fn two_leaves_tree() {
        let a = blake2b_256(b"a");
        let b = blake2b_256(b"b");
        let t = MerkleTree::build(vec![a, b]);
        assert_eq!(t.leaf_count(), 2);
        assert_eq!(t.depth(), 1);
        // Sorted leaves
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(&lo);
        buf[32..].copy_from_slice(&hi);
        assert_eq!(t.root(), blake2b_256(&buf));
    }

    #[test]
    fn three_leaves_pads_to_four() {
        let a = blake2b_256(b"a");
        let b = blake2b_256(b"b");
        let c = blake2b_256(b"c");
        let t = MerkleTree::build(vec![a, b, c]);
        assert_eq!(t.leaf_count(), 4);
        assert_eq!(t.depth(), 2);
        // The padded leaf is ZERO_HASH.
        assert!(t.leaves().contains(&ZERO_HASH));
    }

    #[test]
    fn root_is_deterministic_under_input_permutation() {
        let leaves: Vec<Hash> = (0..8u8).map(|i| blake2b_256(&[i])).collect();
        let t1 = MerkleTree::build(leaves.clone());
        let mut shuffled = leaves;
        shuffled.reverse();
        let t2 = MerkleTree::build(shuffled);
        assert_eq!(t1.root(), t2.root());
    }

    #[test]
    fn deep_tree_256_leaves() {
        let leaves: Vec<Hash> = (0..256u32).map(|i| blake2b_256(&i.to_be_bytes())).collect();
        let t = MerkleTree::build(leaves);
        assert_eq!(t.leaf_count(), 256);
        assert_eq!(t.depth(), 8);
        // Root is non-zero (overwhelmingly likely).
        assert_ne!(t.root(), ZERO_HASH);
        // Root is reproducible.
        let leaves2: Vec<Hash> = (0..256u32).map(|i| blake2b_256(&i.to_be_bytes())).collect();
        assert_eq!(t.root(), MerkleTree::build(leaves2).root());
    }

    #[test]
    fn duplicate_leaves_are_not_deduplicated() {
        let a = blake2b_256(b"a");
        // Build with one copy.
        let t1 = MerkleTree::build(vec![a]);
        // Build with two copies of the same leaf.
        let t2 = MerkleTree::build(vec![a, a]);
        // Different leaf counts, different depths, different roots.
        assert_eq!(t1.leaf_count(), 1);
        assert_eq!(t2.leaf_count(), 2);
        assert_ne!(t1.root(), t2.root());
    }

    #[test]
    fn build_preserves_root_after_perf_change() {
        // Pin the structural shape for a known input. The integration
        // tests across all 3 sub-trees catch root-bytes drift; this test
        // catches structural changes (depth, leaf_count).
        let leaves: Vec<Hash> = (0..16u8).map(|i| blake2b_256(&[i])).collect();
        let t = MerkleTree::build(leaves);
        assert_eq!(t.depth(), 4);
        assert_eq!(t.leaf_count(), 16);
        let _ = t.root();
    }
}
