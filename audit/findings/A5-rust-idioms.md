---
agent: A5
lane: code
title: rust-idioms
files-reviewed: [instructions.md, omega-commitment/README.md, ARCHITECTURE.md, omega-commitment/Cargo.toml, omega-commitment/rust-toolchain.toml, omega-commitment/.cargo/config.toml, omega-commitment/.github/workflows/ci.yml, omega-commitment/crates/omega-commitment-core/Cargo.toml, omega-commitment/crates/omega-commitment-core/src/lib.rs, omega-commitment/crates/omega-commitment-core/src/hash.rs, omega-commitment/crates/omega-commitment-core/src/tree.rs, omega-commitment/crates/omega-commitment-core/src/witness.rs, omega-commitment/crates/omega-commitment-core/src/serde_helpers.rs, omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs, omega-commitment/crates/omega-commitment-core/src/header_leaf.rs, omega-commitment/crates/omega-commitment-core/src/tx_index_leaf.rs, omega-commitment/crates/omega-commitment-core/src/token_policy_leaf.rs, omega-commitment/crates/omega-commitment-core/src/script_registry_leaf.rs, omega-commitment/crates/omega-commitment-core/src/stake_state_leaf.rs, omega-commitment/crates/omega-commitment-core/src/governance_state_leaf.rs, omega-commitment/crates/omega-commitment-cli/Cargo.toml, omega-commitment/crates/omega-commitment-cli/src/main.rs, omega-commitment/crates/omega-commitment-bundle/Cargo.toml, omega-commitment/crates/omega-commitment-bundle/src/lib.rs, omega-commitment/crates/omega-commitment-bundle/src/main.rs, omega-commitment/crates/omega-commitment-bundle/src/bundle.rs, omega-commitment/crates/omega-commitment-bundle/src/recompute.rs, omega-commitment/crates/omega-commitment-bundle/src/sub_tree_id.rs, omega-commitment/crates/omega-commitment-ingest/Cargo.toml, omega-commitment/crates/omega-commitment-ingest/src/lib.rs, omega-commitment/crates/omega-commitment-ingest/src/main.rs, omega-commitment/crates/omega-commitment-ingest/src/cbor.rs, omega-commitment/crates/omega-commitment-ingest/src/utxo.rs, omega-commitment/crates/omega-commitment-ingest/src/token_policy.rs, omega-commitment/crates/omega-commitment-ingest/src/script.rs, omega-commitment/crates/omega-commitment-ingest/src/stake.rs, omega-commitment/crates/omega-commitment-ingest/src/governance.rs, omega-commitment/crates/omega-commitment-ingest/examples/probe_ledger_state_paths.rs, omega-commitment/crates/omega-utxo-snapshot/Cargo.toml, omega-commitment/crates/omega-utxo-snapshot/src/main.rs]
findings-count: { p0: 0, p1: 0, p2: 3, p3: 3 }
---

# Summary

I reviewed the omega-commitment Rust workspace, manifests, toolchain config, CI config, and panic/error-handling surfaces. `rg -n "\bunsafe\b" omega-commitment/crates` found no unsafe code, and `find omega-commitment -name Cargo.lock -print` found no shipped lockfile, matching the stated no-lockfile policy. `cargo clippy --workspace --all-targets -- -D warnings` passed from a copied `/tmp` workspace after fresh crates.io resolution; `cargo fmt --all -- --check` failed on the ingest example.

# Findings

## F001 — Bundle library exposes anyhow in public APIs

