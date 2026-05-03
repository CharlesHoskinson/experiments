//! Canonical UTXO leaf encoding.
//!
//! A UTXO leaf is the deterministic serialization of:
//!   (tx_id: 32 bytes) || (output_index: u32 BE) ||
//!   (address_hash: 32 bytes) || (value_lovelace: u64 BE) ||
//!   (asset_count: u32 BE) || ((id_len: u16 BE) || asset_id || (quantity: u64 BE))* ||
//!   (datum_hash: 0x00 or 0x01 || 32 bytes)
//!
//! The leaf is then hashed with Blake2b-256 to produce the leaf hash.

use crate::hash::{blake2b_256, Hash};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LeafError {
    #[error("asset count exceeds u32::MAX")]
    AssetCountOverflow,
    #[error("asset_id length exceeds u16::MAX")]
    AssetIdLenOverflow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Asset {
    /// Canonical Cardano native-asset identifier: policy_id (28 bytes) ||
    /// asset_name (variable). The outer encoder writes this as
    /// `(id_len: u16 BE) || asset_id || (quantity: u64 BE)`, so the
    /// `asset_id` field carries raw concatenation — the outer `id_len`
    /// makes inner length-prefixing unnecessary.
    #[serde(with = "hex::serde")]
    pub asset_id: Vec<u8>,
    pub quantity: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Utxo {
    #[serde(with = "hex::serde")]
    pub tx_id: [u8; 32],
    pub output_index: u32,
    #[serde(with = "hex::serde")]
    pub address_hash: [u8; 32],
    pub value_lovelace: u64,
    pub assets: Vec<Asset>,
    #[serde(default, with = "crate::serde_helpers::opt_hex")]
    pub datum_hash: Option<[u8; 32]>,
}

impl Utxo {
    /// Canonical byte serialization.
    pub fn encode(&self) -> Result<Vec<u8>, LeafError> {
        let mut out = Vec::with_capacity(128);
        out.extend_from_slice(&self.tx_id);
        out.extend_from_slice(&self.output_index.to_be_bytes());
        out.extend_from_slice(&self.address_hash);
        out.extend_from_slice(&self.value_lovelace.to_be_bytes());
        let asset_count =
            u32::try_from(self.assets.len()).map_err(|_| LeafError::AssetCountOverflow)?;
        out.extend_from_slice(&asset_count.to_be_bytes());
        // Sort assets by asset_id for canonicality
        let mut sorted = self.assets.clone();
        sorted.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        for a in sorted {
            let id_len =
                u16::try_from(a.asset_id.len()).map_err(|_| LeafError::AssetIdLenOverflow)?;
            out.extend_from_slice(&id_len.to_be_bytes());
            out.extend_from_slice(&a.asset_id);
            out.extend_from_slice(&a.quantity.to_be_bytes());
        }
        match self.datum_hash {
            None => out.push(0x00),
            Some(d) => {
                out.push(0x01);
                out.extend_from_slice(&d);
            }
        }
        Ok(out)
    }

    /// Compute the leaf hash: Blake2b-256 of canonical encoding.
    pub fn leaf_hash(&self) -> Result<Hash, LeafError> {
        Ok(blake2b_256(&self.encode()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_utxo() -> Utxo {
        Utxo {
            tx_id: [1u8; 32],
            output_index: 0,
            address_hash: [2u8; 32],
            value_lovelace: 1_000_000,
            assets: vec![],
            datum_hash: None,
        }
    }

    #[test]
    fn empty_assets_no_datum() {
        let u = sample_utxo();
        let enc = u.encode().unwrap();
        // 32 + 4 + 32 + 8 + 4 + 0 + 1 = 81 bytes
        assert_eq!(enc.len(), 81);
        assert_eq!(enc[80], 0x00, "datum_hash absence marker");
    }

    #[test]
    fn datum_hash_present_marker() {
        let mut u = sample_utxo();
        u.datum_hash = Some([3u8; 32]);
        let enc = u.encode().unwrap();
        // 81 - 1 + 33 = 113 bytes
        assert_eq!(enc.len(), 113);
        assert_eq!(enc[80], 0x01, "datum_hash presence marker");
    }

    #[test]
    fn assets_sorted_canonically() {
        let mut u1 = sample_utxo();
        u1.assets = vec![
            Asset {
                asset_id: vec![0xff],
                quantity: 10,
            },
            Asset {
                asset_id: vec![0x00],
                quantity: 20,
            },
        ];
        let mut u2 = sample_utxo();
        u2.assets = vec![
            Asset {
                asset_id: vec![0x00],
                quantity: 20,
            },
            Asset {
                asset_id: vec![0xff],
                quantity: 10,
            },
        ];
        // Different input order, identical canonical encoding.
        assert_eq!(u1.encode().unwrap(), u2.encode().unwrap());
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let u = sample_utxo();
        let h1 = u.leaf_hash().unwrap();
        let h2 = u.leaf_hash().unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn leaf_hash_changes_with_value() {
        let u1 = sample_utxo();
        let mut u2 = sample_utxo();
        u2.value_lovelace = 999_999;
        assert_ne!(u1.leaf_hash().unwrap(), u2.leaf_hash().unwrap());
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn encoding_is_pure(
            tx_id in any::<[u8; 32]>(),
            output_index in any::<u32>(),
            value_lovelace in any::<u64>(),
        ) {
            let mut u = sample_utxo();
            u.tx_id = tx_id;
            u.output_index = output_index;
            u.value_lovelace = value_lovelace;
            let e1 = u.encode().unwrap();
            let e2 = u.encode().unwrap();
            prop_assert_eq!(e1, e2);
        }
    }
}
