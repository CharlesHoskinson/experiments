//! Canonical Plutus / native-script registry leaf encoding.
//!
//! A script-registry leaf is the deterministic serialization of:
//!   (script_hash: 28 bytes) || (deployment_slot: u64 BE) ||
//!   (script_size_bytes: u32 BE) || (language: u8)
//!
//! Total: 41 bytes. The leaf is hashed with Blake2b-256 to produce
//! the leaf hash that goes into the Merkle tree. This sub-tree powers
//! `claim_script` transactions: developers re-anchor a validator hash
//! on the new chain with verifiable lineage. Pure provenance/identity
//! continuity — does NOT re-execute scripts.
//!
//! ## script_hash width
//!
//! Cardano script hashes are 28 bytes (Blake2b-224 of the canonical
//! script bytes), matching the policy-hash width in `token_policy_leaf`.
//! See that module's docstring for the full rationale on why preimage
//! widths can differ from the 32-byte leaf-hash output.
//!
//! ## language byte
//!
//! - `0` = native multi-sig (timelock script)
//! - `1` = Plutus V1
//! - `2` = Plutus V2 (Vasil)
//! - `3` = Plutus V3 (Plomin)
//!
//! Future variants are reserved. The encoding intentionally uses a
//! fixed `u8` slot rather than an open-ended enum so the byte layout
//! stays stable across language additions.

use crate::hash::{blake2b_256, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 28-byte Cardano script hash (Blake2b-224 of the canonical script
/// bytes). Distinct from the 32-byte `Hash` type used for internal
/// Merkle hashing.
pub type ScriptHash = [u8; 28];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ScriptEntry {
    #[serde(with = "hex::serde")]
    pub script_hash: ScriptHash,
    pub deployment_slot: u64,
    pub script_size_bytes: u32,
    pub language: u8,
}

impl ScriptEntry {
    /// Canonical 41-byte serialization.
    pub fn encode(&self) -> [u8; 41] {
        let mut out = [0u8; 41];
        out[0..28].copy_from_slice(&self.script_hash);
        out[28..36].copy_from_slice(&self.deployment_slot.to_be_bytes());
        out[36..40].copy_from_slice(&self.script_size_bytes.to_be_bytes());
        out[40] = self.language;
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

/// Validate that no `script_hash` appears more than once across the
/// entries. Returns the index of the second occurrence of the first
/// duplicate, or None if all `script_hash`es are unique.
///
/// Cardano script hashes are deterministic Blake2b-224 of the
/// canonical script bytes; duplicates indicate a data error
/// (e.g., overlapping epoch ranges in the input snapshot). This is
/// an OPTIONAL sanity helper; commitment generation does NOT require
/// uniqueness.
pub fn validate_script_hash_uniqueness(entries: &[ScriptEntry]) -> Option<usize> {
    let mut seen: HashSet<ScriptHash> = HashSet::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        if !seen.insert(e.script_hash) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(byte: u8, slot: u64, size: u32, lang: u8) -> ScriptEntry {
        ScriptEntry {
            script_hash: [byte; 28],
            deployment_slot: slot,
            script_size_bytes: size,
            language: lang,
        }
    }

    #[test]
    fn encoding_is_exactly_41_bytes() {
        let s = sample(0x11, 100, 2048, 2);
        assert_eq!(s.encode().len(), 41);
    }

    #[test]
    fn encoding_layout_is_correct() {
        let s = ScriptEntry {
            script_hash: [0xAAu8; 28],
            deployment_slot: 0x0102030405060708,
            script_size_bytes: 0x11223344,
            language: 0x07,
        };
        let bytes = s.encode();
        assert_eq!(&bytes[0..28], &[0xAAu8; 28]);
        assert_eq!(
            &bytes[28..36],
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
        assert_eq!(&bytes[36..40], &[0x11, 0x22, 0x33, 0x44]);
        assert_eq!(bytes[40], 0x07);
    }

    #[test]
    fn script_hash_is_28_bytes() {
        let s = sample(0x11, 100, 0, 0);
        assert_eq!(s.script_hash.len(), 28);
    }

    #[test]
    fn leaf_hash_is_32_bytes() {
        let s = sample(0x11, 100, 0, 0);
        assert_eq!(s.leaf_hash().len(), 32);
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let s = sample(0x11, 100, 1024, 2);
        assert_eq!(s.leaf_hash(), s.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_script_hash_change() {
        let a = sample(0x11, 100, 1024, 2);
        let b = sample(0x12, 100, 1024, 2);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_slot_change() {
        let a = sample(0x11, 100, 1024, 2);
        let b = sample(0x11, 101, 1024, 2);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_size_change() {
        let a = sample(0x11, 100, 1024, 2);
        let b = sample(0x11, 100, 1025, 2);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_language_change() {
        let a = sample(0x11, 100, 1024, 2);
        let b = sample(0x11, 100, 1024, 3);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn all_four_languages_produce_distinct_leaves() {
        let leaves: Vec<Hash> = (0..=3u8)
            .map(|lang| sample(0x11, 100, 1024, lang).leaf_hash())
            .collect();
        // All four leaf hashes pairwise distinct.
        for i in 0..leaves.len() {
            for j in (i + 1)..leaves.len() {
                assert_ne!(leaves[i], leaves[j], "lang {} vs {} collided", i, j);
            }
        }
    }

    #[test]
    fn future_language_bytes_round_trip() {
        // language=255 (reserved) must encode and hash without panic.
        let s = sample(0x11, 100, 1024, 255);
        let bytes = s.encode();
        assert_eq!(bytes[40], 255);
        let _ = s.leaf_hash();
    }

    #[test]
    fn validate_script_hash_uniqueness_accepts_unique() {
        let entries = vec![
            sample(0x01, 1, 100, 0),
            sample(0x02, 2, 200, 1),
            sample(0x03, 3, 300, 2),
        ];
        assert_eq!(validate_script_hash_uniqueness(&entries), None);
    }

    #[test]
    fn validate_script_hash_uniqueness_finds_duplicate() {
        let entries = vec![
            sample(0x01, 1, 100, 0),
            sample(0x02, 2, 200, 1),
            sample(0x01, 5, 999, 3),
        ];
        assert_eq!(validate_script_hash_uniqueness(&entries), Some(2));
    }

    #[test]
    fn validate_script_hash_uniqueness_empty_is_valid() {
        assert_eq!(validate_script_hash_uniqueness(&[]), None);
    }

    #[test]
    fn same_script_hash_different_deploy_slot_distinct_leaves() {
        // Even if upstream data accidentally has the same hash twice
        // with different metadata, leaf hashes diverge — confirming
        // the entire tuple contributes to leaf identity.
        let a = sample(0x11, 100, 1024, 2);
        let b = sample(0x11, 200, 1024, 2);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }
}
