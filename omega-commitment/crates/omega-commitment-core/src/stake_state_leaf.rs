//! Canonical stake-state leaf encoding (Conway-era DRep tagged enum).
//!
//! A stake-state leaf is the deterministic serialization of:
//!   (stake_credential_hash: 28 bytes) || (delegated_pool: 28 bytes) ||
//!   (drep_tag: u8) || (drep_payload: 0 or 28 bytes) ||
//!   (rewards_lovelace: u64 BE) || (is_pool_operator: u8)
//!
//! Variable-length: 66 bytes (no DRep payload) or 94 bytes (28-byte
//! credential payload). The leaf is hashed with Blake3-256 to produce
//! the leaf hash that goes into the Merkle tree. This sub-tree powers
//! `claim_stake` transactions: users port over delegation, pool, and
//! DRep history with verifiable lineage.
//!
//! ## Reserved values
//!
//! - `delegated_pool == [0u8; 28]` means the credential is not delegating
//!   to any pool.
//!
//! ## DRep delegation tag table (Conway, CDDL `drep`)
//!
//!   `0x00` — None / not delegated
//!   `0x01` — KeyHash(28 bytes payload)
//!   `0x02` — ScriptHash(28 bytes payload)
//!   `0x03` — AlwaysAbstain (no payload)
//!   `0x04` — AlwaysNoConfidence (no payload)
//!
//! AlwaysAbstain and AlwaysNoConfidence are first-class CIP-1694
//! delegation targets, NOT 28-byte hashes; storing them as their tag
//! keeps the encoding faithful to the Conway ledger spec.

