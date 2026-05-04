//! Canonical block header leaf encoding.
//!
//! A header leaf is the deterministic serialization of:
//!   (slot: u64 BE) || (block_height: u64 BE) ||
//!   (block_hash: 32 bytes) || (prev_hash: 32 bytes)
//!
//! Total: 80 bytes. The leaf is hashed with Blake3-256 to produce
//! the leaf hash that goes into the Merkle tree.

use crate::hash::{blake3_256, Hash};
use serde::{Deserialize, Serialize};

/// A Cardano block header for the header-chain sub-tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockHeader {
    /// Absolute slot number of the block.
    pub slot: u64,
    /// Block height (number of blocks between genesis and this block).
    pub block_height: u64,
    /// 32-byte block hash.
    #[serde(with = "hex::serde")]
    pub block_hash: [u8; 32],
    /// 32-byte hash of the immediately-preceding block.
    #[serde(with = "hex::serde")]
    pub prev_hash: [u8; 32],
}

impl BlockHeader {
    /// Returns the canonical 80-byte serialization
    /// `slot_be || block_height_be || block_hash || prev_hash`.
    ///
    /// # Examples
    ///
    /// ```
    /// use omega_commitment_core::header_leaf::BlockHeader;
    /// let h = BlockHeader {
    ///     slot: 1, block_height: 1,
    ///     block_hash: [0u8; 32], prev_hash: [0u8; 32],
    /// };
    /// assert_eq!(h.encode().len(), 80);
    /// ```
    ///
    /// # Soundness
    ///
    /// The 80-byte layout is `slot (u64 BE) || block_height (u64 BE)
    /// || block_hash (32) || prev_hash (32)`. This byte sequence is
    /// the leaf preimage that determines the leaf hash and therefore
    /// the per-sub-tree root; any change to widths, ordering, or
    /// endianness is a wire break that desynchronises the prover, the
    /// verifier, and every downstream consumer of the header sub-tree
    /// commitment.
    pub fn encode(&self) -> [u8; 80] {
        let mut out = [0u8; 80];
        out[0..8].copy_from_slice(&self.slot.to_be_bytes());
        out[8..16].copy_from_slice(&self.block_height.to_be_bytes());
        out[16..48].copy_from_slice(&self.block_hash);
        out[48..80].copy_from_slice(&self.prev_hash);
        out
    }

    /// Computes the legacy (untagged) leaf hash: Blake3-256 of the
    /// canonical encoding.
    ///
    /// See [`Self::commit_to_subtree`] for the v1 canonical payload
    /// that the domain-separated Merkle builder consumes.
    ///
    /// # Examples
    ///
    /// ```
    /// use omega_commitment_core::header_leaf::BlockHeader;
    /// let h = BlockHeader {
    ///     slot: 1, block_height: 1,
    ///     block_hash: [0u8; 32], prev_hash: [0u8; 32],
    /// };
    /// assert_eq!(h.leaf_hash().len(), 32);
    /// ```
    pub fn leaf_hash(&self) -> Hash {
        blake3_256(&self.encode())
    }

    /// Returns the canonical raw payload bytes for the v1 Merkle
    /// builder.
    ///
    /// Equivalent to [`Self::encode`] returned as a `Vec<u8>` so that
    /// all seven sub-trees expose a uniform v1 entry point.
    ///
    /// # Examples
    ///
    /// ```
    /// use omega_commitment_core::header_leaf::BlockHeader;
    /// let h = BlockHeader {
    ///     slot: 1, block_height: 1,
    ///     block_hash: [0u8; 32], prev_hash: [0u8; 32],
    /// };
    /// assert_eq!(h.commit_to_subtree().len(), 80);
    /// ```
    pub fn commit_to_subtree(&self) -> Vec<u8> {
        self.encode().to_vec()
    }
}

/// Validates that a slice of headers forms a well-linked chain
/// ordered by strictly-increasing slot, where each header's
/// `prev_hash` matches the previous header's `block_hash`.
///
/// Returns the index of the first failure, or `None` if the chain is
/// valid. The first header is treated as genesis and its `prev_hash`
/// is not validated.
///
/// # Examples
///
/// ```
/// use omega_commitment_core::header_leaf::{validate_chain_links, BlockHeader};
/// let headers: [BlockHeader; 0] = [];
/// assert_eq!(validate_chain_links(&headers), None);
/// ```
///
/// This is an optional sanity check for callers; commitment generation
/// does NOT require chain-link validity.
pub fn validate_chain_links(headers: &[BlockHeader]) -> Option<usize> {
    for i in 1..headers.len() {
        if headers[i].slot <= headers[i - 1].slot {
            return Some(i);
        }
        if headers[i].prev_hash != headers[i - 1].block_hash {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header(slot: u64, height: u64) -> BlockHeader {
        BlockHeader {
            slot,
            block_height: height,
            block_hash: [slot as u8; 32],
            prev_hash: [(slot.saturating_sub(1)) as u8; 32],
        }
    }

    #[test]
    fn encoding_is_exactly_80_bytes() {
        let h = sample_header(100, 50);
        assert_eq!(h.encode().len(), 80);
    }

    #[test]
    fn encoding_layout_is_correct() {
        let h = BlockHeader {
            slot: 0x0102030405060708,
            block_height: 0x1112131415161718,
            block_hash: [0xAAu8; 32],
            prev_hash: [0xBBu8; 32],
        };
        let e = h.encode();
        assert_eq!(&e[0..8], &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(&e[8..16], &[0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18]);
        assert_eq!(&e[16..48], &[0xAAu8; 32]);
        assert_eq!(&e[48..80], &[0xBBu8; 32]);
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let h = sample_header(7, 3);
        assert_eq!(h.leaf_hash(), h.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_slot_change() {
        let h1 = sample_header(7, 3);
        let mut h2 = h1.clone();
        h2.slot = 8;
        assert_ne!(h1.leaf_hash(), h2.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_height_change() {
        let h1 = sample_header(7, 3);
        let mut h2 = h1.clone();
        h2.block_height = 4;
        assert_ne!(h1.leaf_hash(), h2.leaf_hash());
    }

    #[test]
    fn validate_chain_links_accepts_well_linked() {
        let mut a = sample_header(1, 1);
        a.block_hash = [0x01; 32];
        let mut b = sample_header(2, 2);
        b.block_hash = [0x02; 32];
        b.prev_hash = a.block_hash;
        let mut c = sample_header(3, 3);
        c.block_hash = [0x03; 32];
        c.prev_hash = b.block_hash;
        assert_eq!(validate_chain_links(&[a, b, c]), None);
    }

    #[test]
    fn validate_chain_links_rejects_bad_prev_hash() {
        let mut a = sample_header(1, 1);
        a.block_hash = [0x01; 32];
        let mut b = sample_header(2, 2);
        b.prev_hash = [0xFF; 32]; // does not match a.block_hash
        assert_eq!(validate_chain_links(&[a, b]), Some(1));
    }

    #[test]
    fn validate_chain_links_rejects_non_monotonic_slot() {
        let mut a = sample_header(5, 1);
        a.block_hash = [0x01; 32];
        let mut b = sample_header(3, 2);
        b.prev_hash = a.block_hash;
        assert_eq!(validate_chain_links(&[a, b]), Some(1));
    }

    #[test]
    fn validate_chain_links_empty_and_single_are_valid() {
        assert_eq!(validate_chain_links(&[]), None);
        assert_eq!(validate_chain_links(&[sample_header(0, 0)]), None);
    }
}
