//! Canonical transaction-index leaf encoding.
//!
//! A tx-index leaf is the deterministic serialization of:
//!   (tx_id: 32 bytes) || (slot: u64 BE) ||
//!   (block_hash: 32 bytes) || (tx_position: u32 BE)
//!
//! Total: 76 bytes. The leaf is hashed with Blake2b-256 to produce
//! the leaf hash that goes into the Merkle tree. This sub-tree powers
//! `claim_tx` transactions: users prove "tx H existed at slot S in
//! block B at position P."

use crate::hash::{blake2b_256, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TxIndexEntry {
    #[serde(with = "hex::serde")]
    pub tx_id: [u8; 32],
    pub slot: u64,
    #[serde(with = "hex::serde")]
    pub block_hash: [u8; 32],
    pub tx_position: u32,
}

impl TxIndexEntry {
    /// Canonical 76-byte serialization.
    pub fn encode(&self) -> [u8; 76] {
        let mut out = [0u8; 76];
        out[0..32].copy_from_slice(&self.tx_id);
        out[32..40].copy_from_slice(&self.slot.to_be_bytes());
        out[40..72].copy_from_slice(&self.block_hash);
        out[72..76].copy_from_slice(&self.tx_position.to_be_bytes());
        out
    }

    /// Compute the legacy (untagged) leaf hash: Blake2b-256 of the
    /// canonical encoding. See [`Self::commit_to_subtree`] for the v1
    /// canonical payload that the domain-separated Merkle builder
    /// consumes.
    pub fn leaf_hash(&self) -> Hash {
        blake2b_256(&self.encode())
    }

    /// Return the canonical raw payload bytes for the v1 Merkle
    /// builder.
    pub fn commit_to_subtree(&self) -> Vec<u8> {
        self.encode().to_vec()
    }
}

/// Validate that no `tx_id` appears more than once across the entries.
/// Returns the index of the second occurrence of the first duplicate
/// found, or None if all `tx_id`s are unique.
///
/// Cardano transaction hashes are deterministic functions of the tx
/// body and should be unique across the whole chain. Duplicate input
/// is a data error (e.g., a snapshot with overlapping epoch ranges).
/// This is an OPTIONAL sanity helper; commitment generation does NOT
/// require uniqueness.
pub fn validate_tx_uniqueness(entries: &[TxIndexEntry]) -> Option<usize> {
    let mut seen: HashSet<[u8; 32]> = HashSet::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        if !seen.insert(e.tx_id) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(tx_id_byte: u8, slot: u64, pos: u32) -> TxIndexEntry {
        TxIndexEntry {
            tx_id: [tx_id_byte; 32],
            slot,
            block_hash: [0xCC; 32],
            tx_position: pos,
        }
    }

    #[test]
    fn encoding_is_exactly_76_bytes() {
        let e = sample(0x11, 100, 0);
        assert_eq!(e.encode().len(), 76);
    }

    #[test]
    fn encoding_layout_is_correct() {
        let e = TxIndexEntry {
            tx_id: [0xAAu8; 32],
            slot: 0x0102030405060708,
            block_hash: [0xBBu8; 32],
            tx_position: 0x11223344,
        };
        let bytes = e.encode();
        assert_eq!(&bytes[0..32], &[0xAAu8; 32]);
        assert_eq!(
            &bytes[32..40],
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
        assert_eq!(&bytes[40..72], &[0xBBu8; 32]);
        assert_eq!(&bytes[72..76], &[0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let e = sample(0x11, 100, 0);
        assert_eq!(e.leaf_hash(), e.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_tx_id_change() {
        let a = sample(0x11, 100, 0);
        let b = sample(0x12, 100, 0);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_slot_change() {
        let a = sample(0x11, 100, 0);
        let b = sample(0x11, 101, 0);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_position_change() {
        let a = sample(0x11, 100, 0);
        let b = sample(0x11, 100, 1);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_block_hash_change() {
        let a = sample(0x11, 100, 0);
        let mut b = a.clone();
        b.block_hash = [0xDD; 32];
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn validate_tx_uniqueness_accepts_unique() {
        let entries = vec![sample(0x01, 1, 0), sample(0x02, 2, 0), sample(0x03, 3, 0)];
        assert_eq!(validate_tx_uniqueness(&entries), None);
    }

    #[test]
    fn validate_tx_uniqueness_finds_duplicate() {
        let entries = vec![sample(0x01, 1, 0), sample(0x02, 2, 0), sample(0x01, 5, 1)];
        assert_eq!(validate_tx_uniqueness(&entries), Some(2));
    }

    #[test]
    fn validate_tx_uniqueness_empty_is_valid() {
        assert_eq!(validate_tx_uniqueness(&[]), None);
    }

    #[test]
    fn same_tx_id_different_slot_still_distinct_leaves() {
        let a = sample(0x11, 100, 0);
        let b = sample(0x11, 200, 0);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }
}
