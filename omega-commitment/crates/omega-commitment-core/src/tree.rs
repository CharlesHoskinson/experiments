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

/// Domain tag (`b"omega:v2:leaf"`) bound into every v1 leaf preimage.
///
/// # Soundness
///
/// Distinct from [`DOMAIN_NODE`] so that no leaf preimage collides with
/// any internal-node preimage. Closes the second-preimage swap class
/// of attack: an adversary who concatenates two child hashes as a
/// 64-byte "leaf payload" cannot reproduce an existing internal-node
/// hash because the domain tags differ at byte zero of the preimage.
pub const DOMAIN_LEAF: &[u8] = b"omega:v2:leaf";

/// Domain tag (`b"omega:v2:node"`) bound into every v1 internal-node
/// preimage.
///
/// # Soundness
///
/// Distinct from [`DOMAIN_LEAF`]; see that constant's `# Soundness`
/// block for the second-preimage-swap framing.
pub const DOMAIN_NODE: &[u8] = b"omega:v2:node";

/// Reserved `canonical_index` value (`u64::MAX`) used by
/// [`MerkleTree::build_v1`] when synthesising the domain-separated
/// empty leaves that pad the tree to the next power of two.
///
/// # Soundness
///
/// A verifier reading the published `item_count` MUST reject any
/// inclusion proof whose `canonical_index >= item_count`, and in
/// particular any proof whose `canonical_index == EMPTY_INDEX_SENTINEL`.
/// The sentinel is reserved for padding and is never a legitimate
/// membership target. This closes the padding-leaf forgery attack: an
/// adversary who points a witness at a padding slot cannot have it
/// accepted as a real leaf because the verifier rejects the index
/// before checking the path.
pub const EMPTY_INDEX_SENTINEL: u64 = u64::MAX;

/// Legacy raw-zero hash used as padding by the deprecated
/// [`MerkleTree::build`] path.
///
/// # Soundness
///
/// `ZERO_HASH` is NOT domain-separated and an adversary can trivially
/// forge `ZERO_HASH` as the "hash" of an unknown preimage by definition
/// (it is the all-zeros byte string). Production paths that need
/// padding-leaf-forgery resistance MUST use [`MerkleTree::build_v1`],
/// which pads with [`leaf_hash_v2(_, EMPTY_INDEX_SENTINEL, &[])`][leaf_hash_v2]
/// instead.
pub const ZERO_HASH: Hash = [0u8; 32];

/// Errors returned by [`MerkleTree::build_v1`].
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildError {
    /// Two distinct leaves shared the same payload bytes after sorting.
    /// In every sub-tree the unsorted semantic key (txid+output_index,
    /// slot+block_hash, policy_id, etc.) is bound into the payload, so
    /// byte-identical payloads always indicate ingest-side data
    /// corruption.
    #[error("duplicate leaf payload in sub-tree {sub_tree_id} at sorted index {index}")]
    DuplicateLeafPayload {
        /// Sub-tree identifier the builder was invoked with.
        sub_tree_id: u8,
        /// Sorted index of the second occurrence of the duplicated
        /// payload.
        index: usize,
    },
}

/// Computes a v1 domain-separated leaf hash.
///
/// The preimage is
/// `DOMAIN_LEAF || sub_tree_id || canonical_index_be || payload_len_be
/// || payload`, hashed with [`blake3_256`]. The domain tag closes the
/// classic Merkle second-preimage swap; the `(sub_tree_id,
/// canonical_index)` pair is bound into the preimage so a verifier
/// reading a published `item_count` can reject any inclusion proof
/// whose `canonical_index >= item_count`.
///
/// # Examples
///
/// ```
/// use omega_commitment_core::tree::leaf_hash_v2;
/// let h = leaf_hash_v2(1, 0, b"alice");
/// assert_eq!(h.len(), 32);
/// ```
///
/// # Soundness
///
/// Output is collision-resistant against any adversary bounded by
/// Blake3's security level (128 bits). Two distinct
/// `(sub_tree_id, canonical_index, payload)` triples produce different
/// hashes; a triple cannot collide with [`node_hash_v2`]'s output
/// because the domain tags differ. Binding the `(sub_tree_id,
/// canonical_index)` pair closes the audit findings A1/F001 (leaves
/// did not bind `sub_tree_id` or canonical `leaf_index`) and A1/F002
/// (leaf and internal-node hashes shared an untagged domain).
///
/// # Limitations
///
/// `payload` MUST be ≤ 64 bytes for the v0.1 verifier circuit (one
/// Blake3 compression block). Longer payloads compute correctly but
/// cannot be proven in the v0.1 `OmegaMembershipAir`; v0.2's
/// `LeafPreimageAir` lifts the bound.
pub fn leaf_hash_v2(sub_tree_id: u8, canonical_index: u64, payload: &[u8]) -> Hash {
    let mut buf = Vec::with_capacity(DOMAIN_LEAF.len() + 1 + 8 + 8 + payload.len());
    buf.extend_from_slice(DOMAIN_LEAF);
    buf.push(sub_tree_id);
    buf.extend_from_slice(&canonical_index.to_be_bytes());
    buf.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    buf.extend_from_slice(payload);
    blake3_256(&buf)
}

