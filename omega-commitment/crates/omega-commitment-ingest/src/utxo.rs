//! UTXO sub-tree ingestion: simplified CBOR fixture → omega-commitment
//! UTXO list → JSON output for `omega-commitment commit --sub-tree utxo`.
//!
//! Reads the hand-crafted fixture format documented in
//! `tests/fixtures/ledger_state_minimal.cbor.md`. Real Mithril/Cardano
//! LedgerState parsing is future work; the simplified format proves
//! the ingestion → JSON → leaf-hash pipeline.

use crate::cbor::{
    expect_end, read_28_bytes, read_32_bytes, read_array_len, read_map_len, read_null_marker,
    read_u32, read_u64, read_u8, read_var_bytes,
};
use anyhow::Result;
use omega_commitment_core::utxo_leaf::{DatumOption, Utxo};
use pallas_codec::minicbor::Decoder;
use serde::Serialize;

/// JSON output shape that matches the input format consumed by
/// `omega-commitment commit --sub-tree utxo`.
#[derive(Debug, Clone, Serialize)]
pub struct UtxoOutput {
    pub utxos: Vec<Utxo>,
}

/// Ingest UTXOs from the simplified CBOR fixture.
///
/// Fixture format (Conway-era LedgerState parsing is future work):
///   CBOR array of N UTXOs, each a 4-element array of:
///     [ tx_id (32 bytes), output_index (u64), address (variable bytes),
///       value_lovelace (u64) ]
///   or a 6-element v0.9+ array that appends:
///     [ multi_assets, script_credential ]
///
/// `address` is the raw Cardano address payload per CIP-19; the leading
/// header byte is preserved verbatim. The fixture historically wrote a
/// 32-byte placeholder there — that placeholder is still accepted as
/// an opaque address when the fixture predates Batch 2.
pub fn ingest_utxos(cbor: &[u8]) -> Result<UtxoOutput> {
    let mut d = Decoder::new(cbor);
    let n = read_array_len(&mut d)?;
    let mut utxos = Vec::with_capacity(n);
    for _ in 0..n {
        let arity = read_array_len(&mut d)?;
        if arity != 4 && arity != 6 {
            return Err(anyhow::anyhow!(
                "utxo entry must be 4-elem (v0.8 minimal) or 6-elem (v0.9 extended), got {arity}"
            ));
        }
        let tx_id = read_32_bytes(&mut d)?;
        let output_index = u32::try_from(read_u64(&mut d)?)
            .map_err(|_| anyhow::anyhow!("output_index too large for u32"))?;
        let address = read_var_bytes(&mut d)?;
        let value_lovelace = read_u64(&mut d)?;
        let mut assets = Vec::new();
        if arity == 6 {
            // Parse multi_assets into Vec<Asset> (preserves native assets per
            // spec §9.1) and skip script_credential (CBOR null or 3-element
            // array, consumed by the script ingestion path).
            assets = parse_multi_assets(&mut d)?;
            skip_script_credential(&mut d)?;
        }
        utxos.push(Utxo {
            tx_id,
            output_index,
            address,
            value_lovelace,
            assets,
            datum_option: DatumOption::None,
            script_ref: None,
        });
    }
    expect_end(&d, cbor.len())?;
    Ok(UtxoOutput { utxos })
}

