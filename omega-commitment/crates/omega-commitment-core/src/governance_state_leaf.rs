//! Canonical governance-state leaf encoding.
//!
//! Governance facts are heterogeneous (treasury balance, CC seats,
//! gov-action records, AccountState pots). Each fact carries a one-byte
//! `kind` discriminant followed by a kind-specific payload, and the
//! whole pre-image is hashed with Blake3-256 to produce the leaf hash
//! that goes into the Merkle tree. This sub-tree powers
//! `claim_governance` transactions: users port over treasury, CC seat,
//! and governance-action history.
//!
//! ## Kind discriminants and pre-image layouts
//!
//! - `0x00` — Treasury balance.
//!   Pre-image: `0x00 || (key=[0u8;32]) || (value: u128 BE) || (slot: u64 BE)`
//!   Width: 57 bytes.
//!
//! - `0x01` — CC seat.
//!   Pre-image: `0x01 || (key: 32 bytes; member's credential hash padded
//!   from 28 to 32 with zeros) || (value: u128 BE; expiration epoch) ||
//!   (slot: u64 BE)`
//!   Width: 57 bytes.
//!
//! - `0x02` — Ratified gov action.
//!   Pre-image: `0x02 || (key: 32 bytes; action's tx_id) || (value: u128 BE;
//!   packed `(action_type:u16 << 0) | (slot_ratified:u64 << 16)`, top 48
//!   bits reserved) || (slot: u64 BE)`
//!   Width: 57 bytes.
//!
//! - `0x03` — In-flight gov action.
//!   Pre-image: `0x03 || (key: 32 bytes; action's tx_id) || (value: u128 BE;
//!   packed `(action_type:u16 << 0) | (slot_submitted:u64 << 16)`, top 48
//!   bits reserved) || (slot: u64 BE)`
//!   Width: 57 bytes.
//!
//! - `0x04` — AccountState pots (reserves / treasury / deposits / fee_pot).
//!   Pre-image: `0x04 || (reserves: u64 BE) || (treasury: u64 BE) ||
//!   (deposits: u64 BE) || (fee_pot: u64 BE)`
//!   Width: 33 bytes.
//!
//! - Future variants reserved.

use crate::hash::{blake3_256, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Kind tags used in the canonical encoding's leading byte.
pub mod kind {
    pub const TREASURY: u8 = 0x00;
    pub const CC_SEAT: u8 = 0x01;
    pub const RATIFIED_ACTION: u8 = 0x02;
    pub const IN_FLIGHT_ACTION: u8 = 0x03;
    pub const ACCOUNT_STATE: u8 = 0x04;
}

/// A single governance-state fact. Variants share a one-byte `kind`
/// discriminant in the canonical pre-image; see the module docstring
/// for the full per-variant layout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GovernanceFact {
    /// `kind == 0x00`. Treasury balance snapshot.
    Treasury {
        #[serde(with = "hex::serde")]
        key: [u8; 32],
        #[serde(with = "crate::serde_helpers::u128_dec")]
        value: u128,
        slot: u64,
    },
    /// `kind == 0x01`. CC seat record.
    CcSeat {
        #[serde(with = "hex::serde")]
        key: [u8; 32],
        #[serde(with = "crate::serde_helpers::u128_dec")]
        value: u128,
        slot: u64,
    },
    /// `kind == 0x02`. Ratified governance action.
    RatifiedAction {
        #[serde(with = "hex::serde")]
        key: [u8; 32],
        #[serde(with = "crate::serde_helpers::u128_dec")]
        value: u128,
        slot: u64,
    },
    /// `kind == 0x03`. In-flight governance action.
    InFlightAction {
        #[serde(with = "hex::serde")]
        key: [u8; 32],
        #[serde(with = "crate::serde_helpers::u128_dec")]
        value: u128,
        slot: u64,
    },
    /// `kind == 0x04`. Conway-era ledger AccountState pots.
    /// All four pots are required; the ingest layer fails closed if
    /// any one is missing from the input snapshot.
    AccountState {
        reserves: u64,
        treasury: u64,
        deposits: u64,
        fee_pot: u64,
    },
}

impl GovernanceFact {
    /// One-byte canonical kind discriminant.
    pub fn kind(&self) -> u8 {
        match self {
            GovernanceFact::Treasury { .. } => kind::TREASURY,
            GovernanceFact::CcSeat { .. } => kind::CC_SEAT,
            GovernanceFact::RatifiedAction { .. } => kind::RATIFIED_ACTION,
            GovernanceFact::InFlightAction { .. } => kind::IN_FLIGHT_ACTION,
            GovernanceFact::AccountState { .. } => kind::ACCOUNT_STATE,
        }
    }

