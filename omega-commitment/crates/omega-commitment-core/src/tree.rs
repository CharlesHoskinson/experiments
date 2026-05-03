//! Plonky3-friendly binary Merkle tree.
//!
//! The v1 construction (`build_v1`) computes:
//!   - leaves via `leaf_hash_v2(sub_tree_id, canonical_index, payload)`
//!     (see `DOMAIN_LEAF`),
//!   - internal nodes via `node_hash_v2(left, right)` (see `DOMAIN_NODE`),
//!   - padding leaves via `leaf_hash_v2(sub_tree_id, EMPTY_INDEX_SENTINEL,
//!     &[])` so that zero-padding is no longer a valid membership target.
//!
//! Properties:
//!   - Leaves are sorted by their raw payload bytes (deterministic
//!     ordering), then bound to their sorted index via the leaf hash.
//!   - Duplicate payloads are rejected (`BuildError::DuplicateLeafPayload`)
//!     because the same canonical key cannot legitimately appear twice
//!     in a sub-tree.
//!   - The tree is padded to the next power of two with the
//!     domain-separated empty-leaf hash (NOT raw zero bytes).
//!   - Internal nodes always carry the `omega:v2:node` tag; leaves
//!     always carry `omega:v2:leaf`. The two domains never collide.
//!
//! The legacy `build(Vec<Hash>)` path is preserved as a deprecated
//! compatibility alias for tests and CLIs that pre-hash. It does NOT
//! apply domain separation and MUST NOT be used by new code paths
//! that need the v1 soundness guarantees.

use crate::hash::{blake3_256, Hash};
use thiserror::Error;

/// Domain tag bound into every v1 leaf preimage.
pub const DOMAIN_LEAF: &[u8] = b"omega:v2:leaf";
/// Domain tag bound into every v1 internal-node preimage.
pub const DOMAIN_NODE: &[u8] = b"omega:v2:node";

/// Sentinel index used when synthesising a domain-separated empty leaf
/// for power-of-two padding. A verifier MUST reject any inclusion proof
/// whose `canonical_index` equals this value — the index is reserved
/// for padding and cannot be a real membership target.
pub const EMPTY_INDEX_SENTINEL: u64 = u64::MAX;

/// Legacy zero-hash padding leaf retained for the deprecated
/// `MerkleTree::build` path. New code must use `build_v1` which pads
/// with the domain-separated empty-leaf hash.
pub const ZERO_HASH: Hash = [0u8; 32];

/// Errors returned by the v1 builder.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildError {
    /// Two distinct leaves shared the same payload bytes after canonical
    /// sorting. The canonical key for each sub-tree must be unique.
    #[error("duplicate leaf payload in sub-tree {sub_tree_id} at sorted index {index}")]
    DuplicateLeafPayload { sub_tree_id: u8, index: usize },
}

/// Domain-separated leaf hash.
///
/// `H(DOMAIN_LEAF || sub_tree_id || canonical_index_be || payload_len_be
///    || payload)` using Blake3-256.
///
/// Binding the `(sub_tree_id, canonical_index)` pair into the preimage
/// closes the audit findings A1/F001 (leaves did not bind sub_tree_id
/// or canonical leaf_index) and A1/F002 (leaf and internal-node hashes
/// shared an untagged domain).
pub fn leaf_hash_v2(sub_tree_id: u8, canonical_index: u64, payload: &[u8]) -> Hash {
    let mut buf = Vec::with_capacity(DOMAIN_LEAF.len() + 1 + 8 + 8 + payload.len());
    buf.extend_from_slice(DOMAIN_LEAF);
    buf.push(sub_tree_id);
    buf.extend_from_slice(&canonical_index.to_be_bytes());
    buf.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    buf.extend_from_slice(payload);
    blake3_256(&buf)
}

/// Domain-separated internal-node hash.
///
/// `H(DOMAIN_NODE || left || right)` using Blake3-256.
pub fn node_hash_v2(left: &Hash, right: &Hash) -> Hash {
    let mut buf = Vec::with_capacity(DOMAIN_NODE.len() + 64);
    buf.extend_from_slice(DOMAIN_NODE);
    buf.extend_from_slice(left);
    buf.extend_from_slice(right);
    blake3_256(&buf)
}

#[derive(Debug, Clone)]
pub struct MerkleTree {
    leaves: Vec<Hash>,      // sorted, padded
    layers: Vec<Vec<Hash>>, // layers[0] = leaves; last = [root]
}