/// Parse a multi-asset bundle (a CBOR map: `{ policy_28 => { name => u64 } }`)
/// into a flat `Vec<Asset>`. Each `(policy_id, asset_name)` pair becomes one
/// `Asset` with `asset_id = policy_id || asset_name` (concatenated bytes per
/// the canonical Cardano native-asset identifier convention).
///
/// **Canonicality.** Both the outer policy map and the inner asset-name
/// map MUST be sorted ascending by raw key bytes and unique. Duplicate
/// or out-of-order keys are rejected — Cardano CBOR snapshots are
/// produced in sorted form, and accepting a non-canonical input would
/// admit two byte-different inputs that hash to the same omega leaf.
/// (Closes audit finding A3/F005; will be typified into
/// `IngestError::NonCanonicalAssetMap` in Batch 5.)
///
/// Asset-name bytes are preserved verbatim — no UTF-8 normalisation —
/// because Cardano asset names are arbitrary byte strings and
/// re-encoding them would silently change the asset identity.
fn parse_multi_assets(
    d: &mut pallas_codec::minicbor::Decoder<'_>,
) -> anyhow::Result<Vec<omega_commitment_core::utxo_leaf::Asset>> {
    use omega_commitment_core::utxo_leaf::Asset;
    let mut assets = Vec::new();
    let n_policies = read_map_len(d)?;
    let mut last_policy: Option<[u8; 28]> = None;
    for _ in 0..n_policies {
        let policy: [u8; 28] = read_28_bytes(d)?;
        if let Some(prev) = last_policy {
            if policy <= prev {
                return Err(anyhow::anyhow!(
                    "non-canonical asset map: policy_id {} not strictly greater than previous {}",
                    hex::encode(policy),
                    hex::encode(prev)
                ));
            }
        }
        last_policy = Some(policy);
        let n_assets = read_map_len(d)?;
        let mut last_name: Option<Vec<u8>> = None;
        for _ in 0..n_assets {
            let name: Vec<u8> = read_var_bytes(d)?;
            if let Some(prev) = &last_name {
                if name.as_slice() <= prev.as_slice() {
                    return Err(anyhow::anyhow!(
                        "non-canonical asset map: asset_name {} not strictly greater than previous {} under policy {}",
                        hex::encode(&name),
                        hex::encode(prev),
                        hex::encode(policy)
                    ));
                }
            }
            last_name = Some(name.clone());
            let qty: u64 = read_u64(d)?;
            // asset_id = policy_id (28 bytes) || asset_name (variable).
            let mut asset_id = Vec::with_capacity(28 + name.len());
            asset_id.extend_from_slice(&policy);
            asset_id.extend_from_slice(&name);
            assets.push(Asset {
                asset_id,
                quantity: qty,
            });
        }
    }
    Ok(assets)
}

