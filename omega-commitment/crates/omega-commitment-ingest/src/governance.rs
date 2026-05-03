//! Governance-state sub-tree ingestion from a hand-crafted
//! governance_snapshot.cbor.
//!
//! Top-level CBOR array of variable-arity entries:
//!   - kind 0..=3: `[ kind (u8), key (32 bytes),
//!                    value (16-byte u128 big-endian), slot (u64) ]`
//!   - kind 4 (AccountState): `[ kind (u8), reserves (u64),
//!                               treasury (u64), deposits (u64),
//!                               fee_pot (u64) ]`
//!
//! The AccountState entry MUST be present in any production snapshot;
//! the ingest layer fails closed if it is missing or its pots are not
//! all four accounted for. The legacy 4-arity entries map onto
//! [`GovernanceFact::Treasury`], [`GovernanceFact::CcSeat`],
//! [`GovernanceFact::RatifiedAction`], and [`GovernanceFact::InFlightAction`]
//! respectively.

use crate::cbor::{expect_end, read_32_bytes, read_array_len, read_u128_bytes, read_u64, read_u8};
use anyhow::Result;
use omega_commitment_core::governance_state_leaf::{kind, GovernanceFact};
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
    let mut saw_account_state = false;
    for _ in 0..n {
        let arity = read_array_len(&mut d)?;
        let kind_byte = read_u8(&mut d)?;
        match kind_byte {
            kind::TREASURY | kind::CC_SEAT | kind::RATIFIED_ACTION | kind::IN_FLIGHT_ACTION => {
                if arity != 4 {
                    return Err(anyhow::anyhow!(
                        "governance fact (kind={kind_byte}) must be 4-elem, got {arity}"
                    ));
                }
                let key = read_32_bytes(&mut d)?;
                let value = read_u128_bytes(&mut d)?;
                let slot = read_u64(&mut d)?;
                facts.push(match kind_byte {
                    kind::TREASURY => GovernanceFact::Treasury { key, value, slot },
                    kind::CC_SEAT => GovernanceFact::CcSeat { key, value, slot },
                    kind::RATIFIED_ACTION => GovernanceFact::RatifiedAction { key, value, slot },
                    kind::IN_FLIGHT_ACTION => GovernanceFact::InFlightAction { key, value, slot },
                    _ => unreachable!(),
                });
            }
            kind::ACCOUNT_STATE => {
                if arity != 5 {
                    return Err(anyhow::anyhow!(
                        "AccountState (kind=4) must be 5-elem [kind, reserves, treasury, deposits, fee_pot], got {arity}"
                    ));
                }
                let reserves = read_account_pot(&mut d, "reserves")?;
                let treasury = read_account_pot(&mut d, "treasury")?;
                let deposits = read_account_pot(&mut d, "deposits")?;
                let fee_pot = read_account_pot(&mut d, "fee_pot")?;
                if saw_account_state {
                    return Err(anyhow::anyhow!(
                        "duplicate AccountState entry: snapshot must carry exactly one"
                    ));
                }
                saw_account_state = true;
                facts.push(GovernanceFact::AccountState {
                    reserves,
                    treasury,
                    deposits,
                    fee_pot,
                });
            }
            other => {
                return Err(anyhow::anyhow!(
                    "unknown governance fact kind byte 0x{other:02x}"
                ))
            }
        }
    }
    expect_end(&d, cbor.len())?;
    Ok(GovernanceOutput { facts })
}

/// Read one AccountState pot (u64). Wrapping in a helper produces a
/// readable error pointing at the missing field name when the wire
/// format truncates partway through.
fn read_account_pot(d: &mut Decoder<'_>, name: &str) -> Result<u64> {
    read_u64(d).map_err(|e| anyhow::anyhow!("AccountState missing pot '{name}': {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        std::fs::read("tests/fixtures/governance_snapshot.cbor").unwrap()
    }

    #[test]
    fn ingest_yields_five_facts() {
        let out = ingest_governance(&fixture()).unwrap();
        // 4 legacy + 1 AccountState.
        assert_eq!(out.facts.len(), 5);
    }

    #[test]
    fn all_five_kinds_present() {
        let out = ingest_governance(&fixture()).unwrap();
        let kinds: std::collections::HashSet<u8> = out.facts.iter().map(|f| f.kind()).collect();
        for k in 0..=4 {
            assert!(kinds.contains(&k), "missing kind={k}");
        }
    }

    #[test]
    fn treasury_fact_decoded_correctly() {
        let out = ingest_governance(&fixture()).unwrap();
        let treasury = out
            .facts
            .iter()
            .find(|f| matches!(f, GovernanceFact::Treasury { .. }))
            .unwrap();
        match treasury {
            GovernanceFact::Treasury { key, value, slot } => {
                assert_eq!(*key, [0u8; 32]);
                assert_eq!(*value, 1_700_000_000_000_000u128);
                assert_eq!(*slot, 100_000);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn account_state_fact_decoded_correctly() {
        let out = ingest_governance(&fixture()).unwrap();
        let acc = out
            .facts
            .iter()
            .find(|f| matches!(f, GovernanceFact::AccountState { .. }))
            .unwrap();
        match acc {
            GovernanceFact::AccountState {
                reserves,
                treasury,
                deposits,
                fee_pot,
            } => {
                assert_eq!(*reserves, 13_000_000_000_000_000);
                assert_eq!(*treasury, 1_700_000_000_000_000);
                assert_eq!(*deposits, 50_000_000_000);
                assert_eq!(*fee_pot, 250_000);
            }
            _ => unreachable!(),
        }
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
    fn rejects_wrong_arity_for_legacy_kind() {
        // kind=0 with arity 2 — should fail.
        let buf = vec![0x81, 0x82, 0x00, 0x40];
        assert!(ingest_governance(&buf).is_err());
    }

    #[test]
    fn rejects_truncated_account_state() {
        // kind=4 (AccountState) declared as 5-elem but only 3 pots
        // physically encoded — the pot helper must fail closed on the
        // missing fee_pot.
        let mut buf = vec![0x81, 0x85];
        buf.push(0x04); // kind = AccountState
        buf.extend_from_slice(&[0x1B]); // u64 8-byte
        buf.extend_from_slice(&1u64.to_be_bytes());
        buf.extend_from_slice(&[0x1B]);
        buf.extend_from_slice(&2u64.to_be_bytes());
        buf.extend_from_slice(&[0x1B]);
        buf.extend_from_slice(&3u64.to_be_bytes());
        // fee_pot intentionally missing.
        let err = ingest_governance(&buf).unwrap_err();
        assert!(format!("{err}").contains("fee_pot"));
    }

    #[test]
    fn rejects_duplicate_account_state() {
        let mut buf = vec![0x82u8]; // outer array length 2
        for _ in 0..2 {
            buf.push(0x85);
            buf.push(0x04);
            for v in [1u64, 2, 3, 4] {
                buf.push(0x1B);
                buf.extend_from_slice(&v.to_be_bytes());
            }
        }
        let err = ingest_governance(&buf).unwrap_err();
        assert!(format!("{err}").contains("duplicate AccountState"));
    }

    #[test]
    fn rejects_unknown_kind_byte() {
        let buf = vec![0x81, 0x82, 0x09, 0x00];
        let err = ingest_governance(&buf).unwrap_err();
        assert!(format!("{err}").contains("unknown governance fact kind"));
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
