# CI and Operational Audit

_Source: parallel agent pass, 2026-05-03. Files audited: `.github/workflows/`, `omega-commitment/scripts/`, `omega-commitment/rust-toolchain.toml`, `omega-commitment/.cargo/config.toml`._

## Workflows

One workflow at the repo root: `.github/workflows/ci.yml`.

- Triggers: `push`, `pull_request`.
- Runner: `ubuntu-latest` with stable Rust via `dtolnay/rust-toolchain` (line 11-13).
- Steps:
  - `cargo fmt --check` (line 14).
  - `cargo clippy --workspace --all-targets -- -D warnings` (line 15).
  - `cargo test --workspace` (line 16).

(Prior audit A10/F002 closed by moving CI to repo root from `omega-commitment/.github/workflows/`.)

**Not in CI**: release publishing, dependency auditing (`cargo audit`), mutation testing (`cargo-mutants` available locally per skills manifest), fuzz testing (`cargo-fuzz`), property-based tests (proptest in dev-deps but not invoked separately), Kani model checking (`kani-verifier` available locally), benchmark comparisons (`criterion` benches present, not run in CI), security scanning, SBOM generation, reproducibility artifacts.

## Build configuration

- `omega-commitment/rust-toolchain.toml`: `channel = stable`, components `clippy, rustfmt`, profile `minimal`.
- MSRV: **1.79** (`omega-commitment/Cargo.toml:13`).
- `omega-commitment/.cargo/config.toml` aliases:
  - `fmt-check = "fmt --all -- --check"`.
  - `lint = "clippy --workspace --all-targets -- -D warnings"`.
- No explicit `[profile.release]`, `[lints.rust]`, or workspace lint inheritance in `Cargo.toml`.
- Workspace deps pinned to minor: `blake2 0.10`, `sha3 0.10`, `serde 1.0`, `clap 4.5`, `criterion 0.5`, `proptest 1.6`. (Prior audit A10/F001 raised major-only floats; partially closed — minor-pinned now, no Cargo.lock policy documented.)

## Scripts

`omega-commitment/scripts/` contains two human-invoked scripts:

1. **`download_snapshot.sh`** (96 lines)
   - Fetches pre-release-preview Mithril-attested Cardano snapshot for manual QA.
   - **DEBUG ONLY** — does not verify Mithril certificate (lines 4, 22; prior audit A10/F002 ref).
   - Usage: `./scripts/download_snapshot.sh [aggregator-url]`. Output: `var/snapshots/<digest>/`.
   - Not invoked by tests; tests use in-tree CBOR fixtures.
2. **`dump_ledger_state.sh`** (130 lines)
   - Dumps Conway-era LedgerState as JSON from a synced headless mainnet node.
   - Canonical input for STAKE and GOVERNANCE sub-trees only; UTXO handled by `omega-utxo-snapshot` (lines 7-8). 2026-05-03 v1.0 ingestion revision.
   - `./scripts/dump_ledger_state.sh [--allow-incomplete] [output-path]`.
   - Writes ~2 GB JSON; configurable via `CARDANO_HOME`, `CARDANO_CLI`, `CARDANO_NODE_SOCKET_PATH`.

## Operational gaps

- **No release automation** — no CD workflow; releases manual.
- **No reproducibility** — no signed artifacts, no build cache, no bit-for-bit verification, no Cargo.lock policy documented.
- **Property tests not in CI** — proptest 1.6 declared but not exercised separately.
- **Benchmarks not in CI** — criterion benches exist (`omega-commitment-core/benches/tree.rs`); no baseline-comparison job.
- **No supply-chain hardening** — no `cargo-audit`, no `cargo-vet`/SLSA provenance, no SBOM.
- **No fuzz harness** — `cargo-fuzz` available locally; no `fuzz/` directory.
- **No formal verification job** — `kani-verifier` available locally; no Kani harnesses.
- **Snapshot acquisition split across two scripts** — no unified orchestration; CI cannot exercise full ingestion path.
- **No workspace-level lint config** — `[lints]` not set; deny lists only on the CLI command line.
- **Snapshot scripts download external binaries** — `download_snapshot.sh` and headless-node setup install `cardano-node` / `mithril-client` without checksum verification. (Prior audit A10/F003-F004 mark this as debug-only; no production path documented.)