impl MerkleTree {
    /// Build a v1 domain-separated tree from canonical leaf payloads.
    ///
    /// `payloads` is the raw canonical-encoded byte representation of
    /// each leaf (NOT pre-hashed). Payloads are sorted by raw bytes for
    /// determinism, then bound to their sorted index via
    /// `leaf_hash_v2`. Duplicate payloads are rejected.
    pub fn build_v1(sub_tree_id: u8, mut payloads: Vec<Vec<u8>>) -> Result<Self, BuildError> {
        payloads.sort();
        // Reject duplicate canonical keys: in every sub-tree the unsorted
        // semantic key (txid+output_index, slot+block_hash, etc.) is
        // injected for membership uniqueness, so two byte-identical
        // payloads always indicate ingest-side data corruption.
        for window in payloads.windows(2) {
            if window[0] == window[1] {
                let index = payloads
                    .iter()
                    .position(|p| p == &window[1])
                    .unwrap_or_default();
                return Err(BuildError::DuplicateLeafPayload { sub_tree_id, index });
            }
        }
        let mut leaves: Vec<Hash> = payloads
            .iter()
            .enumerate()
            .map(|(i, p)| leaf_hash_v2(sub_tree_id, i as u64, p))
            .collect();
        // Pad to next power of two with the domain-separated empty leaf.
        let target = leaves.len().max(1).next_power_of_two();
        while leaves.len() < target {
            leaves.push(leaf_hash_v2(sub_tree_id, EMPTY_INDEX_SENTINEL, &[]));
        }
        Ok(Self::build_layers_v1(leaves))
    }

    fn build_layers_v1(leaves: Vec<Hash>) -> Self {
        let mut layers: Vec<Vec<Hash>> = vec![leaves];
        while layers.last().expect("non-empty").len() > 1 {
            let prev = layers.last().expect("non-empty");
            let mut next = Vec::with_capacity(prev.len() / 2);
            for chunk in prev.chunks(2) {
                next.push(node_hash_v2(&chunk[0], &chunk[1]));
            }
            layers.push(next);
        }
        Self {
            leaves: layers[0].clone(),
            layers,
        }
    }

    /// Build a tree from an unsorted set of pre-hashed leaves.
    ///
    /// **Compatibility-only.** Retained for tests and CLI paths that
    /// already pre-hash their leaves. Does NOT apply v1 domain
    /// separation and pads with `ZERO_HASH`. New production code MUST
    /// use [`Self::build_v1`] to obtain the `omega:v2:leaf` /
    /// `omega:v2:node` soundness guarantees; the inclusion-witness
    /// verifier (`witness::InclusionWitness::verify`) will be migrated
    /// to v1 as part of the v1.0 verifier-circuit work (track T6).
    ///
    /// The caller is responsible for ensuring uniqueness; duplicate
    /// leaves are NOT deduplicated and will occupy distinct slots in
    /// the tree, producing a different root than a deduplicated input.
    pub fn build(input: Vec<Hash>) -> Self {
        Self::build_legacy(input)
    }

    /// Internal alias used by the deprecated `build` and by tests
    /// that need the legacy zero-padded layout. Not part of the
    /// public surface for production code.
    fn build_legacy(mut input: Vec<Hash>) -> Self {
        input.sort();
        // Pad to next power of two (≥ 1) with raw ZERO_HASH (legacy path).
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
                next.push(blake3_256(&buf));
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
        let leaf = blake3_256(b"a");
        let t = MerkleTree::build(vec![leaf]);
        assert_eq!(t.leaf_count(), 1);
        assert_eq!(t.depth(), 0);
        assert_eq!(t.root(), leaf);
    }

    #[test]
    fn two_leaves_tree() {
        let a = blake3_256(b"a");
        let b = blake3_256(b"b");
        let t = MerkleTree::build(vec![a, b]);
        assert_eq!(t.leaf_count(), 2);
        assert_eq!(t.depth(), 1);
        // Sorted leaves
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(&lo);
        buf[32..].copy_from_slice(&hi);
        assert_eq!(t.root(), blake3_256(&buf));
    }

    #[test]
    fn three_leaves_pads_to_four() {
        let a = blake3_256(b"a");
        let b = blake3_256(b"b");
        let c = blake3_256(b"c");
        let t = MerkleTree::build(vec![a, b, c]);
        assert_eq!(t.leaf_count(), 4);
        assert_eq!(t.depth(), 2);
        // The padded leaf is ZERO_HASH (legacy path).
        assert!(t.leaves().contains(&ZERO_HASH));
    }

    #[test]
    fn root_is_deterministic_under_input_permutation() {
        let leaves: Vec<Hash> = (0..8u8).map(|i| blake3_256(&[i])).collect();
        let t1 = MerkleTree::build(leaves.clone());
        let mut shuffled = leaves;
        shuffled.reverse();
        let t2 = MerkleTree::build(shuffled);
        assert_eq!(t1.root(), t2.root());
    }

    #[test]
    fn deep_tree_256_leaves() {
        let leaves: Vec<Hash> = (0..256u32).map(|i| blake3_256(&i.to_be_bytes())).collect();
        let t = MerkleTree::build(leaves);
        assert_eq!(t.leaf_count(), 256);
        assert_eq!(t.depth(), 8);
        // Root is non-zero (overwhelmingly likely).
        assert_ne!(t.root(), ZERO_HASH);
        // Root is reproducible.
        let leaves2: Vec<Hash> = (0..256u32).map(|i| blake3_256(&i.to_be_bytes())).collect();
        assert_eq!(t.root(), MerkleTree::build(leaves2).root());
    }

