//! Stake-state sub-tree ingestion from a hand-crafted stake_snapshot.cbor.
//!
//! Top-level CBOR array of 5-element entries:
//!   [ stake_credential_hash (28),
//!     delegated_pool (28),
//!     delegated_drep ([tag: u8] or [tag: u8, payload: 28-byte bytes]),
//!     rewards_lovelace (u64),
//!     is_pool_operator (u8) ]
//!
//! `delegated_drep` is encoded as a CBOR array (length 1 or 2) so the
//! Conway-era DRep tag (None / KeyHash / ScriptHash / AlwaysAbstain /
//! AlwaysNoConfidence) round-trips faithfully into
//! `omega_commitment_core::stake_state_leaf::DrepDelegation`.

use crate::cbor::{expect_end, read_28_bytes, read_array_len, read_u64, read_u8};
use anyhow::Result;
use omega_commitment_core::stake_state_leaf::{DrepDelegation, StakeEntry};
use pallas_codec::minicbor::Decoder;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct StakeOutput {
    pub stake_entries: Vec<StakeEntry>,
}

pub fn ingest_stake(cbor: &[u8]) -> Result<StakeOutput> {
    let mut d = Decoder::new(cbor);
    let n = read_array_len(&mut d)?;
    let mut stake_entries = Vec::with_capacity(n);
    for _ in 0..n {
        let arity = read_array_len(&mut d)?;
        if arity != 5 {
            return Err(anyhow::anyhow!("stake entry must be 5-elem, got {arity}"));
        }
        let stake_credential_hash = read_28_bytes(&mut d)?;
        let delegated_pool = read_28_bytes(&mut d)?;
        let delegated_drep = read_drep(&mut d)?;
        let rewards_lovelace = read_u64(&mut d)?;
        let is_pool_operator = read_u8(&mut d)?;
        stake_entries.push(StakeEntry {
            stake_credential_hash,
            delegated_pool,
            delegated_drep,
            rewards_lovelace,
            is_pool_operator,
        });
    }
    expect_end(&d, cbor.len())?;
    Ok(StakeOutput { stake_entries })
}

/// Read a Conway-era DRep delegation. Wire format: `[tag: u8]`
/// (no payload — None / AlwaysAbstain / AlwaysNoConfidence) or
/// `[tag: u8, payload: 28-byte bytes]` (KeyHash / ScriptHash).
///
/// Tag table mirrors `DrepDelegation::tag_byte`:
///   `0x00` = None, `0x01` = KeyHash, `0x02` = ScriptHash,
///   `0x03` = AlwaysAbstain, `0x04` = AlwaysNoConfidence.
fn read_drep(d: &mut Decoder<'_>) -> Result<DrepDelegation> {
    let arity = read_array_len(d)?;
    if arity == 0 || arity > 2 {
        return Err(anyhow::anyhow!(
            "drep array must be 1- or 2-elem, got {arity}"
        ));
    }
    let tag = read_u8(d)?;
    match tag {
        0x00 => {
            if arity != 1 {
                return Err(anyhow::anyhow!(
                    "drep tag 0x00 (None) must carry no payload, got arity {arity}"
                ));
            }
            Ok(DrepDelegation::None)
        }
        0x01 => {
            if arity != 2 {
                return Err(anyhow::anyhow!(
                    "drep tag 0x01 (KeyHash) requires 28-byte payload, got arity {arity}"
                ));
            }
            Ok(DrepDelegation::KeyHash {
                hash: read_28_bytes(d)?,
            })
        }
        0x02 => {
            if arity != 2 {
                return Err(anyhow::anyhow!(
                    "drep tag 0x02 (ScriptHash) requires 28-byte payload, got arity {arity}"
                ));
            }
            Ok(DrepDelegation::ScriptHash {
                hash: read_28_bytes(d)?,
            })
        }
        0x03 => {
            if arity != 1 {
                return Err(anyhow::anyhow!(
                    "drep tag 0x03 (AlwaysAbstain) must carry no payload, got arity {arity}"
                ));
            }
            Ok(DrepDelegation::AlwaysAbstain)
        }
        0x04 => {
            if arity != 1 {
                return Err(anyhow::anyhow!(
                    "drep tag 0x04 (AlwaysNoConfidence) must carry no payload, got arity {arity}"
                ));
            }
            Ok(DrepDelegation::AlwaysNoConfidence)
        }
        other => Err(anyhow::anyhow!("unknown drep tag byte 0x{other:02x}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        std::fs::read("tests/fixtures/stake_snapshot.cbor").unwrap()
    }

    #[test]
    fn ingest_yields_four_entries() {
        let out = ingest_stake(&fixture()).unwrap();
        assert_eq!(out.stake_entries.len(), 4);
    }

    #[test]
    fn entry_zero_is_undelegated() {
        let out = ingest_stake(&fixture()).unwrap();
        let e = &out.stake_entries[0];
        assert_eq!(e.stake_credential_hash, [0x11; 28]);
        assert_eq!(e.delegated_pool, [0u8; 28]);
        assert_eq!(e.delegated_drep, DrepDelegation::None);
        assert_eq!(e.rewards_lovelace, 0);
        assert_eq!(e.is_pool_operator, 0);
    }

    #[test]
    fn entry_three_is_pool_operator() {
        let out = ingest_stake(&fixture()).unwrap();
        let e = &out.stake_entries[3];
        assert_eq!(e.stake_credential_hash, [0x44; 28]);
        assert_eq!(e.delegated_pool, [0xAA; 28]);
        assert_eq!(e.rewards_lovelace, 100_000_000);
        assert_eq!(e.is_pool_operator, 1);
    }

    #[test]
    fn deterministic_across_runs() {
        let cbor = fixture();
        let a = ingest_stake(&cbor).unwrap();
        let b = ingest_stake(&cbor).unwrap();
        let a_json = serde_json::to_string(&a).unwrap();
        let b_json = serde_json::to_string(&b).unwrap();
        assert_eq!(a_json, b_json);
    }

    #[test]
    fn rejects_wrong_arity() {
        // 3-elem stake entry inside a 1-elem outer array.
        let buf = vec![0x81, 0x83, 0x40, 0x40, 0x40];
        assert!(ingest_stake(&buf).is_err());
    }

    #[test]
    fn ingest_rejects_trailing_garbage() {
        let cbor_buf = std::fs::read("tests/fixtures/stake_snapshot.cbor").unwrap();
        let mut tampered = cbor_buf.clone();
        tampered.push(0xFF); // trailing byte
        let result = ingest_stake(&tampered);
        assert!(result.is_err(), "trailing byte must be rejected");
    }
}
