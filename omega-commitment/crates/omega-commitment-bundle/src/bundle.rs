//! Bundle assembly + verification.
//!
//! Reads seven sub-tree input JSON files from a directory, recomputes
//! each sub-tree's `(blake3_root, sha3_root)`, and aggregates into
//! the canonical Ω-Commitment tuple `(blake3_bundle_root, sha3_bundle_root)`.
//!
//! ## SHA3 bundle root: drift detection, not binding-agility hedge
//!
//! Both per-sub-tree roots are built from the same v1 Blake3 leaf
//! hashes. The SHA3 root is therefore a drift-detection signal over
//! the seven Blake3 roots — divergence between the two bundle roots
//! means the aggregation step disagreed with itself, not that one of
//! the underlying hash functions broke. A truly-independent SHA3 tree
//! (separate per-leaf hashing under SHA3) is tracked as a v2.0
//! follow-up; the audit reframing (A1/F004, 2026-05-03) makes the
//! current SHA3 status explicit so consumers do not over-rely on it
//! as a Blake3-break hedge.

use crate::recompute::recompute;
use crate::sub_tree_id::ALL;
use crate::BundleError;
use omega_commitment_core::hash::{blake3_256, sha3_256, Hash};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

pub const BUNDLE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubTreeRecord {
    /// Stable kebab-case label, e.g. "utxo", "tx-index", "token-policy".
    pub sub_tree: String,
    #[serde(with = "hex::serde")]
    pub blake3_root: Hash,
    #[serde(with = "hex::serde")]
    pub sha3_root: Hash,
    #[serde(with = "hex::serde")]
    pub input_digest: Hash,
    pub leaf_count: usize,
    pub tree_depth: usize,
    pub item_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleRecord {
    pub schema_version: u32,
    #[serde(with = "hex::serde")]
    pub blake3_bundle_root: Hash,
    #[serde(with = "hex::serde")]
    pub sha3_bundle_root: Hash,
    /// Sub-trees in canonical order (matches `sub_tree_id::ALL`).
    pub sub_trees: Vec<SubTreeRecord>,
}

/// Read each of the seven sub-tree input files from `input_dir`, recompute
/// roots, and return the aggregated `BundleRecord`.
///
/// `input_dir` must contain exactly seven files named per
/// `SubTreeId::filename()`. Any missing file produces an error.
pub fn assemble(input_dir: &Path) -> Result<BundleRecord, BundleError> {
    let mut sub_trees: Vec<SubTreeRecord> = Vec::with_capacity(7);
    for st in ALL {
        let path = input_dir.join(st.filename());
        let raw = fs::read_to_string(&path).map_err(|e| BundleError::io(&path, e))?;
        let roots = recompute(st, &raw).map_err(|e| BundleError::Recompute {
            sub_tree: st.label().to_string(),
            source: e,
        })?;
        sub_trees.push(SubTreeRecord {
            sub_tree: st.label().to_string(),
            blake3_root: roots.blake3_root,
            sha3_root: roots.sha3_root,
            input_digest: roots.input_digest,
            leaf_count: roots.leaf_count,
            tree_depth: roots.tree_depth,
            item_count: roots.item_count,
        });
    }
    let blake3_bundle_root = aggregate_blake3(&sub_trees);
    let sha3_bundle_root = aggregate_sha3(&sub_trees);
    Ok(BundleRecord {
        schema_version: BUNDLE_SCHEMA_VERSION,
        blake3_bundle_root,
        sha3_bundle_root,
        sub_trees,
    })
}

/// Re-run assembly against `input_dir` and confirm the resulting roots
/// match the published `bundle`. Returns `Ok(())` on match; a
/// [`BundleError::Mismatch`] (or [`BundleError::SchemaVersionMismatch`])
/// describing the mismatch otherwise.
///
/// In addition to the root checks, this verifier explicitly asserts
/// that the published `item_count` for every sub-tree matches the
/// unpadded item count produced by recomputation. The `item_count`
/// field bounds the maximum valid `canonical_index` in any v1
/// inclusion proof: an inclusion witness whose claimed index is
/// `>= item_count` MUST be rejected as a padding-leaf forgery
/// (audit finding A1/F003, closed in Batch 1).
pub fn verify(bundle: &BundleRecord, input_dir: &Path) -> Result<(), BundleError> {
    let fresh = assemble(input_dir)?;
    if fresh.blake3_bundle_root != bundle.blake3_bundle_root {
        return Err(BundleError::Mismatch {
            field: "blake3_bundle_root".to_string(),
            published: hex::encode(bundle.blake3_bundle_root),
            recomputed: hex::encode(fresh.blake3_bundle_root),
        });
    }
    if fresh.sha3_bundle_root != bundle.sha3_bundle_root {
        return Err(BundleError::Mismatch {
            field: "sha3_bundle_root".to_string(),
            published: hex::encode(bundle.sha3_bundle_root),
            recomputed: hex::encode(fresh.sha3_bundle_root),
        });
    }
    // Explicit per-sub-tree item_count check: catches a published
    // commitment whose leaf set has been padded but whose item_count
    // was forged to admit a padding-leaf inclusion proof.
    if fresh.sub_trees.len() != bundle.sub_trees.len() {
        return Err(BundleError::Mismatch {
            field: "sub_trees.len".to_string(),
            published: bundle.sub_trees.len().to_string(),
            recomputed: fresh.sub_trees.len().to_string(),
        });
    }
    for (published, recomputed) in bundle.sub_trees.iter().zip(fresh.sub_trees.iter()) {
        if published.item_count != recomputed.item_count {
            return Err(BundleError::Mismatch {
                field: format!("item_count[{}]", published.sub_tree),
                published: published.item_count.to_string(),
                recomputed: recomputed.item_count.to_string(),
            });
        }
    }
    if fresh.sub_trees != bundle.sub_trees {
        return Err(BundleError::Mismatch {
            field: "sub_trees".to_string(),
            published: "<per-sub-tree records>".to_string(),
            recomputed: "<per-sub-tree records differ>".to_string(),
        });
    }
    if fresh.schema_version != bundle.schema_version {
        return Err(BundleError::SchemaVersionMismatch {
            published: bundle.schema_version,
            current: fresh.schema_version,
        });
    }
    Ok(())
}

fn aggregate_blake3(sub_trees: &[SubTreeRecord]) -> Hash {
    let mut buf = Vec::with_capacity(7 * 32);
    for r in sub_trees {
        buf.extend_from_slice(&r.blake3_root);
    }
    blake3_256(&buf)
}

fn aggregate_sha3(sub_trees: &[SubTreeRecord]) -> Hash {
    let mut buf = Vec::with_capacity(7 * 32);
    for r in sub_trees {
        buf.extend_from_slice(&r.sha3_root);
    }
    sha3_256(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_minimal_inputs(dir: &Path) {
        // Minimal valid input for each sub-tree (1 entry each).
        fs::write(
            dir.join("utxo.json"),
            r#"{"utxos":[{"tx_id":"0101010101010101010101010101010101010101010101010101010101010101","output_index":0,"address":"6102020202020202020202020202020202020202020202020202020202020202","value_lovelace":1,"assets":[],"datum_option":{"kind":"none"},"script_ref":null}]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("header.json"),
            r#"{"headers":[{"slot":1,"block_height":1,"block_hash":"1100000000000000000000000000000000000000000000000000000000000000","prev_hash":"0000000000000000000000000000000000000000000000000000000000000000"}]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("tx_index.json"),
            r#"{"entries":[{"tx_id":"1100000000000000000000000000000000000000000000000000000000000000","slot":1,"block_hash":"aa00000000000000000000000000000000000000000000000000000000000000","tx_position":0}]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("token_policy.json"),
            r#"{"policies":[{"policy_id":"11000000000000000000000000000000000000000000000000000000","first_issuance_slot":1,"total_supply_at_h":1}]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("script.json"),
            r#"{"scripts":[{"script_hash":"11000000000000000000000000000000000000000000000000000000","deployment_slot":1,"script_size_bytes":1,"language":0}]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("stake.json"),
            r#"{"stake_entries":[{"stake_credential_hash":"11000000000000000000000000000000000000000000000000000000","delegated_pool":"00000000000000000000000000000000000000000000000000000000","delegated_drep":{"kind":"none"},"rewards_lovelace":0,"is_pool_operator":0}]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("governance.json"),
            r#"{"facts":[{"kind":"treasury","key":"0000000000000000000000000000000000000000000000000000000000000000","value":1,"slot":1}]}"#,
        )
        .unwrap();
    }

    #[test]
    fn assemble_minimal_inputs_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let bundle = assemble(dir.path()).unwrap();
        assert_eq!(bundle.schema_version, BUNDLE_SCHEMA_VERSION);
        assert_eq!(bundle.sub_trees.len(), 7);
        assert_ne!(bundle.blake3_bundle_root, [0u8; 32]);
        assert_ne!(bundle.sha3_bundle_root, [0u8; 32]);
        assert_ne!(bundle.blake3_bundle_root, bundle.sha3_bundle_root);
    }

    #[test]
    fn assemble_fails_loudly_on_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        // Delete one file.
        fs::remove_file(dir.path().join("script.json")).unwrap();
        let result = assemble(dir.path());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("script.json"),
            "error should mention the missing file: {msg}"
        );
    }

    #[test]
    fn assemble_canonical_order_in_output() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let bundle = assemble(dir.path()).unwrap();
        let labels: Vec<&str> = bundle
            .sub_trees
            .iter()
            .map(|s| s.sub_tree.as_str())
            .collect();
        assert_eq!(
            labels,
            vec![
                "utxo",
                "header",
                "tx-index",
                "token-policy",
                "script",
                "stake",
                "governance"
            ]
        );
    }

    #[test]
    fn verify_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let bundle = assemble(dir.path()).unwrap();
        verify(&bundle, dir.path()).expect("fresh bundle must verify against same inputs");
    }

    #[test]
    fn verify_detects_blake3_root_tamper() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let mut bundle = assemble(dir.path()).unwrap();
        bundle.blake3_bundle_root[0] ^= 0x01;
        let result = verify(&bundle, dir.path());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("blake3_bundle_root"), "got: {msg}");
        assert!(msg.contains("mismatch"), "got: {msg}");
    }

    #[test]
    fn verify_detects_sha3_root_tamper() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let mut bundle = assemble(dir.path()).unwrap();
        bundle.sha3_bundle_root[0] ^= 0x01;
        let result = verify(&bundle, dir.path());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("sha3_bundle_root"), "got: {msg}");
        assert!(msg.contains("mismatch"), "got: {msg}");
    }

    #[test]
    fn verify_detects_input_data_tamper() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let bundle = assemble(dir.path()).unwrap();
        // Tamper with one input file.
        fs::write(
            dir.path().join("utxo.json"),
            r#"{"utxos":[{"tx_id":"0101010101010101010101010101010101010101010101010101010101010101","output_index":0,"address":"6102020202020202020202020202020202020202020202020202020202020202","value_lovelace":99999,"assets":[],"datum_option":{"kind":"none"},"script_ref":null}]}"#,
        )
        .unwrap();
        let result = verify(&bundle, dir.path());
        assert!(result.is_err(), "tampered input must fail verify");
    }

    #[test]
    fn bundle_round_trips_through_json() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let bundle = assemble(dir.path()).unwrap();
        let s = serde_json::to_string_pretty(&bundle).unwrap();
        let parsed: BundleRecord = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, bundle);
    }
}
