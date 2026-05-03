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
use omega_commitment_core::utxo_leaf::Utxo;
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
///     [ tx_id (32 bytes), output_index (u64), address (32 bytes),
///       value_lovelace (u64) ]
///   or a 6-element v0.9+ array that appends:
///     [ multi_assets, script_credential ]
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
        let address_hash = read_32_bytes(&mut d)?;
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
            address_hash,
            value_lovelace,
            assets,
            datum_hash: None,
        });
    }
    expect_end(&d, cbor.len())?;
    Ok(UtxoOutput { utxos })
}

/// Parse a multi-asset bundle (a CBOR map: `{ policy_28 => { name => u64 } }`)
/// into a flat `Vec<Asset>`. Each `(policy_id, asset_name)` pair becomes one
/// `Asset` with `asset_id = policy_id || asset_name` (concatenated bytes per
/// the canonical Cardano native-asset identifier convention).
fn parse_multi_assets(
    d: &mut pallas_codec::minicbor::Decoder<'_>,
) -> anyhow::Result<Vec<omega_commitment_core::utxo_leaf::Asset>> {
    use omega_commitment_core::utxo_leaf::Asset;
    let mut assets = Vec::new();
    let n_policies = read_map_len(d)?;
    for _ in 0..n_policies {
        let policy: [u8; 28] = read_28_bytes(d)?;
        let n_assets = read_map_len(d)?;
        for _ in 0..n_assets {
            let name: Vec<u8> = read_var_bytes(d)?;
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
        assert_eq!(out.utxos[2].value_lovelace, 250_000_000);
        assert!(out.utxos.iter().all(|u| u.assets.is_empty()));
        assert!(out.utxos.iter().all(|u| u.datum_hash.is_none()));
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
        assert_eq!(out.utxos[0].address_hash, [0x22; 32]);
        assert_eq!(out.utxos[0].value_lovelace, 1_000_000);
        assert!(out.utxos[0].assets.is_empty());
        assert!(out.utxos[0].datum_hash.is_none());
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
}