use crate::hash::{blake3_256, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 28-byte Cardano credential hash (Blake3-224 of a stake key or
/// stake script). Used for `stake_credential_hash` and `delegated_pool`,
/// and as the payload of [`DrepDelegation::KeyHash`] /
/// [`DrepDelegation::ScriptHash`].
pub type CredentialHash = [u8; 28];

/// Conway-era DRep delegation target.
///
/// Mirrors CIP-1694: a stake credential delegates to either a key-hash
/// DRep, a script-hash DRep, one of the two reserved "predefined" DReps
/// (always-abstain, always-no-confidence), or to no DRep at all.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DrepDelegation {
    #[default]
    None,
    KeyHash {
        #[serde(with = "hex::serde")]
        hash: CredentialHash,
    },
    ScriptHash {
        #[serde(with = "hex::serde")]
        hash: CredentialHash,
    },
    AlwaysAbstain,
    AlwaysNoConfidence,
}

impl DrepDelegation {
    fn tag_byte(&self) -> u8 {
        match self {
            DrepDelegation::None => 0x00,
            DrepDelegation::KeyHash { .. } => 0x01,
            DrepDelegation::ScriptHash { .. } => 0x02,
            DrepDelegation::AlwaysAbstain => 0x03,
            DrepDelegation::AlwaysNoConfidence => 0x04,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct StakeEntry {
    #[serde(with = "hex::serde")]
    pub stake_credential_hash: CredentialHash,
    #[serde(with = "hex::serde")]
    pub delegated_pool: CredentialHash,
    #[serde(default)]
    pub delegated_drep: DrepDelegation,
    pub rewards_lovelace: u64,
    pub is_pool_operator: u8,
}

impl StakeEntry {
    /// Canonical serialization (66 or 94 bytes depending on DRep variant).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(94);
        out.extend_from_slice(&self.stake_credential_hash);
        out.extend_from_slice(&self.delegated_pool);
        out.push(self.delegated_drep.tag_byte());
        match &self.delegated_drep {
            DrepDelegation::KeyHash { hash } | DrepDelegation::ScriptHash { hash } => {
                out.extend_from_slice(hash);
            }
            DrepDelegation::None
            | DrepDelegation::AlwaysAbstain
            | DrepDelegation::AlwaysNoConfidence => {}
        }
        out.extend_from_slice(&self.rewards_lovelace.to_be_bytes());
        out.push(self.is_pool_operator);
        out
    }

    /// Compute the legacy (untagged) leaf hash: Blake3-256 of the
    /// canonical encoding. See [`Self::commit_to_subtree`] for the v1
    /// canonical payload that the domain-separated Merkle builder
    /// consumes.
    pub fn leaf_hash(&self) -> Hash {
        blake3_256(&self.encode())
    }

    /// Return the canonical raw payload bytes for the v1 Merkle
    /// builder.
    pub fn commit_to_subtree(&self) -> Vec<u8> {
        self.encode()
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
            delegated_drep: DrepDelegation::KeyHash {
                hash: [byte.wrapping_add(2); 28],
            },
            rewards_lovelace: rewards,
            is_pool_operator: op,
        }
    }

    #[test]
    fn encoding_with_keyhash_drep_is_94_bytes() {
        let s = sample(0x11, 100, 0);
        assert_eq!(s.encode().len(), 94);
    }

    #[test]
    fn encoding_with_none_drep_is_66_bytes() {
        let mut s = sample(0x11, 100, 0);
        s.delegated_drep = DrepDelegation::None;
        assert_eq!(s.encode().len(), 66);
    }

    #[test]
    fn encoding_with_predefined_drep_is_66_bytes() {
        let mut s = sample(0x11, 100, 0);
        s.delegated_drep = DrepDelegation::AlwaysAbstain;
        assert_eq!(s.encode().len(), 66);
        s.delegated_drep = DrepDelegation::AlwaysNoConfidence;
        assert_eq!(s.encode().len(), 66);
    }

    #[test]
    fn drep_tag_table() {
        let mut s = sample(0x11, 100, 0);
        s.delegated_drep = DrepDelegation::None;
        assert_eq!(s.encode()[56], 0x00);
        s.delegated_drep = DrepDelegation::KeyHash { hash: [0xCC; 28] };
        assert_eq!(s.encode()[56], 0x01);
        s.delegated_drep = DrepDelegation::ScriptHash { hash: [0xCC; 28] };
        assert_eq!(s.encode()[56], 0x02);
        s.delegated_drep = DrepDelegation::AlwaysAbstain;
        assert_eq!(s.encode()[56], 0x03);
        s.delegated_drep = DrepDelegation::AlwaysNoConfidence;
        assert_eq!(s.encode()[56], 0x04);
    }

    #[test]
    fn encoding_layout_is_correct_with_keyhash() {
        let s = StakeEntry {
            stake_credential_hash: [0xAAu8; 28],
            delegated_pool: [0xBBu8; 28],
            delegated_drep: DrepDelegation::KeyHash { hash: [0xCCu8; 28] },
            rewards_lovelace: 0x0102030405060708,
            is_pool_operator: 0x09,
        };
        let bytes = s.encode();
        assert_eq!(&bytes[0..28], &[0xAAu8; 28]);
        assert_eq!(&bytes[28..56], &[0xBBu8; 28]);
        assert_eq!(bytes[56], 0x01); // tag = KeyHash
        assert_eq!(&bytes[57..85], &[0xCCu8; 28]);
        assert_eq!(
            &bytes[85..93],
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
        assert_eq!(bytes[93], 0x09);
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
        b.delegated_drep = DrepDelegation::ScriptHash { hash: [0xEE; 28] };
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn drep_keyhash_vs_scripthash_distinguishable_with_same_payload() {
        // Same 28-byte payload, different DRep kind tag → different leaf.
        let mut a = sample(0x11, 100, 0);
        a.delegated_drep = DrepDelegation::KeyHash { hash: [0xCC; 28] };
        let mut b = a.clone();
        b.delegated_drep = DrepDelegation::ScriptHash { hash: [0xCC; 28] };
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn drep_predefined_distinguishable_from_none() {
        let mut none = sample(0x11, 100, 0);
        none.delegated_drep = DrepDelegation::None;
        let mut abstain = sample(0x11, 100, 0);
        abstain.delegated_drep = DrepDelegation::AlwaysAbstain;
        let mut noconf = sample(0x11, 100, 0);
        noconf.delegated_drep = DrepDelegation::AlwaysNoConfidence;
        assert_ne!(none.leaf_hash(), abstain.leaf_hash());
        assert_ne!(none.leaf_hash(), noconf.leaf_hash());
        assert_ne!(abstain.leaf_hash(), noconf.leaf_hash());
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
            delegated_drep: DrepDelegation::None,
            rewards_lovelace: 0,
            is_pool_operator: 0,
        };
        let bytes = s.encode();
        assert_eq!(&bytes[28..56], &[0u8; 28]);
        assert_eq!(bytes[56], 0x00);
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
