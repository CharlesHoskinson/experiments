//! Canonical UTXO leaf encoding (Conway-era Cardano semantic fidelity).
//!
//! A UTXO leaf is the deterministic serialization of:
//!   (tx_id: 32 bytes) || (output_index: u32 BE) ||
//!   (address_len: u16 BE) || (address: variable) ||
//!   (value_lovelace: u64 BE) ||
//!   (asset_count: u32 BE) || ((id_len: u16 BE) || asset_id || (quantity: u64 BE))* ||
//!   (datum_option_tag: u8) || (datum_payload: variable) ||
//!   (script_ref_tag: u8) || (script_ref_payload: variable)
//!
//! The leaf is then hashed with Blake2b-256 to produce the leaf hash.
//!
//! ## Address bytes (CIP-19)
//!
//! `address` is the raw canonical Cardano address bytes per the CDDL:
//! the leading discriminator byte (CIP-19 header byte) is preserved
//! verbatim. The encoder length-prefixes with `u16` BE so addresses up
//! to 65535 bytes can be represented; mainnet addresses are at most
//! 57 bytes (Byron 76).
//!
//! ## datum_option (Conway era)
//!
//! Tag byte:
//!   - `0x00` = None              (no datum payload follows)
//!   - `0x01` = DatumHash         (32-byte hash payload follows)
//!   - `0x02` = InlineDatum       (`u32` BE length || bytes follow)
//!
//! ## script_ref (Conway era)
//!
//! Optional. Outer tag byte:
//!   - `0x00` = None              (no script ref payload follows)
//!   - `0x01` = Some              (language tag + length-prefixed bytes follow)
//!
//! When present, language tag:
//!   - `0x01` = Native
//!   - `0x02` = Plutus V1
//!   - `0x03` = Plutus V2
//!   - `0x04` = Plutus V3
//!
//! Followed by `u32` BE length and the raw script bytes.

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
    #[error("address length exceeds u16::MAX")]
    AddressLenOverflow,
    #[error("inline datum length exceeds u32::MAX")]
    InlineDatumLenOverflow,
    #[error("script_ref length exceeds u32::MAX")]
    ScriptRefLenOverflow,
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

/// Conway-era datum_option (CDDL: `[0, $hash32 // 1, data]`).
///
/// `Hash` retains the legacy datum-hash referencing path; `Inline` is
/// the inline-datum bytes Conway introduced. `None` means no datum at
/// all.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DatumOption {
    #[default]
    None,
    Hash {
        #[serde(with = "hex::serde")]
        hash: [u8; 32],
    },
    Inline {
        #[serde(with = "hex::serde")]
        data: Vec<u8>,
    },
}

/// Conway-era script_ref language tag (CDDL: `script` discriminant).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScriptLanguage {
    Native,
    PlutusV1,
    PlutusV2,
    PlutusV3,
}

