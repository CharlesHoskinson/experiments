//! Canonical stake-state leaf encoding.
//!
//! A stake-state leaf is the deterministic serialization of:
//!   (stake_credential_hash: 28 bytes) || (delegated_pool: 28 bytes) ||
//!   (delegated_drep: 28 bytes) || (rewards_lovelace: u64 BE) ||
//!   (is_pool_operator: u8)
//!
//! Total: 93 bytes. The leaf is hashed with Blake2b-256 to produce
//! the leaf hash that goes into the Merkle tree. This sub-tree powers
//! `claim_stake` transactions: users port over delegation, pool, and
//! DRep history with verifiable lineage.
//!
//! ## Reserved values
//!
//! - `delegated_pool == [0u8; 28]` means the credential is not delegating
//!   to any pool.
//! - `delegated_drep == [0u8; 28]` means no active DRep delegation.
//!   The canonical "always-abstain" and "always-no-confidence" DRep IDs
//!   are upstream Cardano constants and are stored as their literal
//!   28-byte values (NOT encoded as zero).

use crate::hash::{blake2b_256, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 28-byte Cardano credential hash (Blake2b-224 of a stake key or
/// stake script). Used for `stake_credential_hash`, `delegated_pool`,
/// and `delegated_drep`.
pub type CredentialHash = [u8; 28];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct StakeEntry {
    #[serde(with = "hex::serde")]
    pub stake_credential_hash: CredentialHash,
    #[serde(with = "hex::serde")]
    pub delegated_pool: CredentialHash,
    #[serde(with = "hex::serde")]
    pub delegated_drep: CredentialHash,
    pub rewards_lovelace: u64,
    pub is_pool_operator: u8,
}

impl StakeEntry {
    /// Canonical 93-byte serialization.
    pub fn encode(&self) -> [u8; 93] {
        let mut out = [0u8; 93];
        out[0..28].copy_from_slice(&self.stake_credential_hash);
        out[28..56].copy_from_slice(&self.delegated_pool);
        out[56..84].copy_from_slice(&self.delegated_drep);
        out[84..92].copy_from_slice(&self.rewards_lovelace.to_be_bytes());
        out[92] = self.is_pool_operator;
        out
    }

    /// Compute the leaf hash: Blake2b-256 of canonical encoding.
    pub fn leaf_hash(&self) -> Hash {
        blake2b_256(&self.encode())
    }
}

/// Validate that no `stake_credential_hash` appears more than once
/// across the entries. Returns the index of the second occurrence
/// of the first duplicate, or None if all are unique.
///
/// Cardano stake credentials are deterministic; duplicates indicate
/// a data error. Optional sanity helper; commitment generation does
/// NOT require uniqueness.
pub fn validate_stake_credential_uniqueness(entries: &[StakeEntry]) -> Option<usize> {
    let mut seen: HashSet<CredentialHash> = HashSet::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        if !seen.insert(e.stake_credential_hash) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(byte: u8, rewards: u64, op: u8) -> StakeEntry {
        StakeEntry {
            stake_credential_hash: [byte; 28],
            delegated_pool: [byte.wrapping_add(1); 28],
            delegated_drep: [byte.wrapping_add(2); 28],
            rewards_lovelace: rewards,
            is_pool_operator: op,
        }
    }

    #[test]
    fn encoding_is_exactly_93_bytes() {
        let s = sample(0x11, 100, 0);
        assert_eq!(s.encode().len(), 93);
    }

    #[test]
    fn encoding_layout_is_correct() {
        let s = StakeEntry {
            stake_credential_hash: [0xAAu8; 28],
            delegated_pool: [0xBBu8; 28],
            delegated_drep: [0xCCu8; 28],
            rewards_lovelace: 0x0102030405060708,
            is_pool_operator: 0x09,
        };
        let bytes = s.encode();
        assert_eq!(&bytes[0..28], &[0xAAu8; 28]);
        assert_eq!(&bytes[28..56], &[0xBBu8; 28]);
        assert_eq!(&bytes[56..84], &[0xCCu8; 28]);
        assert_eq!(
            &bytes[84..92],
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
        assert_eq!(bytes[92], 0x09);
    }

    #[test]
    fn leaf_hash_is_32_bytes() {
        let s = sample(0x11, 100, 0);
        assert_eq!(s.leaf_hash().len(), 32);
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let s = sample(0x11, 100, 1);
        assert_eq!(s.leaf_hash(), s.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_credential_change() {
        let a = sample(0x11, 100, 0);
        let b = sample(0x12, 100, 0);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_pool_change() {
        let a = sample(0x11, 100, 0);
        let mut b = a.clone();
        b.delegated_pool = [0xFF; 28];
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_drep_change() {
        let a = sample(0x11, 100, 0);
        let mut b = a.clone();
        b.delegated_drep = [0xEE; 28];
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_rewards_change() {
        let a = sample(0x11, 100, 0);
        let b = sample(0x11, 101, 0);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_pool_operator_flag() {
        let a = sample(0x11, 100, 0);
        let b = sample(0x11, 100, 1);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn zero_pool_means_undelegated() {
        let s = StakeEntry {
            stake_credential_hash: [0x11; 28],
            delegated_pool: [0u8; 28],
            delegated_drep: [0u8; 28],
            rewards_lovelace: 0,
            is_pool_operator: 0,
        };
        let bytes = s.encode();
        assert_eq!(&bytes[28..56], &[0u8; 28]);
    }

    #[test]
    fn validate_stake_credential_uniqueness_accepts_unique() {
        let entries = vec![
            sample(0x01, 100, 0),
            sample(0x02, 200, 0),
            sample(0x03, 300, 1),
        ];
        assert_eq!(validate_stake_credential_uniqueness(&entries), None);
    }

    #[test]
    fn validate_stake_credential_uniqueness_finds_duplicate() {
        let entries = vec![
            sample(0x01, 100, 0),
            sample(0x02, 200, 0),
            sample(0x01, 999, 0),
        ];
        assert_eq!(validate_stake_credential_uniqueness(&entries), Some(2));
    }

    #[test]
    fn validate_stake_credential_uniqueness_empty_is_valid() {
        assert_eq!(validate_stake_credential_uniqueness(&[]), None);
    }
}
