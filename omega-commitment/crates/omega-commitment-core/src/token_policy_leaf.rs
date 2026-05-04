//! Canonical native-token policy leaf encoding.
//!
//! A token-policy leaf is the deterministic serialization of:
//!   (policy_id: 28 bytes) || (first_issuance_slot: u64 BE) ||
//!   (total_supply_at_h: u128 BE)
//!
//! Total: 52 bytes. The leaf is hashed with Blake3-256 to produce
//! the leaf hash that goes into the Merkle tree. This sub-tree powers
//! `claim_token_policy` transactions: token issuers can re-anchor a
//! minting policy on the new chain with verifiable lineage.
//!
//! ## Note on policy_id width
//!
//! Cardano policy hashes are **28 bytes** (Blake3-224 of the minting
//! script), not 32. This is the first cross-sub-tree asymmetry in the
//! Ω-Commitment library. The 28-byte size is canonical Cardano
//! ledger semantics; verifiers must encode policies as 28-byte values
//! to compute leaf hashes consistent with on-chain identifiers.
//!
//! Note that the leaf hash itself remains Blake3-256 → 32 bytes;
//! only the preimage contains a 28-byte field.

use crate::hash::{blake3_256, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 28-byte Cardano native-token policy hash (Blake3-224 of the
/// minting script). Distinct from the 32-byte `Hash` type used for
/// internal Merkle hashing.
pub type PolicyId = [u8; 28];

/// A native-token policy entry: the issuance lineage of a single
/// minting policy at the snapshot height `H`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TokenPolicy {
    /// 28-byte Cardano policy hash (Blake3-224 of the minting script).
    #[serde(with = "hex::serde")]
    pub policy_id: PolicyId,
    /// Slot at which the policy first issued tokens.
    pub first_issuance_slot: u64,
    /// Total supply minted under this policy as of height `H`.
    pub total_supply_at_h: u128,
}

impl TokenPolicy {
    /// Returns the canonical 52-byte serialization
    /// `policy_id || first_issuance_slot_be || total_supply_at_h_be`.
    ///
    /// # Examples
    ///
    /// ```
    /// use omega_commitment_core::token_policy_leaf::TokenPolicy;
    /// let p = TokenPolicy {
    ///     policy_id: [0u8; 28], first_issuance_slot: 0, total_supply_at_h: 0,
    /// };
    /// assert_eq!(p.encode().len(), 52);
    /// ```
    ///
    /// # Soundness
    ///
    /// The 52-byte layout is `policy_id (28) || first_issuance_slot
    /// (u64 BE) || total_supply_at_h (u128 BE)`. This byte sequence is
    /// the leaf preimage and therefore determines the leaf hash and
    /// the per-sub-tree root; any change to widths, ordering, or
    /// endianness is a wire break. Note that `policy_id` is 28 bytes
    /// (Cardano canonical width) while the leaf hash is 32 bytes —
    /// the asymmetry is intentional and matches on-chain identifiers.
    pub fn encode(&self) -> [u8; 52] {
        let mut out = [0u8; 52];
        out[0..28].copy_from_slice(&self.policy_id);
        out[28..36].copy_from_slice(&self.first_issuance_slot.to_be_bytes());
        out[36..52].copy_from_slice(&self.total_supply_at_h.to_be_bytes());
        out
    }

    /// Computes the legacy (untagged) leaf hash: Blake3-256 of the
    /// canonical encoding.
    ///
    /// # Examples
    ///
    /// ```
    /// use omega_commitment_core::token_policy_leaf::TokenPolicy;
    /// let p = TokenPolicy {
    ///     policy_id: [0u8; 28], first_issuance_slot: 0, total_supply_at_h: 0,
    /// };
    /// assert_eq!(p.leaf_hash().len(), 32);
    /// ```
    pub fn leaf_hash(&self) -> Hash {
        blake3_256(&self.encode())
    }

    /// Returns the canonical raw payload bytes for the v1 Merkle
    /// builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use omega_commitment_core::token_policy_leaf::TokenPolicy;
    /// let p = TokenPolicy {
    ///     policy_id: [0u8; 28], first_issuance_slot: 0, total_supply_at_h: 0,
    /// };
    /// assert_eq!(p.commit_to_subtree().len(), 52);
    /// ```
    pub fn commit_to_subtree(&self) -> Vec<u8> {
        self.encode().to_vec()
    }
}

/// Validates that no `policy_id` appears more than once across the
/// entries.
///
/// Returns the index of the second occurrence of the first duplicate
/// found, or `None` if all `policy_id`s are unique.
///
/// # Examples
///
/// ```
/// use omega_commitment_core::token_policy_leaf::{
///     validate_policy_id_uniqueness, TokenPolicy,
/// };
/// let entries: [TokenPolicy; 0] = [];
/// assert_eq!(validate_policy_id_uniqueness(&entries), None);
/// ```
///
/// Cardano policy hashes are deterministic functions of the minting
/// script and should be unique. Duplicate input is a data error
/// (e.g., overlapping epoch ranges). This is an OPTIONAL sanity helper;
/// commitment generation does NOT require uniqueness.
pub fn validate_policy_id_uniqueness(entries: &[TokenPolicy]) -> Option<usize> {
    let mut seen: HashSet<PolicyId> = HashSet::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        if !seen.insert(e.policy_id) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(byte: u8, slot: u64, supply: u128) -> TokenPolicy {
        TokenPolicy {
            policy_id: [byte; 28],
            first_issuance_slot: slot,
            total_supply_at_h: supply,
        }
    }

    #[test]
    fn encoding_is_exactly_52_bytes() {
        let p = sample(0x11, 100, 1_000_000);
        assert_eq!(p.encode().len(), 52);
    }

    #[test]
    fn encoding_layout_is_correct() {
        let p = TokenPolicy {
            policy_id: [0xAAu8; 28],
            first_issuance_slot: 0x0102030405060708,
            total_supply_at_h: 0x1112131415161718_2122232425262728,
        };
        let bytes = p.encode();
        assert_eq!(&bytes[0..28], &[0xAAu8; 28]);
        assert_eq!(
            &bytes[28..36],
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
        assert_eq!(
            &bytes[36..52],
            &[
                0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26,
                0x27, 0x28,
            ]
        );
    }

    #[test]
    fn policy_id_is_28_bytes() {
        let p = sample(0x11, 100, 0);
        assert_eq!(p.policy_id.len(), 28);
    }

    #[test]
    fn leaf_hash_is_32_bytes() {
        let p = sample(0x11, 100, 0);
        let h = p.leaf_hash();
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let p = sample(0x11, 100, 1000);
        assert_eq!(p.leaf_hash(), p.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_policy_id_change() {
        let a = sample(0x11, 100, 1000);
        let b = sample(0x12, 100, 1000);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_slot_change() {
        let a = sample(0x11, 100, 1000);
        let b = sample(0x11, 101, 1000);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_supply_change() {
        let a = sample(0x11, 100, 1000);
        let b = sample(0x11, 100, 1001);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn supply_at_u128_max_encodes_correctly() {
        let p = TokenPolicy {
            policy_id: [0x11; 28],
            first_issuance_slot: 0,
            total_supply_at_h: u128::MAX,
        };
        let bytes = p.encode();
        assert_eq!(&bytes[36..52], &[0xFFu8; 16]);
    }

    #[test]
    fn validate_policy_id_uniqueness_accepts_unique() {
        let entries = vec![
            sample(0x01, 1, 100),
            sample(0x02, 2, 200),
            sample(0x03, 3, 300),
        ];
        assert_eq!(validate_policy_id_uniqueness(&entries), None);
    }

    #[test]
    fn validate_policy_id_uniqueness_finds_duplicate() {
        let entries = vec![
            sample(0x01, 1, 100),
            sample(0x02, 2, 200),
            sample(0x01, 5, 999),
        ];
        assert_eq!(validate_policy_id_uniqueness(&entries), Some(2));
    }

    #[test]
    fn validate_policy_id_uniqueness_empty_is_valid() {
        assert_eq!(validate_policy_id_uniqueness(&[]), None);
    }

    #[test]
    fn same_policy_id_different_slot_still_distinct_leaves() {
        let a = sample(0x11, 100, 1000);
        let b = sample(0x11, 200, 1000);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }
}
