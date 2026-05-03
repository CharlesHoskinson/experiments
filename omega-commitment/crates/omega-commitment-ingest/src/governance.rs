//! Governance-state sub-tree ingestion from a hand-crafted
//! governance_snapshot.cbor.
//!
//! Top-level CBOR array of 4-element entries:
//!   [ kind (u8), key (32 bytes), value (16-byte u128 big-endian),
//!     slot (u64) ]
//!
//! Maps 1:1 onto `omega_commitment_core::governance_state_leaf::GovernanceFact`.

use crate::cbor::{expect_end, read_32_bytes, read_array_len, read_u128_bytes, read_u64, read_u8};
use anyhow::Result;
use omega_commitment_core::governance_state_leaf::GovernanceFact;
use pallas_codec::minicbor::Decoder;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GovernanceOutput {
    pub facts: Vec<GovernanceFact>,
}

pub fn ingest_governance(cbor: &[u8]) -> Result<GovernanceOutput> {
    let mut d = Decoder::new(cbor);
    let n = read_array_len(&mut d)?;
    let mut facts = Vec::with_capacity(n);
    for _ in 0..n {
        let arity = read_array_len(&mut d)?;
        if arity != 4 {
            return Err(anyhow::anyhow!(
                "governance fact must be 4-elem, got {arity}"
            ));
        }
        let kind = read_u8(&mut d)?;
        let key = read_32_bytes(&mut d)?;
        let value = read_u128_bytes(&mut d)?;
        let slot = read_u64(&mut d)?;
        facts.push(GovernanceFact {
            kind,
            key,
            value,
            slot,
        });
    }
    expect_end(&d, cbor.len())?;
    Ok(GovernanceOutput { facts })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        std::fs::read("tests/fixtures/governance_snapshot.cbor").unwrap()
    }

    #[test]
    fn ingest_yields_four_facts() {
        let out = ingest_governance(&fixture()).unwrap();
        assert_eq!(out.facts.len(), 4);
    }

    #[test]
    fn all_four_kinds_present() {
        let out = ingest_governance(&fixture()).unwrap();
        let kinds: std::collections::HashSet<u8> = out.facts.iter().map(|f| f.kind).collect();
        for k in 0..=3 {
            assert!(kinds.contains(&k));
        }
    }

    #[test]
    fn treasury_fact_decoded_correctly() {
        let out = ingest_governance(&fixture()).unwrap();
        let treasury = out.facts.iter().find(|f| f.kind == 0).unwrap();
        assert_eq!(treasury.key, [0u8; 32]);
        assert_eq!(treasury.value, 1_700_000_000_000_000u128);
        assert_eq!(treasury.slot, 100_000);
    }

    #[test]
    fn deterministic_across_runs() {
        let cbor = fixture();
        let a = ingest_governance(&cbor).unwrap();
        let b = ingest_governance(&cbor).unwrap();
        let a_json = serde_json::to_string(&a).unwrap();
        let b_json = serde_json::to_string(&b).unwrap();
        assert_eq!(a_json, b_json);
    }

    #[test]
    fn rejects_wrong_arity() {
        let buf = vec![0x81, 0x82, 0x00, 0x40];
        assert!(ingest_governance(&buf).is_err());
    }

    #[test]
    fn ingest_rejects_trailing_garbage() {
        let cbor_buf = std::fs::read("tests/fixtures/governance_snapshot.cbor").unwrap();
        let mut tampered = cbor_buf.clone();
        tampered.push(0xFF); // trailing byte
        let result = ingest_governance(&tampered);
        assert!(result.is_err(), "trailing byte must be rejected");
    }
}