/// Skip a script credential, which is either CBOR null or a 3-element
/// array [script_hash_28, language_u8, script_size_u32].
fn skip_script_credential(d: &mut pallas_codec::minicbor::Decoder<'_>) -> anyhow::Result<()> {
    if read_null_marker(d)? {
        return Ok(());
    }
    let arity = read_array_len(d)?;
    if arity != 3 {
        return Err(anyhow::anyhow!(
            "script_credential array must be 3-elem, got {arity}"
        ));
    }
    let _hash: [u8; 28] = read_28_bytes(d)?;
    let _language: u8 = read_u8(d)?;
    let _size: u32 = read_u32(d)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_minimal_cbor() -> Vec<u8> {
        // Same encoding as the test fixture (Task 5).
        // Three UTXOs with deterministic content.
        fn cbor_array_header(len: usize) -> Vec<u8> {
            if len < 24 {
                vec![0x80u8 | len as u8]
            } else if len < 256 {
                vec![0x98, len as u8]
            } else {
                vec![0x99, (len >> 8) as u8, len as u8]
            }
        }
        fn cbor_bytes_header(len: usize) -> Vec<u8> {
            if len < 24 {
                vec![0x40u8 | len as u8]
            } else if len < 256 {
                vec![0x58, len as u8]
            } else {
                vec![0x59, (len >> 8) as u8, len as u8]
            }
        }
        fn cbor_uint(v: u64) -> Vec<u8> {
            if v < 24 {
                vec![v as u8]
            } else if v <= 0xff {
                vec![0x18, v as u8]
            } else if v <= 0xffff {
                vec![0x19, (v >> 8) as u8, v as u8]
            } else if v <= 0xffff_ffff {
                let mut o = vec![0x1a];
                o.extend_from_slice(&(v as u32).to_be_bytes());
                o
            } else {
                let mut o = vec![0x1b];
                o.extend_from_slice(&v.to_be_bytes());
                o
            }
        }
        fn cbor_bytes(b: &[u8]) -> Vec<u8> {
            let mut o = cbor_bytes_header(b.len());
            o.extend_from_slice(b);
            o
        }
        fn utxo(tx_id: [u8; 32], oi: u64, addr: [u8; 32], v: u64) -> Vec<u8> {
            let mut o = cbor_array_header(4);
            o.extend(cbor_bytes(&tx_id));
            o.extend(cbor_uint(oi));
            o.extend(cbor_bytes(&addr));
            o.extend(cbor_uint(v));
            o
        }
        let mut buf = Vec::new();
        buf.extend(cbor_array_header(3));
        buf.extend(utxo([0x11; 32], 0, [0xAA; 32], 1_000_000));
        buf.extend(utxo([0x22; 32], 1, [0xBB; 32], 5_000_000));
        buf.extend(utxo([0x33; 32], 2, [0xCC; 32], 250_000_000));
        buf
    }

    #[test]
    fn ingest_minimal_fixture() {
        let cbor = make_minimal_cbor();
        let out = ingest_utxos(&cbor).unwrap();
        assert_eq!(out.utxos.len(), 3);
        assert_eq!(out.utxos[0].tx_id, [0x11; 32]);
        assert_eq!(out.utxos[0].output_index, 0);
        assert_eq!(out.utxos[0].value_lovelace, 1_000_000);
        // Address byte length matches the fixture (32 bytes of 0xAA).
        assert_eq!(out.utxos[0].address, vec![0xAAu8; 32]);
        assert_eq!(out.utxos[2].value_lovelace, 250_000_000);
        assert!(out.utxos.iter().all(|u| u.assets.is_empty()));
        assert!(out
            .utxos
            .iter()
            .all(|u| matches!(u.datum_option, DatumOption::None)));
        assert!(out.utxos.iter().all(|u| u.script_ref.is_none()));
    }

    #[test]
    fn ingest_then_leaf_hashes_are_deterministic() {
        let cbor = make_minimal_cbor();
        let out1 = ingest_utxos(&cbor).unwrap();
        let out2 = ingest_utxos(&cbor).unwrap();
        let h1: Vec<_> = out1.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
        let h2: Vec<_> = out2.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
        assert_eq!(h1, h2);
    }

    #[test]
    fn ingest_truncated_input_fails() {
        let cbor = make_minimal_cbor();
        let truncated = &cbor[..cbor.len() / 2];
        assert!(ingest_utxos(truncated).is_err());
    }

    #[test]
    fn ingest_extended_fixture_path_yields_same_utxo_shape() {
        // Build a minimal extended (6-elem) fixture inline and confirm
        // the UTXO sub-tree output is identical to what the 4-elem path
        // would produce — extension fields are skipped at the UTXO layer.
        fn cbor_array_header(len: usize) -> Vec<u8> {
            if len < 24 {
                vec![0x80u8 | len as u8]
            } else if len < 256 {
                vec![0x98, len as u8]
            } else {
                vec![0x99, (len >> 8) as u8, len as u8]
            }
        }
        fn cbor_bytes_header(len: usize) -> Vec<u8> {
            if len < 24 {
                vec![0x40u8 | len as u8]
            } else if len < 256 {
                vec![0x58, len as u8]
            } else {
                vec![0x59, (len >> 8) as u8, len as u8]
            }
        }
        fn cbor_uint(v: u64) -> Vec<u8> {
            if v < 24 {
                vec![v as u8]
            } else if v <= 0xff {
                vec![0x18, v as u8]
            } else if v <= 0xffff {
                vec![0x19, (v >> 8) as u8, v as u8]
            } else if v <= 0xffff_ffff {
                let mut o = vec![0x1a];
                o.extend_from_slice(&(v as u32).to_be_bytes());
                o
            } else {
                let mut o = vec![0x1b];
                o.extend_from_slice(&v.to_be_bytes());
                o
            }
        }
        fn cbor_bytes(b: &[u8]) -> Vec<u8> {
            let mut o = cbor_bytes_header(b.len());
            o.extend_from_slice(b);
            o
        }
        // One extended UTXO with empty multi-assets and null script credential.
        let mut buf = Vec::new();
        buf.extend(cbor_array_header(1));
        buf.extend(cbor_array_header(6));
        buf.extend(cbor_bytes(&[0x11; 32]));
        buf.extend(cbor_uint(0));
        buf.extend(cbor_bytes(&[0x22; 32]));
        buf.extend(cbor_uint(1_000_000));
        buf.push(0xA0); // empty map
        buf.push(0xF6); // null

        let out = ingest_utxos(&buf).unwrap();
        assert_eq!(out.utxos.len(), 1);
        assert_eq!(out.utxos[0].tx_id, [0x11; 32]);
        assert_eq!(out.utxos[0].output_index, 0);
        assert_eq!(out.utxos[0].address, vec![0x22u8; 32]);
        assert_eq!(out.utxos[0].value_lovelace, 1_000_000);
        assert!(out.utxos[0].assets.is_empty());
        assert!(matches!(out.utxos[0].datum_option, DatumOption::None));
        assert!(out.utxos[0].script_ref.is_none());
    }

    #[test]
    fn ingest_real_extended_fixture_succeeds() {
        let cbor = std::fs::read("tests/fixtures/ledger_state_extended.cbor")
            .expect("extended fixture readable");
        let out = ingest_utxos(&cbor).unwrap();
        assert_eq!(out.utxos.len(), 4);
        // UTXO base fields are independent of multi-assets/script_credential.
        assert_eq!(out.utxos[0].value_lovelace, 1_000_000);
        assert_eq!(out.utxos[1].value_lovelace, 5_000_000);
        assert_eq!(out.utxos[2].value_lovelace, 25_000_000);
        assert_eq!(out.utxos[3].value_lovelace, 10_000_000);
    }

    #[test]
    fn ingest_rejects_5_elem_arity() {
        // 5-element UTXO is not a recognized format.
        let buf = vec![0x81, 0x85, 0x40, 0x40, 0x40, 0x40, 0x40];
        assert!(ingest_utxos(&buf).is_err());
    }

    #[test]
    fn extended_fixture_preserves_multi_assets_in_utxos() {
        let cbor = std::fs::read("tests/fixtures/ledger_state_extended.cbor").unwrap();
        let out = ingest_utxos(&cbor).unwrap();
        assert_eq!(out.utxos.len(), 4);
        // UTXO 0: bare, no assets.
        assert!(out.utxos[0].assets.is_empty());
        // UTXO 1: one policy, one asset (COIN qty=100).
        assert_eq!(out.utxos[1].assets.len(), 1);
        assert_eq!(out.utxos[1].assets[0].quantity, 100);
        // UTXO 2: two policies, three assets total (COIN+NFT under policy_a, TOKEN under policy_b).
        assert_eq!(out.utxos[2].assets.len(), 3);
        let total_qty: u64 = out.utxos[2].assets.iter().map(|a| a.quantity).sum();
        assert_eq!(total_qty, 50 + 1 + 999);
        // UTXO 3: bare, no assets (script credential present but no multi-asset).
        assert!(out.utxos[3].assets.is_empty());
    }

    #[test]
    fn ingest_rejects_trailing_garbage() {
        let cbor_buf = std::fs::read("tests/fixtures/ledger_state_extended.cbor").unwrap();
        let mut tampered = cbor_buf.clone();
        tampered.push(0xFF); // trailing byte
        let result = ingest_utxos(&tampered);
        assert!(result.is_err(), "trailing byte must be rejected");
    }

    #[test]
    fn parse_multi_assets_rejects_out_of_order_policies() {
        // CBOR for { policy_FF: {}, policy_00: {} } — policies in
        // descending order. The outer policy map must be ascending.
        // Build a minimal extended UTXO that wraps this map.
        fn cbor_bytes(b: &[u8]) -> Vec<u8> {
            let mut o = vec![0x58u8, b.len() as u8];
            o.extend_from_slice(b);
            o
        }
        let mut policy_hi = [0u8; 28];
        policy_hi[0] = 0xFF;
        let mut policy_lo = [0u8; 28];
        policy_lo[0] = 0x00;

        let mut multi_asset = vec![0xA2u8]; // map of 2
        multi_asset.extend(cbor_bytes(&policy_hi));
        multi_asset.push(0xA0); // empty inner map
        multi_asset.extend(cbor_bytes(&policy_lo));
        multi_asset.push(0xA0);

        let mut buf = vec![0x81u8, 0x86u8]; // outer array of 1, inner UTXO of 6
        buf.extend(cbor_bytes(&[0x11u8; 32])); // tx_id
        buf.push(0x00); // output_index = 0
        buf.extend(cbor_bytes(&[0x22u8; 32])); // address (placeholder)
        buf.push(0x00); // value = 0
        buf.extend(multi_asset);
        buf.push(0xF6); // null script_credential

        let err = ingest_utxos(&buf).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("non-canonical asset map") && msg.contains("policy_id"),
            "expected non-canonical asset-map error, got {msg}"
        );
    }

    #[test]
    fn parse_multi_assets_rejects_duplicate_policies() {
        fn cbor_bytes(b: &[u8]) -> Vec<u8> {
            let mut o = vec![0x58u8, b.len() as u8];
            o.extend_from_slice(b);
            o
        }
        let policy = [0x10u8; 28];

        let mut multi_asset = vec![0xA2u8]; // map of 2
        multi_asset.extend(cbor_bytes(&policy));
        multi_asset.push(0xA0);
        multi_asset.extend(cbor_bytes(&policy));
        multi_asset.push(0xA0);

        let mut buf = vec![0x81u8, 0x86u8];
        buf.extend(cbor_bytes(&[0x11u8; 32]));
        buf.push(0x00);
        buf.extend(cbor_bytes(&[0x22u8; 32]));
        buf.push(0x00);
        buf.extend(multi_asset);
        buf.push(0xF6);

        let err = ingest_utxos(&buf).unwrap_err();
        assert!(format!("{err}").contains("non-canonical asset map"));
    }

    #[test]
    fn parse_multi_assets_rejects_out_of_order_asset_names() {
        fn cbor_bytes(b: &[u8]) -> Vec<u8> {
            let mut o = vec![0x58u8, b.len() as u8];
            o.extend_from_slice(b);
            o
        }
        let policy = [0x10u8; 28];
        // Inner map: { name_HI => 1, name_LO => 1 }
        let mut inner = vec![0xA2u8];
        inner.push(0x42);
        inner.extend_from_slice(&[0xFF, 0xFF]);
        inner.push(0x01);
        inner.push(0x42);
        inner.extend_from_slice(&[0x00, 0x00]);
        inner.push(0x01);

        let mut multi_asset = vec![0xA1u8];
        multi_asset.extend(cbor_bytes(&policy));
        multi_asset.extend(inner);

        let mut buf = vec![0x81u8, 0x86u8];
        buf.extend(cbor_bytes(&[0x11u8; 32]));
        buf.push(0x00);
        buf.extend(cbor_bytes(&[0x22u8; 32]));
        buf.push(0x00);
        buf.extend(multi_asset);
        buf.push(0xF6);

        let err = ingest_utxos(&buf).unwrap_err();
        assert!(format!("{err}").contains("non-canonical asset map"));
    }
}