/// Computes a v1 domain-separated internal-node hash.
///
/// The preimage is `DOMAIN_NODE || left || right`, hashed with
/// [`blake3_256`].
///
/// # Examples
///
/// ```
/// use omega_commitment_core::tree::node_hash_v2;
/// let left = [0x11u8; 32];
/// let right = [0x22u8; 32];
/// let h = node_hash_v2(&left, &right);
/// assert_eq!(h.len(), 32);
/// ```
///
/// # Soundness
///
/// Output is collision-resistant against any adversary bounded by
/// Blake3's security level (128 bits). The `DOMAIN_NODE` tag prevents
/// any internal-node preimage from colliding with a leaf preimage: an
/// adversary who supplies a 64-byte concatenation `left || right` as
/// a leaf payload cannot reproduce the matching internal-node hash
/// because the leaf preimage carries [`DOMAIN_LEAF`] while the node
/// preimage carries [`DOMAIN_NODE`]. The `(left, right)` pair is
/// position-sensitive: swapping the two siblings produces a different
/// hash, which is what the witness verifier exploits to enforce path
/// orientation.
pub fn node_hash_v2(left: &Hash, right: &Hash) -> Hash {
    let mut buf = Vec::with_capacity(DOMAIN_NODE.len() + 64);
    buf.extend_from_slice(DOMAIN_NODE);
    buf.extend_from_slice(left);
    buf.extend_from_slice(right);
    blake3_256(&buf)
}

/// Plonky3-friendly binary Merkle tree, padded to the next power of
/// two.
///
/// Constructed via [`MerkleTree::build_v1`] for production paths
/// (domain-separated, sub-tree-bound) or [`MerkleTree::build`] for
/// legacy pre-hashed inputs. Once built, the tree exposes
/// [`MerkleTree::root`], [`MerkleTree::depth`], [`MerkleTree::leaves`],
/// and [`MerkleTree::layers`] for witness construction.
#[derive(Debug, Clone)]
pub struct MerkleTree {
    leaves: Vec<Hash>,      // sorted, padded
    layers: Vec<Vec<Hash>>, // layers[0] = leaves; last = [root]
}

