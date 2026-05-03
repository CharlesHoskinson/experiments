//! Script-registry sub-tree ingestion: shipped (v0.9.x synthetic, v1.0
//! mainnet pending Task 4 of `2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`).
//!
//! Walks each UTXO's optional `script_credential` field, deduplicates by
//! `script_hash`, and emits the script-registry leaf entries. Output is
//! sorted by `script_hash` for stability.
//!
//! `deployment_slot` is pinned to `0` (the synthetic fixture does not
//! carry per-script deployment timing; real-data ingestion will pull
//! this from chain history in v1.0).

use crate::cbor::{
    expect_end, read_28_bytes, read_32_bytes, read_array_len, read_map_len, read_null_marker,
    read_u32, read_u64, read_u8, read_var_bytes,
};
use crate::IngestError;
use omega_commitment_core::script_registry_leaf::ScriptEntry;
use pallas_codec::minicbor::Decoder;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct ScriptOutput {
    pub scripts: Vec<ScriptEntry>,
}

/// Ingest script-registry entries by walking the extended UTXO
/// fixture's per-UTXO `script_credential` field. Deduplicates by
/// `script_hash`; if the same hash appears with different metadata
/// across UTXOs, the first occurrence wins.
pub fn ingest_scripts(cbor: &[u8]) -> Result<ScriptOutput, IngestError> {
    let mut seen: BTreeMap<[u8; 28], ScriptEntry> = BTreeMap::new();
    let mut d = Decoder::new(cbor);
    let n = read_array_len(&mut d)?;
    for _ in 0..n {
        let arity = read_array_len(&mut d)?;
        if arity != 4 && arity != 6 {
            return Err(IngestError::schema(
                "script.utxo_entry",
                format!("utxo entry must be 4-elem or 6-elem, got {arity}"),
            ));
        }
        let _tx_id = read_32_bytes(&mut d)?;
        let _out_idx = read_u64(&mut d)?;
        let _addr = read_32_bytes(&mut d)?;
        let _value = read_u64(&mut d)?;
        if arity == 4 {
            continue;
        }
        // Skip multi-assets first, then read script credential.
        let n_policies = read_map_len(&mut d)?;
        for _ in 0..n_policies {
            let _policy: [u8; 28] = read_28_bytes(&mut d)?;
            let n_assets = read_map_len(&mut d)?;
            for _ in 0..n_assets {
                let _name: Vec<u8> = read_var_bytes(&mut d)?;
                let _qty: u64 = read_u64(&mut d)?;
            }
        }
        if read_null_marker(&mut d)? {
            continue;
        }
        let arity = read_array_len(&mut d)?;
        if arity != 3 {
            return Err(IngestError::schema(
                "script.credential",
                format!("script_credential arity {arity} != 3"),
            ));
        }
        let script_hash: [u8; 28] = read_28_bytes(&mut d)?;
        let language: u8 = read_u8(&mut d)?;
        let script_size_bytes: u32 = read_u32(&mut d)?;
        seen.entry(script_hash).or_insert(ScriptEntry {
            script_hash,
            deployment_slot: 0,
            script_size_bytes,
            language,
        });
    }
    expect_end(&d, cbor.len())?;
    Ok(ScriptOutput {
        scripts: seen.into_values().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extended_fixture_bytes() -> Vec<u8> {
        std::fs::read("tests/fixtures/ledger_state_extended.cbor").unwrap()
    }

    #[test]
    fn ingest_extended_fixture_yields_two_scripts() {
        let out = ingest_scripts(&extended_fixture_bytes()).unwrap();
        assert_eq!(out.scripts.len(), 2);
        // Sorted by script_hash, so script_one (0xCC…) precedes script_two (0xDD…).
        assert_eq!(out.scripts[0].script_hash, [0xCC; 28]);
        assert_eq!(out.scripts[1].script_hash, [0xDD; 28]);
    }

    #[test]
    fn script_metadata_preserved() {
        let out = ingest_scripts(&extended_fixture_bytes()).unwrap();
        // script_one: Plutus V2 (language=2), 1024 bytes
        assert_eq!(out.scripts[0].language, 2);
        assert_eq!(out.scripts[0].script_size_bytes, 1024);
        // script_two: native multi-sig (language=0), 256 bytes
        assert_eq!(out.scripts[1].language, 0);
        assert_eq!(out.scripts[1].script_size_bytes, 256);
    }

    #[test]
    fn deployment_slot_pinned_to_zero() {
        let out = ingest_scripts(&extended_fixture_bytes()).unwrap();
        for s in &out.scripts {
            assert_eq!(s.deployment_slot, 0);
        }
    }

    #[test]
    fn minimal_fixture_yields_zero_scripts() {
        let cbor = std::fs::read("tests/fixtures/ledger_state_minimal.cbor").unwrap();
        let out = ingest_scripts(&cbor).unwrap();
        assert!(out.scripts.is_empty());
    }

    #[test]
    fn deterministic_across_runs() {
        let cbor = extended_fixture_bytes();
        let a = ingest_scripts(&cbor).unwrap();
        let b = ingest_scripts(&cbor).unwrap();
        let a_json = serde_json::to_string(&a).unwrap();
        let b_json = serde_json::to_string(&b).unwrap();
        assert_eq!(a_json, b_json);
    }

    #[test]
    fn ingest_rejects_trailing_garbage() {
        let cbor_buf = std::fs::read("tests/fixtures/ledger_state_extended.cbor").unwrap();
        let mut tampered = cbor_buf.clone();
        tampered.push(0xFF); // trailing byte
        let result = ingest_scripts(&tampered);
        assert!(result.is_err(), "trailing byte must be rejected");
    }
}
