//! Golden vector for the canonical Ω-Commitment bundle root tuple
//! against the seven shipped synthetic fixtures.
//!
//! These two hashes are the canonical "synthetic-corpus" Ω-Commitment
//! under the v1 domain-separated Merkle construction (Batch 1 of the
//! 2026-05-03 audit-resolution plan). They lock down:
//!   - per-sub-tree leaf encodings
//!   - per-sub-tree root aggregation (Blake2b, v1 domain tags)
//!   - the SHA3 root parallel computation (drift-detection, NOT a
//!     binding-agility hedge — see `bundle.rs` module docs)
//!   - bundle root aggregation (Blake2b + SHA3 over concatenated roots)
//!   - canonical sub-tree ordering
//!
//! Any failure means SOMETHING in the dual-track commitment path
//! changed. Investigate before regenerating these constants.

use omega_commitment_bundle::bundle::assemble;
use std::{fs, path::PathBuf};

/// Path to the omega-commitment-core fixtures dir.
fn core_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("omega-commitment-core/tests/fixtures")
}

const FIXTURE_RENAMES: &[(&str, &str)] = &[
    ("utxo_set_small.json", "utxo.json"),
    ("header_chain_small.json", "header.json"),
    ("tx_index_small.json", "tx_index.json"),
    ("token_policies_small.json", "token_policy.json"),
    ("script_registry_small.json", "script.json"),
    ("stake_state_small.json", "stake.json"),
    ("governance_state_small.json", "governance.json"),
];

fn populate_input_dir(dest: &std::path::Path) {
    let src = core_fixtures_dir();
    for (src_name, dest_name) in FIXTURE_RENAMES {
        fs::copy(src.join(src_name), dest.join(dest_name)).unwrap();
    }
}

#[test]
fn golden_bundle_blake2b_root() {
    let dir = tempfile::tempdir().unwrap();
    populate_input_dir(dir.path());
    let bundle = assemble(dir.path()).unwrap();
    assert_eq!(
        hex::encode(bundle.blake2b_bundle_root),
        // re-pinned 2026-05-03: Batch 1 crypto soundness (A1/F001-F005)
        "cb9fce73c83de7281b6a0731951e0074a6f09d7131e91e46097486d8d0178f79",
        "blake2b_bundle_root drifted from Batch 1 v1 pin"
    );
}

#[test]
fn golden_bundle_sha3_root() {
    let dir = tempfile::tempdir().unwrap();
    populate_input_dir(dir.path());
    let bundle = assemble(dir.path()).unwrap();
    assert_eq!(
        hex::encode(bundle.sha3_bundle_root),
        // re-pinned 2026-05-03: Batch 1 crypto soundness (A1/F001-F005)
        "f67959dc085f83a76d403cd27b89b67e1e210dbbf150682e9e9aafae45c3606c",
        "sha3_bundle_root drifted from Batch 1 v1 pin"
    );
}

#[test]
fn golden_bundle_canonical_order_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    populate_input_dir(dir.path());
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
        ],
        "canonical sub-tree order changed"
    );
}
