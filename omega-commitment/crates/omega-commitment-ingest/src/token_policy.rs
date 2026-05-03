//! Token-policy sub-tree ingestion from the v0.9.0 extended CBOR fixture.
//!
//! Walks each UTXO's `multi_assets` map and aggregates per-`policy_id`:
//!   - `total_supply_at_h` = sum of quantities across all assets in all
//!     UTXOs that mention this policy.
//!   - `first_issuance_slot` = pinned to `0` (the simplified fixture
//!     does not carry per-policy timing data; real-data ingestion will
//!     pull this from chain history in v1.0).
//!
//! Output policies are sorted by `policy_id` to make the output stable.

use crate::cbor::{
    expect_end, read_28_bytes, read_32_bytes, read_array_len, read_map_len, read_null_marker,
    read_u32, read_u64, read_u8, read_var_bytes,
};
use anyhow::Result;
use omega_commitment_core::token_policy_leaf::TokenPolicy;
use pallas_codec::minicbor::Decoder;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct TokenPolicyOutput {
    pub policies: Vec<TokenPolicy>,
}

/// Ingest token-policy entries by walking the extended UTXO fixture's
/// per-UTXO multi-asset bundles.
///
/// Only the v0.9.0 extended (6-elem) fixture format carries multi-asset
/// data. If the input is the v0.8.0 minimal (4-elem) format, no policies
/// are emitted (the result is `policies: []`).
pub fn ingest_token_policies(cbor: &[u8]) -> Result<TokenPolicyOutput> {
    let mut totals: BTreeMap<[u8; 28], u128> = BTreeMap::new();
    let mut d = Decoder::new(cbor);
    let n = read_array_len(&mut d)?;
    for _ in 0..n {
        let arity = read_array_len(&mut d)?;
        if arity != 4 && arity != 6 {
            return Err(anyhow::anyhow!(
                "utxo entry must be 4-elem or 6-elem, got {arity}"
            ));
        }
        let _tx_id = read_32_bytes(&mut d)?;
        let _out_idx = read_u64(&mut d)?;
        let _addr = read_32_bytes(&mut d)?;
        let _value = read_u64(&mut d)?;
        if arity == 4 {
            // Minimal format carries no multi-assets; nothing to walk.
            continue;
        }
        // Extended format: walk multi_assets map, then skip script_credential.
        let n_policies = read_map_len(&mut d)?;
        for _ in 0..n_policies {
            let policy: [u8; 28] = read_28_bytes(&mut d)?;
            let n_assets = read_map_len(&mut d)?;
            let mut policy_total: u128 = 0;
            for _ in 0..n_assets {
                let _name: Vec<u8> = read_var_bytes(&mut d)?;
                let qty: u64 = read_u64(&mut d)?;
                policy_total = policy_total
                    .checked_add(qty as u128)
                    .ok_or_else(|| anyhow::anyhow!("token-policy total_supply overflow u128"))?;
            }
            let entry = totals.entry(policy).or_insert(0u128);
            *entry = entry
                .checked_add(policy_total)
                .ok_or_else(|| anyhow::anyhow!("token-policy total_supply overflow u128"))?;
        }
        // Consume script credential (null or 3-elem array).
        if !read_null_marker(&mut d)? {
            let arity = read_array_len(&mut d)?;
            if arity != 3 {
                return Err(anyhow::anyhow!("script_credential arity {arity} != 3"));
            }
            let _hash: [u8; 28] = read_28_bytes(&mut d)?;
            let _language: u8 = read_u8(&mut d)?;
            let _size: u32 = read_u32(&mut d)?;
        }
    }
    expect_end(&d, cbor.len())?;
    let policies: Vec<TokenPolicy> = totals
        .into_iter()
        .map(|(policy_id, total_supply_at_h)| TokenPolicy {
            policy_id,
            first_issuance_slot: 0,
            total_supply_at_h,
        })
        .collect();
    Ok(TokenPolicyOutput { policies })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extended_fixture_bytes() -> Vec<u8> {
        std::fs::read("tests/fixtures/ledger_state_extended.cbor").unwrap()
    }

    #[test]
    fn ingest_extended_fixture_yields_two_policies() {
        let out = ingest_token_policies(&extended_fixture_bytes()).unwrap();
        assert_eq!(out.policies.len(), 2);
        // Policies are sorted by policy_id, so policy_a (0xAA…) comes before policy_b (0xBB…).
        assert_eq!(out.policies[0].policy_id, [0xAA; 28]);
        assert_eq!(out.policies[1].policy_id, [0xBB; 28]);
    }

    #[test]
    fn total_supply_aggregated_correctly() {
        let out = ingest_token_policies(&extended_fixture_bytes()).unwrap();
        // policy_a: COIN(100) from UTXO 1 + COIN(50) + NFT(1) from UTXO 2 = 151
        assert_eq!(out.policies[0].total_supply_at_h, 151);
        // policy_b: TOKEN(999) from UTXO 2
        assert_eq!(out.policies[1].total_supply_at_h, 999);
    }

    #[test]
    fn first_issuance_slot_pinned_to_zero() {
        let out = ingest_token_policies(&extended_fixture_bytes()).unwrap();
        for p in &out.policies {
            assert_eq!(
                p.first_issuance_slot, 0,
                "synthetic-fixture limitation: pin all policies to slot 0"
            );
        }
    }

    #[test]
    fn minimal_fixture_yields_zero_policies() {
        let cbor = std::fs::read("tests/fixtures/ledger_state_minimal.cbor").unwrap();
        let out = ingest_token_policies(&cbor).unwrap();
        assert!(
            out.policies.is_empty(),
            "v0.8 minimal fixture has no multi-assets"
        );
    }

    #[test]
    fn deterministic_across_runs() {
        let cbor = extended_fixture_bytes();
        let a = ingest_token_policies(&cbor).unwrap();
        let b = ingest_token_policies(&cbor).unwrap();
        let a_json = serde_json::to_string(&a).unwrap();
        let b_json = serde_json::to_string(&b).unwrap();
        assert_eq!(a_json, b_json);
    }

    #[test]
    fn ingest_rejects_trailing_garbage() {
        let cbor_buf = std::fs::read("tests/fixtures/ledger_state_extended.cbor").unwrap();
        let mut tampered = cbor_buf.clone();
        tampered.push(0xFF); // trailing byte
        let result = ingest_token_policies(&tampered);
        assert!(result.is_err(), "trailing byte must be rejected");
    }
}