    /// Canonical byte serialization. Width depends on variant:
    /// 57 bytes for the legacy 4 variants, 33 bytes for AccountState.
    pub fn encode(&self) -> Vec<u8> {
        match self {
            GovernanceFact::Treasury { key, value, slot }
            | GovernanceFact::CcSeat { key, value, slot }
            | GovernanceFact::RatifiedAction { key, value, slot }
            | GovernanceFact::InFlightAction { key, value, slot } => {
                let mut out = Vec::with_capacity(57);
                out.push(self.kind());
                out.extend_from_slice(key);
                out.extend_from_slice(&value.to_be_bytes());
                out.extend_from_slice(&slot.to_be_bytes());
                out
            }
            GovernanceFact::AccountState {
                reserves,
                treasury,
                deposits,
                fee_pot,
            } => {
                let mut out = Vec::with_capacity(33);
                out.push(self.kind());
                out.extend_from_slice(&reserves.to_be_bytes());
                out.extend_from_slice(&treasury.to_be_bytes());
                out.extend_from_slice(&deposits.to_be_bytes());
                out.extend_from_slice(&fee_pot.to_be_bytes());
                out
            }
        }
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

/// Identity used by [`validate_governance_keys_unique_per_kind`] to
/// detect duplicate facts. AccountState has no `key` field; it is
/// represented by a sentinel `(0xFF, [0u8;32])` pair so that a
/// snapshot can carry at most one AccountState fact.
fn fact_identity(f: &GovernanceFact) -> (u8, [u8; 32]) {
    match f {
        GovernanceFact::Treasury { key, .. }
        | GovernanceFact::CcSeat { key, .. }
        | GovernanceFact::RatifiedAction { key, .. }
        | GovernanceFact::InFlightAction { key, .. } => (f.kind(), *key),
        GovernanceFact::AccountState { .. } => (f.kind(), [0u8; 32]),
    }
}

/// Validate that no `(kind, key)` pair appears more than once across
/// the entries. Returns the index of the second occurrence of the
/// first duplicate, or None if all `(kind, key)` pairs are unique.
///
/// The same `key` is allowed across different `kind`s (e.g., a
/// gov-action tx_id could appear as both ratified and in-flight in
/// theory, though rare in practice). Optional sanity helper;
/// commitment generation does NOT require uniqueness. AccountState
/// is allowed at most once because it is a singleton snapshot.
pub fn validate_governance_keys_unique_per_kind(entries: &[GovernanceFact]) -> Option<usize> {
    let mut seen: HashSet<(u8, [u8; 32])> = HashSet::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        if !seen.insert(fact_identity(e)) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(kind_byte: u8, key_byte: u8, value: u128, slot: u64) -> GovernanceFact {
        let key = [key_byte; 32];
        match kind_byte {
            kind::TREASURY => GovernanceFact::Treasury { key, value, slot },
            kind::CC_SEAT => GovernanceFact::CcSeat { key, value, slot },
            kind::RATIFIED_ACTION => GovernanceFact::RatifiedAction { key, value, slot },
            kind::IN_FLIGHT_ACTION => GovernanceFact::InFlightAction { key, value, slot },
            other => panic!("unknown kind {other}"),
        }
    }

    #[test]
    fn encoding_is_exactly_57_bytes_for_treasury_variants() {
        let f = fact(kind::TREASURY, 0, 1_000_000, 100);
        assert_eq!(f.encode().len(), 57);
    }

    #[test]
    fn encoding_is_exactly_33_bytes_for_account_state() {
        let f = GovernanceFact::AccountState {
            reserves: 1,
            treasury: 2,
            deposits: 3,
            fee_pot: 4,
        };
        assert_eq!(f.encode().len(), 33);
    }

    #[test]
    fn account_state_layout_is_correct() {
        let f = GovernanceFact::AccountState {
            reserves: 0x1112_1314_1516_1718,
            treasury: 0x2122_2324_2526_2728,
            deposits: 0x3132_3334_3536_3738,
            fee_pot: 0x4142_4344_4546_4748,
        };
        let b = f.encode();
        assert_eq!(b[0], kind::ACCOUNT_STATE);
        assert_eq!(&b[1..9], &0x1112_1314_1516_1718u64.to_be_bytes());
        assert_eq!(&b[9..17], &0x2122_2324_2526_2728u64.to_be_bytes());
        assert_eq!(&b[17..25], &0x3132_3334_3536_3738u64.to_be_bytes());
        assert_eq!(&b[25..33], &0x4142_4344_4546_4748u64.to_be_bytes());
    }

    #[test]
    fn encoding_layout_is_correct_for_legacy_variants() {
        let f = GovernanceFact::CcSeat {
            key: [0xAAu8; 32],
            value: 0x1112131415161718_2122232425262728,
            slot: 0x3132333435363738,
        };
        let bytes = f.encode();
        assert_eq!(bytes[0], kind::CC_SEAT);
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
        let f = fact(kind::TREASURY, 0, 0, 0);
        assert_eq!(f.leaf_hash().len(), 32);
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let f = fact(kind::CC_SEAT, 0x11, 100, 200);
        assert_eq!(f.leaf_hash(), f.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_kind_change() {
        let a = fact(kind::TREASURY, 0x11, 100, 200);
        let b = fact(kind::CC_SEAT, 0x11, 100, 200);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_key_change() {
        let a = fact(kind::TREASURY, 0x11, 100, 200);
        let b = fact(kind::TREASURY, 0x12, 100, 200);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_value_change() {
        let a = fact(kind::TREASURY, 0x11, 100, 200);
        let b = fact(kind::TREASURY, 0x11, 101, 200);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_slot_change() {
        let a = fact(kind::TREASURY, 0x11, 100, 200);
        let b = fact(kind::TREASURY, 0x11, 100, 201);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn account_state_leaf_hash_distinct_from_legacy_kinds() {
        let acc = GovernanceFact::AccountState {
            reserves: 100,
            treasury: 200,
            deposits: 300,
            fee_pot: 400,
        };
        let legacy = fact(kind::TREASURY, 0x11, 100, 200);
        assert_ne!(acc.leaf_hash(), legacy.leaf_hash());
    }

    #[test]
    fn account_state_leaf_hash_changes_on_pot_change() {
        let a = GovernanceFact::AccountState {
            reserves: 1,
            treasury: 2,
            deposits: 3,
            fee_pot: 4,
        };
        let b = GovernanceFact::AccountState {
            reserves: 1,
            treasury: 2,
            deposits: 3,
            fee_pot: 5,
        };
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn all_five_kinds_distinct_leaves() {
        let leaves: Vec<Hash> = vec![
            fact(kind::TREASURY, 0x11, 100, 200).leaf_hash(),
            fact(kind::CC_SEAT, 0x11, 100, 200).leaf_hash(),
            fact(kind::RATIFIED_ACTION, 0x11, 100, 200).leaf_hash(),
            fact(kind::IN_FLIGHT_ACTION, 0x11, 100, 200).leaf_hash(),
            GovernanceFact::AccountState {
                reserves: 100,
                treasury: 200,
                deposits: 300,
                fee_pot: 400,
            }
            .leaf_hash(),
        ];
        for i in 0..leaves.len() {
            for j in (i + 1)..leaves.len() {
                assert_ne!(leaves[i], leaves[j], "kind {} vs {} collided", i, j);
            }
        }
    }

    #[test]
    fn u128_max_value_encodes_correctly() {
        let f = fact(kind::TREASURY, 0, u128::MAX, 0);
        let bytes = f.encode();
        assert_eq!(&bytes[33..49], &[0xFFu8; 16]);
    }

    #[test]
    fn validate_keys_unique_per_kind_accepts_unique() {
        let entries = vec![
            fact(kind::TREASURY, 0x01, 100, 200),
            fact(kind::CC_SEAT, 0x02, 100, 200),
            fact(kind::RATIFIED_ACTION, 0x03, 100, 200),
        ];
        assert_eq!(validate_governance_keys_unique_per_kind(&entries), None);
    }

    #[test]
    fn validate_keys_unique_per_kind_finds_duplicate() {
        let entries = vec![
            fact(kind::TREASURY, 0x01, 100, 200),
            fact(kind::CC_SEAT, 0x01, 200, 300),
            fact(kind::TREASURY, 0x01, 999, 999), // duplicate (kind=0, key=0x01)
        ];
        assert_eq!(validate_governance_keys_unique_per_kind(&entries), Some(2));
    }

    #[test]
    fn validate_keys_unique_per_kind_allows_same_key_different_kind() {
        // Same key 0x01 used for kind=Treasury and kind=RatifiedAction —
        // should be allowed.
        let entries = vec![
            fact(kind::TREASURY, 0x01, 100, 200),
            fact(kind::RATIFIED_ACTION, 0x01, 200, 300),
        ];
        assert_eq!(validate_governance_keys_unique_per_kind(&entries), None);
    }

    #[test]
    fn validate_keys_unique_per_kind_rejects_duplicate_account_state() {
        let entries = vec![
            GovernanceFact::AccountState {
                reserves: 1,
                treasury: 2,
                deposits: 3,
                fee_pot: 4,
            },
            GovernanceFact::AccountState {
                reserves: 5,
                treasury: 6,
                deposits: 7,
                fee_pot: 8,
            },
        ];
        assert_eq!(validate_governance_keys_unique_per_kind(&entries), Some(1));
    }

    #[test]
    fn validate_keys_unique_per_kind_empty_is_valid() {
        assert_eq!(validate_governance_keys_unique_per_kind(&[]), None);
    }
}
