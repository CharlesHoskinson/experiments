//! Integration tests for the `omega-ingest` CLI binary.
//!
//! Asserts that each per-sub-tree subcommand actually writes its
//! ingested JSON output to `--output` (regression coverage for the
//! v0.9.0 bug where `token-policy`, `script`, `stake`, and `governance`
//! discarded their output and exited successfully).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn fixture_path_extended() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ledger_state_extended.cbor")
}
fn fixture_path_stake() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stake_snapshot.cbor")
}
fn fixture_path_governance() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/governance_snapshot.cbor")
}

#[test]
fn cli_ingest_token_policy_writes_output() {
    let exe = env!("CARGO_BIN_EXE_omega-ingest");
    let fixture = fixture_path_extended();
    let out_dir = tempfile::tempdir().unwrap();
    let out_file = out_dir.path().join("policies.json");
    let status = Command::new(exe)
        .args([
            "token-policy",
            "--input",
            fixture.to_str().unwrap(),
            "--output",
            out_file.to_str().unwrap(),
        ])
        .status()
        .expect("cli runs");
    assert!(status.success(), "exit code");
    assert!(out_file.exists(), "output file must exist");
    let body = fs::read_to_string(&out_file).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    let policies = parsed["policies"].as_array().expect("policies array");
    assert_eq!(policies.len(), 2, "extended fixture has 2 policies");
}

#[test]
fn cli_ingest_script_writes_output() {
    let exe = env!("CARGO_BIN_EXE_omega-ingest");
    let fixture = fixture_path_extended();
    let out_dir = tempfile::tempdir().unwrap();
    let out_file = out_dir.path().join("scripts.json");
    let status = Command::new(exe)
        .args([
            "script",
            "--input",
            fixture.to_str().unwrap(),
            "--output",
            out_file.to_str().unwrap(),
        ])
        .status()
        .expect("cli runs");
    assert!(status.success());
    assert!(out_file.exists());
    let body = fs::read_to_string(&out_file).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["scripts"].as_array().unwrap().len(), 2);
}

#[test]
fn cli_ingest_stake_writes_output() {
    let exe = env!("CARGO_BIN_EXE_omega-ingest");
    let fixture = fixture_path_stake();
    let out_dir = tempfile::tempdir().unwrap();
    let out_file = out_dir.path().join("stake.json");
    let status = Command::new(exe)
        .args([
            "stake",
            "--input",
            fixture.to_str().unwrap(),
            "--output",
            out_file.to_str().unwrap(),
        ])
        .status()
        .expect("cli runs");
    assert!(status.success());
    assert!(out_file.exists());
    let body = fs::read_to_string(&out_file).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["stake_entries"].as_array().unwrap().len(), 4);
}

#[test]
fn cli_ingest_governance_writes_output() {
    let exe = env!("CARGO_BIN_EXE_omega-ingest");
    let fixture = fixture_path_governance();
    let out_dir = tempfile::tempdir().unwrap();
    let out_file = out_dir.path().join("gov.json");
    let status = Command::new(exe)
        .args([
            "governance",
            "--input",
            fixture.to_str().unwrap(),
            "--output",
            out_file.to_str().unwrap(),
        ])
        .status()
        .expect("cli runs");
    assert!(status.success());
    assert!(out_file.exists());
    let body = fs::read_to_string(&out_file).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["facts"].as_array().unwrap().len(), 4);
}
