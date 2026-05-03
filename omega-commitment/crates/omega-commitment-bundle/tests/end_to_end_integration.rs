//! End-to-end integration test for the bundle-assembly tool.
//!
//! Pulls the 8-entry per-sub-tree fixtures shipped by
//! `omega-commitment-core` and runs the full assemble + verify
//! pipeline against them.

use omega_commitment_bundle::bundle::{assemble, verify};
use std::{fs, path::PathBuf};

/// Path to the omega-commitment-core fixtures dir.
fn core_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("omega-commitment-core/tests/fixtures")
}

/// Mapping from per-sub-tree fixture filename to bundle-tool filename.
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
        fs::copy(src.join(src_name), dest.join(dest_name))
            .unwrap_or_else(|e| panic!("copy {src_name} -> {dest_name}: {e}"));
    }
}

#[test]
fn assemble_and_verify_against_real_fixtures() {
    let dir = tempfile::tempdir().unwrap();
    populate_input_dir(dir.path());

    let bundle = assemble(dir.path()).expect("assemble must succeed against real fixtures");

    // Structural assertions.
    assert_eq!(bundle.schema_version, 1);
    assert_eq!(bundle.sub_trees.len(), 7);
    assert_ne!(bundle.blake2b_bundle_root, [0u8; 32]);
    assert_ne!(bundle.sha3_bundle_root, [0u8; 32]);
    assert_ne!(bundle.blake2b_bundle_root, bundle.sha3_bundle_root);

    // Per-sub-tree assertions: each sub-tree commits 8 items
    // (every fixture in this repo has 8 entries except utxo which has 3).
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

    // utxo fixture has 3 items; others have 8.
    let item_counts: Vec<usize> = bundle.sub_trees.iter().map(|s| s.item_count).collect();
    assert_eq!(item_counts, vec![3, 8, 8, 8, 8, 8, 8]);

    // Each sub-tree's blake2b and sha3 roots differ.
    for s in &bundle.sub_trees {
        assert_ne!(
            s.blake2b_root, s.sha3_root,
            "sub_tree {} has equal blake2b and sha3 roots",
            s.sub_tree
        );
        assert_ne!(s.blake2b_root, [0u8; 32]);
        assert_ne!(s.sha3_root, [0u8; 32]);
    }

    // Round-trip through JSON.
    let s = serde_json::to_string_pretty(&bundle).unwrap();
    let parsed: omega_commitment_bundle::bundle::BundleRecord = serde_json::from_str(&s).unwrap();
    assert_eq!(parsed, bundle);

    // Verify.
    verify(&bundle, dir.path()).expect("fresh bundle must verify against same inputs");
}

#[test]
fn assemble_is_deterministic_across_runs() {
    let dir1 = tempfile::tempdir().unwrap();
    let dir2 = tempfile::tempdir().unwrap();
    populate_input_dir(dir1.path());
    populate_input_dir(dir2.path());
    let b1 = assemble(dir1.path()).unwrap();
    let b2 = assemble(dir2.path()).unwrap();
    assert_eq!(b1.blake2b_bundle_root, b2.blake2b_bundle_root);
    assert_eq!(b1.sha3_bundle_root, b2.sha3_bundle_root);
}

#[test]
fn verify_against_tampered_inputs_fails() {
    let dir = tempfile::tempdir().unwrap();
    populate_input_dir(dir.path());
    let bundle = assemble(dir.path()).unwrap();

    // Tamper with header.json by appending whitespace (changes input_digest).
    let header_path = dir.path().join("header.json");
    let mut content = fs::read_to_string(&header_path).unwrap();
    content.push('\n');
    fs::write(&header_path, content).unwrap();

    let result = verify(&bundle, dir.path());
    assert!(result.is_err(), "tampered header.json must fail verify");
}