- **Severity:** P2
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-bundle/src/bundle.rs:46-77`
- **Issue:** The bundle crate is both a library and a binary crate, but its public library API returns `anyhow::Result` and documents `anyhow::Error`. The A5 invariant in `instructions.md:25` says the intended pattern is one `anyhow` boundary per binary and typed errors per library.
- **Evidence:**
```rust
pub fn assemble(input_dir: &Path) -> anyhow::Result<BundleRecord> {
```
```rust
/// `anyhow::Error` describing the mismatch otherwise.
pub fn verify(bundle: &BundleRecord, input_dir: &Path) -> anyhow::Result<()> {
```
- **Suggested fix:** Add a non-exhaustive `BundleError` using `thiserror`, convert read/parse/recompute/verification mismatches into variants, and keep `anyhow` only in `src/main.rs`.
- **Verification:** `rg -n "pub fn (assemble|verify).*anyhow|anyhow::Error" omega-commitment/crates/omega-commitment-bundle/src`

## F002 — Ingest library exports anyhow instead of typed parse errors

- **Severity:** P2
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-ingest/src/utxo.rs:13-33`
- **Issue:** `omega-commitment-ingest` is a library crate, but its public CBOR helpers and ingest functions return `anyhow::Result`. That erases error categories that downstream consumers need to distinguish malformed CBOR, unsupported fixture shape, integer overflow, and trailing bytes.
- **Evidence:**
```rust
use anyhow::Result;

pub fn ingest_utxos(cbor: &[u8]) -> Result<UtxoOutput> {
```
```rust
pub fn read_32_bytes<'b>(d: &mut Decoder<'b>) -> Result<[u8; 32]> {
```
- **Suggested fix:** Introduce `IngestError`/`CborError` enums with `thiserror`, expose `Result<T, IngestError>` from public ingest functions, and let `omega-ingest` convert those typed errors into `anyhow` at the CLI boundary.
- **Verification:** `rg -n "anyhow::Result|use anyhow|pub fn ingest_.*Result|pub fn read_.*Result" omega-commitment/crates/omega-commitment-ingest/src`

## F003 — Several dependencies are major-only under a no-lockfile policy

- **Severity:** P2
- **Confidence:** high
- **Location:** `omega-commitment/Cargo.toml:17-25`
- **Issue:** The workspace policy is "no lockfile, fresh resolution per consumer" per `instructions.md:25`, but several dependencies are specified only by major version. With no committed `Cargo.lock`, fresh consumers can silently pick newer minor lines for `serde`, `serde_json`, `thiserror`, `clap`, `proptest`, `anyhow`, `tempfile`, and `tokio`.
- **Evidence:**
```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
clap = { version = "4", features = ["derive"] }
proptest = "1"
```
Additional major-only entries include `anyhow = "1"`, `tempfile = "3"`, and `tokio = { version = "1", ... }` in crate manifests.
- **Suggested fix:** Change major-only requirements to at least explicit major.minor requirements chosen from the fresh resolution already tested, or use exact `=` requirements if the no-lockfile policy is meant to provide reproducibility rather than freshness.
- **Verification:** `rg -n '= "[0-9]+"|version = "[0-9]+"' omega-commitment/Cargo.toml omega-commitment/crates/*/Cargo.toml`

## F004 — rustfmt check fails on the ingest example

- **Severity:** P3
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-ingest/examples/probe_ledger_state_paths.rs:63-119`
- **Issue:** `cargo fmt --all -- --check` exits non-zero. The repository advertises a fmt-check CI step, so this file currently breaks the rustfmt-clean status required by A5.
- **Evidence:**
```rust
Value::Array(a) => a.first().map(|x| x.to_string()).unwrap_or_else(|| "<empty>".into()),
```
```rust
eprintln!("probe: total wall {total_ms} ms ; peak VmHWM {} KiB ({:.2} GiB)", final_rss, final_rss as f64 / (1024.0 * 1024.0));
```
- **Suggested fix:** Run `cargo fmt --all` after addressing any source changes, or manually wrap the long expressions in `probe_ledger_state_paths.rs` to match rustfmt output.
- **Verification:** `/home/hoskinson/.cargo/bin/cargo fmt --all -- --check` from `omega-commitment/`

## F005 — Probe example panics on missing user input

- **Severity:** P3
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-ingest/examples/probe_ledger_state_paths.rs:85-86`
- **Issue:** The example binary uses `expect` on a command-line argument. Missing user input is not an infallible condition, and A5's review rule requires every `unwrap`/`expect`/`panic!` to be obviously infallible or a TODO.
- **Evidence:**
```rust
fn main() -> anyhow::Result<()> {
    let path = env::args().nth(1).expect("usage: probe_ledger_state_paths <path-to-ledger.json>");
```
- **Suggested fix:** Return a normal error, or use `clap` like the workspace binaries. Minimal one-line shape:
```rust
let path = env::args().nth(1).ok_or_else(|| anyhow::anyhow!("usage: probe_ledger_state_paths <path-to-ledger.json>"))?;
```
- **Verification:** `rg -n "expect\\(\"usage: probe_ledger_state_paths" omega-commitment/crates/omega-commitment-ingest/examples/probe_ledger_state_paths.rs`

## F006 — Ingest source docs still claim implemented paths are scaffolded

- **Severity:** P3
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-ingest/src/lib.rs:5-8`
- **Issue:** The crate-level rustdoc still says token-policy, script, stake, and governance return `unimplemented!()`, while the modules and CLI now dispatch to implemented functions. This is not a runtime panic, but it leaves a false panic marker in source-level documentation and misleads `rg`-based audit passes.
- **Evidence:**
```rust
//! other four LedgerState-derivable sub-trees (token-policy, script,
//! stake, governance) — those four return `unimplemented!()` with a
//! pointer to the follow-up `omega-commitment-ingest-mainnet` plan.
```
- **Suggested fix:** Update `src/lib.rs` and the `omega-ingest` command help comments to reflect the current implemented fixture-ingestion status, or add precise TODO wording for only the real-data chain-follower gaps.
- **Verification:** `rg -n "SCAFFOLD|unimplemented!\\(\\)|not yet implemented" omega-commitment/crates/omega-commitment-ingest/src`