impl MerkleTree {
    /// Builds a v1 domain-separated Merkle tree from canonical leaf
    /// payloads.
    ///
    /// `payloads` is the raw canonical-encoded byte representation of
    /// each leaf (NOT pre-hashed). The builder:
    ///
    /// 1. Sorts `payloads` lexicographically by raw bytes (deterministic
    ///    ordering: two byte-different snapshots representing the same
    ///    logical set produce the same root).
    /// 2. Rejects duplicate payloads with
    ///    [`BuildError::DuplicateLeafPayload`].
    /// 3. Hashes each leaf via [`leaf_hash_v2(sub_tree_id, sorted_index,
    ///    payload)`][leaf_hash_v2], binding the sorted index into the
    ///    preimage.
    /// 4. Pads to the next power of two with
    ///    [`leaf_hash_v2(sub_tree_id, EMPTY_INDEX_SENTINEL, &[])`][leaf_hash_v2].
    /// 5. Builds the binary tree with [`node_hash_v2`] at every internal
    ///    node.
    ///
    /// # Examples
    ///
    /// ```
    /// use omega_commitment_core::tree::MerkleTree;
    /// let payloads = vec![b"alice".to_vec(), b"bob".to_vec()];
    /// let tree = MerkleTree::build_v1(1, payloads)?;
    /// assert_eq!(tree.depth(), 1);
    /// # Ok::<(), omega_commitment_core::tree::BuildError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// - [`BuildError::DuplicateLeafPayload`] when two distinct entries
    ///   share canonical bytes after sorting. The semantic key for each
    ///   sub-tree (txid+output_index, slot+block_hash, policy_id, etc.)
    ///   is injected into the payload, so byte-identical payloads always
    ///   indicate ingest-side data corruption.
    ///
    /// # Soundness
    ///
    /// `build_v1` preserves three invariants that v0.1 verifiers and
    /// auditors rely on:
    ///
    /// - **Set-not-sequence semantics.** The leaf set determines the
    ///   root. Sorting by raw bytes is the canonical ordering; a
    ///   permutation of the input produces the same root. Two
    ///   independent ingest pipelines that produce the same logical
    ///   leaf set will produce the same Merkle root regardless of the
    ///   order in which their parsers emitted entries.
    /// - **Uniqueness.** No two leaves share canonical bytes. The
    ///   builder rejects duplicates rather than silently deduplicating
    ///   them, because byte-identical payloads with the protocol's
    ///   semantic-key injection scheme indicates a data error upstream
    ///   that must surface, not be masked.
    /// - **Padding-leaf forgery resistance.** Padding leaves are
    ///   computed via [`leaf_hash_v2`] with the
    ///   [`EMPTY_INDEX_SENTINEL`] (`u64::MAX`) bound into the preimage.
    ///   A verifier that knows the published `item_count` rejects any
    ///   inclusion proof whose `canonical_index >= item_count`, so an
    ///   adversary cannot present a padding slot as a real leaf. The
    ///   v0.1 legacy [`Self::build`] path uses raw [`ZERO_HASH`] for
    ///   padding and does NOT carry this protection.
    ///
    /// The output's [`Self::root`] is bound to `sub_tree_id`: the same
    /// payload set built under a different `sub_tree_id` produces a
    /// different root, so a leaf claimed against the wrong sub-tree
    /// cannot be re-pointed at a sibling sub-tree's commitment.
    ///
    /// # Limitations
    ///
    /// Each individual `payloads[i]` MUST be ≤ 64 bytes for the v0.1
    /// verifier circuit. See [`leaf_hash_v2`] for the boundary.
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

    /// Builds a tree from an unsorted set of pre-hashed leaves
    /// (legacy compatibility path).
    ///
    /// **Compatibility-only.** Retained for tests and CLI paths that
    /// already pre-hash their leaves. Does NOT apply v1 domain
    /// separation and pads with [`ZERO_HASH`]. New production code MUST
    /// use [`Self::build_v1`] to obtain the `omega:v2:leaf` /
    /// `omega:v2:node` soundness guarantees; the inclusion-witness
    /// verifier ([`crate::witness::InclusionWitness::verify`]) will be
    /// migrated to v1 as part of the v1.0 verifier-circuit work
    /// (track T6).
    ///
    /// # Examples
    ///
    /// ```
    /// use omega_commitment_core::hash::blake3_256;
    /// use omega_commitment_core::tree::MerkleTree;
    /// let leaves = vec![blake3_256(b"a"), blake3_256(b"b")];
    /// let tree = MerkleTree::build(leaves);
    /// assert_eq!(tree.depth(), 1);
    /// ```
    ///
    /// # Soundness
    ///
    /// This path is NOT soundness-bearing for production. The caller is
    /// responsible for ensuring uniqueness; duplicate leaves are NOT
    /// deduplicated and will occupy distinct slots in the tree,
    /// producing a different root than a deduplicated input. Padding
    /// uses raw [`ZERO_HASH`], which is trivially forge-able as the
    /// "hash" of an unknown preimage; padding-leaf forgery is open on
    /// this path. Use [`Self::build_v1`] in new code.
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

    /// Returns the sorted, padded leaf hashes (layer 0).
    pub fn leaves(&self) -> &[Hash] {
        &self.leaves
    }

    /// Returns every layer from leaves (index 0) up to and including
    /// the single-element root layer.
    pub fn layers(&self) -> &[Vec<Hash>] {
        &self.layers
    }

    /// Returns the Merkle root.
    pub fn root(&self) -> Hash {
        *self
            .layers
            .last()
            .expect("post-build: layers is non-empty")
            .first()
            .expect("post-build: top layer is single root")
    }

    /// Returns the depth of the tree (number of edges from leaf to
    /// root). A single-leaf tree has depth `0`.
    pub fn depth(&self) -> usize {
        self.layers.len() - 1
    }

    /// Returns the number of leaves after sorting and padding to the
    /// next power of two.
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
