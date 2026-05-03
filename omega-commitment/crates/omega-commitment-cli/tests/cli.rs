use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("omega-commitment-core/tests/fixtures")
        .join(name)
}

fn run_commit(sub_tree: &str, fixture: &str) -> tempfile::TempDir {
    let exe = env!("CARGO_BIN_EXE_omega-commitment");
    let out = tempfile::tempdir().unwrap();
    let status = Command::new(exe)
        .args([
            "commit",
            "--sub-tree",
            sub_tree,
            "--input",
            fixture_path(fixture).to_str().unwrap(),
            "--output",
            out.path().to_str().unwrap(),
        ])
        .status()
        .expect("cli runs");
    assert!(status.success(), "sub-tree {sub_tree} failed");
    out
}

#[test]
fn cli_commit_utxo_smoke() {
    let out = run_commit("utxo", "utxo_set_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(
        body.contains("\"sub_tree\": \"utxo\""),
        "wrong sub_tree tag: {body}"
    );
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 3"));
    assert!(out.path().join("witnesses").exists());
}

#[test]
fn cli_commit_header_smoke() {
    let out = run_commit("header", "header_chain_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(
        body.contains("\"sub_tree\": \"header\""),
        "wrong sub_tree tag: {body}"
    );
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 8"));
    let witness_count = fs::read_dir(out.path().join("witnesses")).unwrap().count();
    assert_eq!(witness_count, 8, "expected 8 header witness files");
}

#[test]
fn cli_commit_default_is_utxo() {
    let exe = env!("CARGO_BIN_EXE_omega-commitment");
    let out = tempfile::tempdir().unwrap();
    let status = Command::new(exe)
        .args([
            "commit",
            "--input",
            fixture_path("utxo_set_small.json").to_str().unwrap(),
            "--output",
            out.path().to_str().unwrap(),
        ])
        .status()
        .expect("cli runs");
    assert!(status.success());
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(
        body.contains("\"sub_tree\": \"utxo\""),
        "default sub_tree should be utxo: {body}"
    );
}

#[test]
fn cli_commit_tx_index_smoke() {
    let out = run_commit("tx-index", "tx_index_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(
        body.contains("\"sub_tree\": \"tx-index\""),
        "wrong sub_tree tag: {body}"
    );
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 8"));
    let witness_count = fs::read_dir(out.path().join("witnesses")).unwrap().count();
    assert_eq!(witness_count, 8, "expected 8 tx-index witness files");
}

#[test]
fn cli_rejects_input_path_that_does_not_exist() {
    let exe = env!("CARGO_BIN_EXE_omega-commitment");
    let out = tempfile::tempdir().unwrap();
    let status = Command::new(exe)
        .args([
            "commit",
            "--sub-tree",
            "utxo",
            "--input",
            "/nonexistent/path/utxos.json",
            "--output",
            out.path().to_str().unwrap(),
        ])
        .status()
        .expect("cli runs");
    assert!(!status.success(), "missing input must fail");
}

#[test]
fn cli_rejects_oversized_input() {
    let exe = env!("CARGO_BIN_EXE_omega-commitment");
    let out = tempfile::tempdir().unwrap();
    let status = Command::new(exe)
        .args([
            "commit",
            "--sub-tree",
            "utxo",
            "--input",
            fixture_path("utxo_set_small.json").to_str().unwrap(),
            "--output",
            out.path().to_str().unwrap(),
            "--max-input-bytes",
            "10",
        ])
        .status()
        .expect("cli runs");
    assert!(!status.success(), "10-byte cap must reject the fixture");
}

#[test]
fn cli_commitment_file_lands_at_expected_path() {
    let out = run_commit("utxo", "utxo_set_small.json");
    assert!(out.path().join("commitment.json").exists());
    let leftovers: Vec<_> = fs::read_dir(out.path())
        .unwrap()
        .filter_map(|r| r.ok())
        .filter(|e| {
            let n = e.file_name();
            let s = n.to_string_lossy();
            s.starts_with(".commitment.") && s.ends_with(".json.tmp")
        })
        .collect();
    assert!(
        leftovers.is_empty(),
        "tempfile not cleaned up: {leftovers:?}"
    );
}

#[test]
fn cli_commit_token_policy_smoke() {
    let out = run_commit("token-policy", "token_policies_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(
        body.contains("\"sub_tree\": \"token-policy\""),
        "wrong sub_tree tag: {body}"
    );
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 8"));
    let witness_count = fs::read_dir(out.path().join("witnesses")).unwrap().count();
    assert_eq!(witness_count, 8, "expected 8 token-policy witness files");
}

#[test]
fn cli_commit_script_smoke() {
    let out = run_commit("script", "script_registry_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(
        body.contains("\"sub_tree\": \"script\""),
        "wrong sub_tree tag: {body}"
    );
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 8"));
    let witness_count = fs::read_dir(out.path().join("witnesses")).unwrap().count();
    assert_eq!(witness_count, 8, "expected 8 script witness files");
}

#[test]
fn cli_commit_stake_smoke() {
    let out = run_commit("stake", "stake_state_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(
        body.contains("\"sub_tree\": \"stake\""),
        "wrong sub_tree tag: {body}"
    );
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 8"));
    let witness_count = fs::read_dir(out.path().join("witnesses")).unwrap().count();
    assert_eq!(witness_count, 8, "expected 8 stake witness files");
}

#[test]
fn cli_commit_governance_smoke() {
    let out = run_commit("governance", "governance_state_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(
        body.contains("\"sub_tree\": \"governance\""),
        "wrong sub_tree tag: {body}"
    );
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 8"));
    let witness_count = fs::read_dir(out.path().join("witnesses")).unwrap().count();
    assert_eq!(witness_count, 8, "expected 8 governance witness files");
}