impl ScriptLanguage {
    fn tag_byte(self) -> u8 {
        match self {
            ScriptLanguage::Native => 0x01,
            ScriptLanguage::PlutusV1 => 0x02,
            ScriptLanguage::PlutusV2 => 0x03,
            ScriptLanguage::PlutusV3 => 0x04,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScriptRef {
    pub language: ScriptLanguage,
    #[serde(with = "hex::serde")]
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Utxo {
    #[serde(with = "hex::serde")]
    pub tx_id: [u8; 32],
    pub output_index: u32,
    /// Raw Cardano address bytes per CIP-19 (the discriminator/header
    /// byte is preserved as the first byte). Length-prefixed in the
    /// canonical encoding so any address shape (Byron, Shelley, etc.)
    /// round-trips losslessly.
    #[serde(with = "hex::serde")]
    pub address: Vec<u8>,
    pub value_lovelace: u64,
    pub assets: Vec<Asset>,
    #[serde(default)]
    pub datum_option: DatumOption,
    #[serde(default)]
    pub script_ref: Option<ScriptRef>,
}

impl Utxo {
    /// Canonical byte serialization.
    pub fn encode(&self) -> Result<Vec<u8>, LeafError> {
        let mut out = Vec::with_capacity(128 + self.address.len());
        out.extend_from_slice(&self.tx_id);
        out.extend_from_slice(&self.output_index.to_be_bytes());
        // Address: u16 BE length prefix + raw bytes (CIP-19 header byte
        // preserved as the first byte of the payload).
        let addr_len =
            u16::try_from(self.address.len()).map_err(|_| LeafError::AddressLenOverflow)?;
        out.extend_from_slice(&addr_len.to_be_bytes());
        out.extend_from_slice(&self.address);
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
        match &self.datum_option {
            DatumOption::None => out.push(0x00),
            DatumOption::Hash { hash } => {
                out.push(0x01);
                out.extend_from_slice(hash);
            }
            DatumOption::Inline { data } => {
                out.push(0x02);
                let n = u32::try_from(data.len()).map_err(|_| LeafError::InlineDatumLenOverflow)?;
                out.extend_from_slice(&n.to_be_bytes());
                out.extend_from_slice(data);
            }
        }
        match &self.script_ref {
            None => out.push(0x00),
            Some(sr) => {
                out.push(0x01);
                out.push(sr.language.tag_byte());
                let n =
                    u32::try_from(sr.bytes.len()).map_err(|_| LeafError::ScriptRefLenOverflow)?;
                out.extend_from_slice(&n.to_be_bytes());
                out.extend_from_slice(&sr.bytes);
            }
        }
        Ok(out)
    }

    /// Compute the legacy (untagged) leaf hash: Blake2b-256 of the
    /// canonical encoding.
    ///
    /// **Deprecated for production use.** New code should call
    /// [`Utxo::commit_to_subtree`] and feed the resulting payload into
    /// `MerkleTree::build_v1(SUB_TREE_ID_UTXO, payloads)`, which binds
    /// the `(sub_tree_id, canonical_index)` pair into every leaf hash
    /// per the v1 domain-separation spec. This method is retained only
    /// for tests, CLIs, and witness paths that have not yet been
    /// migrated to the v1 builder.
    pub fn leaf_hash(&self) -> Result<Hash, LeafError> {
        Ok(blake2b_256(&self.encode()?))
    }

    /// Return the canonical raw payload bytes that the v1 Merkle
    /// builder consumes. The v1 builder calls `leaf_hash_v1` on this
    /// payload; do NOT pre-hash here.
    pub fn commit_to_subtree(&self) -> Result<Vec<u8>, LeafError> {
        self.encode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_utxo() -> Utxo {
        Utxo {
            tx_id: [1u8; 32],
            output_index: 0,
            // Canonical Shelley mainnet address: header byte 0x61 (PaymentKeyHash, mainnet)
            // followed by a 28-byte payment-key hash. 29 bytes total.
            address: {
                let mut a = vec![0x61u8];
                a.extend_from_slice(&[0x02u8; 28]);
                a
            },
            value_lovelace: 1_000_000,
            assets: vec![],
            datum_option: DatumOption::None,
            script_ref: None,
        }
    }

    #[test]
    fn empty_assets_no_datum_no_script_ref() {
        let u = sample_utxo();
        let enc = u.encode().unwrap();
        // 32 (tx_id) + 4 (output_index) + 2 (addr_len) + 29 (addr) +
        // 8 (value) + 4 (asset_count=0) + 1 (datum tag=None) +
        // 1 (script_ref tag=None) = 81 bytes.
        assert_eq!(enc.len(), 81);
        let datum_idx = 32 + 4 + 2 + 29 + 8 + 4;
        assert_eq!(enc[datum_idx], 0x00, "datum_option None marker");
        assert_eq!(enc[datum_idx + 1], 0x00, "script_ref None marker");
    }

    #[test]
    fn datum_hash_present_marker() {
        let mut u = sample_utxo();
        u.datum_option = DatumOption::Hash { hash: [3u8; 32] };
        let enc = u.encode().unwrap();
        let datum_idx = 32 + 4 + 2 + 29 + 8 + 4;
        assert_eq!(enc[datum_idx], 0x01, "datum_option Hash marker");
        assert_eq!(&enc[datum_idx + 1..datum_idx + 33], &[3u8; 32]);
        // Trailing script_ref None.
        assert_eq!(enc[datum_idx + 33], 0x00);
    }

    #[test]
    fn inline_datum_marker_and_length_prefix() {
        let mut u = sample_utxo();
        u.datum_option = DatumOption::Inline {
            data: vec![0xAA, 0xBB, 0xCC, 0xDD],
        };
        let enc = u.encode().unwrap();
        let datum_idx = 32 + 4 + 2 + 29 + 8 + 4;
        assert_eq!(enc[datum_idx], 0x02);
        // Next 4 bytes: u32 BE length = 4
        assert_eq!(&enc[datum_idx + 1..datum_idx + 5], &4u32.to_be_bytes());
        assert_eq!(
            &enc[datum_idx + 5..datum_idx + 9],
            &[0xAA, 0xBB, 0xCC, 0xDD]
        );
        // Trailing script_ref None.
        assert_eq!(enc[datum_idx + 9], 0x00);
    }

    #[test]
    fn script_ref_marker_and_language_tags() {
        for (lang, tag) in [
            (ScriptLanguage::Native, 0x01u8),
            (ScriptLanguage::PlutusV1, 0x02u8),
            (ScriptLanguage::PlutusV2, 0x03u8),
            (ScriptLanguage::PlutusV3, 0x04u8),
        ] {
            let mut u = sample_utxo();
            u.script_ref = Some(ScriptRef {
                language: lang,
                bytes: vec![0x55, 0x66],
            });
            let enc = u.encode().unwrap();
            let sr_idx = 32 + 4 + 2 + 29 + 8 + 4 + 1; // datum_option None precedes
            assert_eq!(enc[sr_idx], 0x01, "script_ref Some marker");
            assert_eq!(enc[sr_idx + 1], tag);
            assert_eq!(&enc[sr_idx + 2..sr_idx + 6], &2u32.to_be_bytes());
            assert_eq!(&enc[sr_idx + 6..sr_idx + 8], &[0x55, 0x66]);
        }
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

    #[test]
    fn cip19_header_byte_is_committed() {
        // Two UTXOs identical except for the address discriminator byte
        // (e.g. mainnet payment-key vs script-key Shelley header) MUST
        // produce different leaf hashes — the discriminator selects the
        // address kind on Cardano and a verifier must not normalise it.
        let mut u_payment = sample_utxo();
        u_payment.address[0] = 0x61; // payment-key, mainnet
        let mut u_script = sample_utxo();
        u_script.address[0] = 0x71; // script, mainnet
        assert_ne!(
            u_payment.leaf_hash().unwrap(),
            u_script.leaf_hash().unwrap(),
            "address discriminator byte must be bound into the leaf"
        );
    }

    #[test]
    fn datum_hash_vs_inline_distinguishable() {
        let mut a = sample_utxo();
        a.datum_option = DatumOption::Hash { hash: [0u8; 32] };
        let mut b = sample_utxo();
        b.datum_option = DatumOption::Inline { data: vec![] };
        assert_ne!(a.leaf_hash().unwrap(), b.leaf_hash().unwrap());
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
