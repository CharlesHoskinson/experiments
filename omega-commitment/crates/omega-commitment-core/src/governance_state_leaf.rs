//! Canonical governance-state leaf encoding.
//!
//! A governance-state leaf is the deterministic serialization of:
//!   (kind: u8) || (key: 32 bytes) || (value: u128 BE) || (slot: u64 BE)
//!
//! Total: 57 bytes. The leaf is hashed with Blake2b-256 to produce
//! the leaf hash that goes into the Merkle tree. This sub-tree powers
//! `claim_governance` transactions: users port over treasury, CC seat,
//! and governance-action history.
//!
//! ## Heterogeneity vs. uniformity
//!
//! Governance state is intrinsically heterogeneous (treasury balance,
//! CC seats, gov-action records). Rather than building seven inner
//! trees, we commit to one tree of "governance facts" where each fact
//! has a `kind` discriminant and a fixed-width payload. This keeps
//! the encoding canonical and Plonky3-friendly.
//!
//! ## Kind discriminants
//!
//! - `0` — Treasury balance. `key` = all-zero. `value` = lovelace balance.
//! - `1` — CC seat. `key` = member's credential hash (right-padded from
//!   28 bytes to 32 with zeros). `value` = expiration epoch.
//! - `2` — Ratified gov action. `key` = action's tx_id (full 32 bytes).
//!   `value` = packed `(action_type:u16 << 0) | (slot_ratified:u64 << 16)`;
//!   top 48 bits reserved.
//! - `3` — In-flight gov action. `key` = action's tx_id. `value` = packed
//!   `(action_type:u16 << 0) | (slot_submitted:u64 << 16)`; top 48 bits
//!   reserved.
//! - Future variants reserved.
//!
//! A verifier reading a leaf's preimage MUST consult `kind` to interpret
//! `key` and `value`; the encoding does not self-describe beyond that.

use crate::hash::{blake2b_256, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct GovernanceFact {
    pub kind: u8,
    #[serde(with = "hex::serde")]
    pub key: [u8; 32],
    pub value: u128,
    pub slot: u64,
}

impl GovernanceFact {
    /// Canonical 57-byte serialization.
    pub fn encode(&self) -> [u8; 57] {
        let mut out = [0u8; 57];
        out[0] = self.kind;
        out[1..33].copy_from_slice(&self.key);
        out[33..49].copy_from_slice(&self.value.to_be_bytes());
        out[49..57].copy_from_slice(&self.slot.to_be_bytes());
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

/// Validate that no `(kind, key)` pair appears more than once across
/// the entries. Returns the index of the second occurrence of the
/// first duplicate, or None if all `(kind, key)` pairs are unique.
///
/// The same `key` is allowed across different `kind`s (e.g., a
/// gov-action tx_id could appear as both ratified and in-flight in
/// theory, though rare in practice). Optional sanity helper;
/// commitment generation does NOT require uniqueness.
pub fn validate_governance_keys_unique_per_kind(entries: &[GovernanceFact]) -> Option<usize> {
    let mut seen: HashSet<(u8, [u8; 32])> = HashSet::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        if !seen.insert((e.kind, e.key)) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(kind: u8, key_byte: u8, value: u128, slot: u64) -> GovernanceFact {
        GovernanceFact {
            kind,
            key: [key_byte; 32],
            value,
            slot,
        }
    }

    #[test]
    fn encoding_is_exactly_57_bytes() {
        let f = fact(0, 0, 1_000_000, 100);
        assert_eq!(f.encode().len(), 57);
    }

    #[test]
    fn encoding_layout_is_correct() {
        let f = GovernanceFact {
            kind: 0x07,
            key: [0xAAu8; 32],
            value: 0x1112131415161718_2122232425262728,
            slot: 0x3132333435363738,
        };
        let bytes = f.encode();
        assert_eq!(bytes[0], 0x07);
        assert_eq!(&bytes[1..33], &[0xAAu8; 32]);
        assert_eq!(
            &bytes[33..49],
            &[
                0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26,
                0x27, 0x28,
            ]
        );
        assert_eq!(
            &bytes[49..57],
            &[0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38]
        );
    }

    #[test]
    fn leaf_hash_is_32_bytes() {
        let f = fact(0, 0, 0, 0);
        assert_eq!(f.leaf_hash().len(), 32);
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let f = fact(1, 0x11, 100, 200);
        assert_eq!(f.leaf_hash(), f.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_kind_change() {
        let a = fact(0, 0x11, 100, 200);
        let b = fact(1, 0x11, 100, 200);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_key_change() {
        let a = fact(0, 0x11, 100, 200);
        let b = fact(0, 0x12, 100, 200);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_value_change() {
        let a = fact(0, 0x11, 100, 200);
        let b = fact(0, 0x11, 101, 200);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_slot_change() {
        let a = fact(0, 0x11, 100, 200);
        let b = fact(0, 0x11, 100, 201);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn all_four_kinds_distinct_leaves() {
        let leaves: Vec<Hash> = (0..=3u8)
            .map(|k| fact(k, 0x11, 100, 200).leaf_hash())
            .collect();
        for i in 0..leaves.len() {
            for j in (i + 1)..leaves.len() {
                assert_ne!(leaves[i], leaves[j], "kind {} vs {} collided", i, j);
            }
        }
    }

    #[test]
    fn future_kind_byte_round_trips() {
        // kind=255 (reserved) must encode and hash without panic.
        let f = fact(255, 0, 0, 0);
        assert_eq!(f.encode()[0], 255);
        let _ = f.leaf_hash();
    }

    #[test]
    fn u128_max_value_encodes_correctly() {
        let f = fact(0, 0, u128::MAX, 0);
        let bytes = f.encode();
        assert_eq!(&bytes[33..49], &[0xFFu8; 16]);
    }

    #[test]
    fn validate_keys_unique_per_kind_accepts_unique() {
        let entries = vec![
            fact(0, 0x01, 100, 200),
            fact(1, 0x02, 100, 200),
            fact(2, 0x03, 100, 200),
        ];
        assert_eq!(validate_governance_keys_unique_per_kind(&entries), None);
    }

    #[test]
    fn validate_keys_unique_per_kind_finds_duplicate() {
        let entries = vec![
            fact(0, 0x01, 100, 200),
            fact(1, 0x01, 200, 300),
            fact(0, 0x01, 999, 999), // duplicate (kind=0, key=0x01)
        ];
        assert_eq!(validate_governance_keys_unique_per_kind(&entries), Some(2));
    }

    #[test]
    fn validate_keys_unique_per_kind_allows_same_key_different_kind() {
        // Same key 0x01 used for kind=0 (treasury) and kind=2 (ratified
        // gov action) — should be allowed.
        let entries = vec![fact(0, 0x01, 100, 200), fact(2, 0x01, 200, 300)];
        assert_eq!(validate_governance_keys_unique_per_kind(&entries), None);
    }

    #[test]
    fn validate_keys_unique_per_kind_empty_is_valid() {
        assert_eq!(validate_governance_keys_unique_per_kind(&[]), None);
    }
}