    #[test]
    fn duplicate_leaves_are_not_deduplicated() {
        let a = blake3_256(b"a");
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
        let leaves: Vec<Hash> = (0..16u8).map(|i| blake3_256(&[i])).collect();
        let t = MerkleTree::build(leaves);
        assert_eq!(t.depth(), 4);
        assert_eq!(t.leaf_count(), 16);
        let _ = t.root();
    }

    // ---------------------------------------------------------------
    // v1 domain-separation lock tests (Batch 1, A4/F001).
    // ---------------------------------------------------------------

    #[test]
    fn test_leaf_hash_v2_differs_from_raw_blake3_of_payload() {
        // A v1 leaf binds the domain tag, sub_tree_id, canonical_index,
        // and length prefix into the preimage. A raw Blake3 over just
        // the payload MUST NOT collide with the domain-separated hash.
        let payload = b"some-canonical-leaf-payload".to_vec();
        let domain_separated = leaf_hash_v2(1, 0, &payload);
        let raw = blake3_256(&payload);
        assert_ne!(
            domain_separated, raw,
            "v1 leaf hash collided with raw Blake3 — domain separation lost"
        );
    }

    #[test]
    fn test_leaf_hash_v2_differs_from_node_hash_of_same_input_pair() {
        // The leaf and node hashes carry distinct domain tags. Even if
        // an attacker could supply an internal-node concatenation as a
        // 64-byte "payload" of a leaf, the domain tags must keep the
        // outputs apart so that a node preimage cannot be reinterpreted
        // as a membership leaf.
        let left = blake3_256(b"left");
        let right = blake3_256(b"right");
        let mut concat = Vec::with_capacity(64);
        concat.extend_from_slice(&left);
        concat.extend_from_slice(&right);
        let leaf = leaf_hash_v2(1, 0, &concat);
        let node = node_hash_v2(&left, &right);
        assert_ne!(
            leaf, node,
            "leaf and node hashes collided — second-preimage swap is open"
        );
    }

    #[test]
    fn test_zero_padded_leaf_distinct_from_zero_payload_at_same_index() {
        // The padding leaf uses EMPTY_INDEX_SENTINEL. A real leaf
        // carrying an empty payload at canonical index 0 (or any
        // non-sentinel index) MUST hash to a different value, so
        // padding cannot be presented as a valid membership target.
        let pad_leaf = leaf_hash_v2(1, EMPTY_INDEX_SENTINEL, &[]);
        let zero_payload_at_index_0 = leaf_hash_v2(1, 0, &[]);
        assert_ne!(
            pad_leaf, zero_payload_at_index_0,
            "padding sentinel collided with index-0 empty leaf"
        );
        // The padding leaf must also differ from the legacy raw zero
        // hash that the deprecated build path uses for padding.
        assert_ne!(
            pad_leaf, ZERO_HASH,
            "v1 padding leaf collided with legacy ZERO_HASH"
        );
    }

    // ---------------------------------------------------------------
    // v1 builder behaviour tests.
    // ---------------------------------------------------------------

    #[test]
    fn build_v1_rejects_duplicate_payloads() {
        let payloads = vec![b"alpha".to_vec(), b"beta".to_vec(), b"alpha".to_vec()];
        let err = MerkleTree::build_v1(1, payloads).unwrap_err();
        match err {
            BuildError::DuplicateLeafPayload { sub_tree_id, .. } => {
                assert_eq!(sub_tree_id, 1);
            }
        }
    }

    #[test]
    fn build_v1_changing_sub_tree_id_changes_root() {
        let payloads = vec![b"x".to_vec(), b"y".to_vec()];
        let t1 = MerkleTree::build_v1(1, payloads.clone()).unwrap();
        let t2 = MerkleTree::build_v1(2, payloads).unwrap();
        assert_ne!(
            t1.root(),
            t2.root(),
            "sub_tree_id is bound into every leaf — roots must diverge"
        );
    }

    #[test]
    fn build_v1_pads_to_power_of_two() {
        // 3 payloads -> 4 leaves (one padding leaf at index 3).
        let payloads = vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()];
        let t = MerkleTree::build_v1(1, payloads).unwrap();
        assert_eq!(t.leaf_count(), 4);
        assert_eq!(t.depth(), 2);
    }

    #[test]
    fn build_v1_is_deterministic_under_permutation() {
        let payloads_a = vec![b"x".to_vec(), b"y".to_vec(), b"z".to_vec(), b"w".to_vec()];
        let payloads_b = vec![b"w".to_vec(), b"z".to_vec(), b"y".to_vec(), b"x".to_vec()];
        let t_a = MerkleTree::build_v1(3, payloads_a).unwrap();
        let t_b = MerkleTree::build_v1(3, payloads_b).unwrap();
        assert_eq!(t_a.root(), t_b.root());
    }
}
