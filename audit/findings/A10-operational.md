---
agent: A10
lane: cross
title: operational
files-reviewed: [
  ARCHITECTURE.md,
  GOALS.md,
  LICENSE,
  README.md,
  audit-prompt.md,
  instructions.md,
  audit/findings/A1-cryptographic-correctness.md,
  audit/findings/A2-cbor-strictness.md,
  audit/findings/A3-cardano-semantics.md,
  audit/findings/A4-test-design.md,
  audit/findings/A5-rust-idioms.md,
  audit/findings/A6-lsq-binary.md,
  cardano-wiki/README.md,
  cardano-wiki/SCHEMA.md,
  cardano-wiki/wiki/.search-index.md,
  cardano-wiki/wiki/index.md,
  cardano-wiki/wiki/log.md,
  cardano-wiki/wiki/overview.md,
  cardano-wiki/wiki/pages/cardano-orgs.md,
  cardano-wiki/wiki/pages/cardano-repos.md,
  cardano-wiki/wiki/pages/cip-1694-governance.md,
  cardano-wiki/wiki/pages/eutxo-model.md,
  cardano-wiki/wiki/pages/hydra-scaling.md,
  cardano-wiki/wiki/pages/intersect-mbo.md,
  cardano-wiki/wiki/pages/ledger-state-json-layout.md,
  cardano-wiki/wiki/pages/leios-scaling.md,
  cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md,
  cardano-wiki/wiki/pages/midnight-sidechain.md,
  cardano-wiki/wiki/pages/mithril-certificates.md,
  cardano-wiki/wiki/pages/ouroboros-consensus.md,
  cardano-wiki/wiki/pages/plomin-hard-fork.md,
  cardano-wiki/wiki/pages/plutus-and-smart-contracts.md,
  cardano-wiki/wiki/pages/project-catalyst.md,
  cardano-wiki/wiki/pages/spec-ouroboros-omega.md,
  cardano-wiki/wiki/pages/voltaire-roadmap.md,
  cardano-wiki/docs/codex_briefings/2026-05-01-omega-codex-debug-brief.md,
  cardano-wiki/docs/codex_briefings/2026-05-03-omega-codex-pipeline-update-brief.md,
  cardano-wiki/docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md,
  cardano-wiki/docs/superpowers/plans/2026-05-01-omega-block-header-accumulator-plan.md,
  cardano-wiki/docs/superpowers/plans/2026-05-01-omega-bundle-assembly-plan.md,
  cardano-wiki/docs/superpowers/plans/2026-05-01-omega-cardano-ingestion-and-qa-plan.md,
  cardano-wiki/docs/superpowers/plans/2026-05-01-omega-ingest-mainnet-plan.md,
  cardano-wiki/docs/superpowers/plans/2026-05-01-omega-script-registry-plan.md,
  cardano-wiki/docs/superpowers/plans/2026-05-01-omega-stake-and-governance-plan.md,
  cardano-wiki/docs/superpowers/plans/2026-05-01-omega-token-policies-plan.md,
  cardano-wiki/docs/superpowers/plans/2026-05-01-omega-tx-index-plan.md,
  cardano-wiki/docs/superpowers/plans/2026-05-01-omega-utxo-commitment-plan.md,
  cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v0.3.x-hardening-plan.md,
  cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md,
  cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.1-chain-follower-plan.md,
  cardano-wiki/docs/superpowers/plans/2026-05-01-ouroboros-omega-program-roadmap.md,
  cardano-wiki/docs/superpowers/plans/2026-05-02-omega-v0.9.1-codex-fixes-plan.md,
  cardano-wiki/docs/superpowers/plans/2026-05-03-experiments-repo-readme-architecture.md,
  cardano-wiki/docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md,
  omega-commitment/.cargo/config.toml,
  omega-commitment/.github/workflows/ci.yml,
  omega-commitment/.gitignore,
  omega-commitment/Cargo.toml,
  omega-commitment/README.md,
  omega-commitment/rust-toolchain.toml,
  omega-commitment/scripts/download_snapshot.sh,
  omega-commitment/scripts/dump_ledger_state.sh,
  omega-commitment/scripts/setup_daedalus.md,
  omega-commitment/scripts/setup_headless_node.md,
  omega-commitment/crates/omega-commitment-bundle/Cargo.toml,
  omega-commitment/crates/omega-commitment-bundle/src/bundle.rs,
  omega-commitment/crates/omega-commitment-bundle/src/lib.rs,
  omega-commitment/crates/omega-commitment-bundle/src/main.rs,
  omega-commitment/crates/omega-commitment-bundle/src/recompute.rs,
  omega-commitment/crates/omega-commitment-bundle/src/sub_tree_id.rs,
  omega-commitment/crates/omega-commitment-bundle/tests/end_to_end_integration.rs,
  omega-commitment/crates/omega-commitment-bundle/tests/golden_bundle.rs,
  omega-commitment/crates/omega-commitment-cli/Cargo.toml,
  omega-commitment/crates/omega-commitment-cli/src/main.rs,
  omega-commitment/crates/omega-commitment-cli/tests/cli.rs,
  omega-commitment/crates/omega-commitment-core/Cargo.toml,
  omega-commitment/crates/omega-commitment-core/benches/tree.rs,
  omega-commitment/crates/omega-commitment-core/src/governance_state_leaf.rs,
  omega-commitment/crates/omega-commitment-core/src/hash.rs,
  omega-commitment/crates/omega-commitment-core/src/header_leaf.rs,
  omega-commitment/crates/omega-commitment-core/src/lib.rs,
  omega-commitment/crates/omega-commitment-core/src/script_registry_leaf.rs,
  omega-commitment/crates/omega-commitment-core/src/serde_helpers.rs,
  omega-commitment/crates/omega-commitment-core/src/stake_state_leaf.rs,
  omega-commitment/crates/omega-commitment-core/src/token_policy_leaf.rs,
  omega-commitment/crates/omega-commitment-core/src/tree.rs,
  omega-commitment/crates/omega-commitment-core/src/tx_index_leaf.rs,
  omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs,
  omega-commitment/crates/omega-commitment-core/src/witness.rs,
  omega-commitment/crates/omega-commitment-core/tests/fixtures/governance_state_small.json,
  omega-commitment/crates/omega-commitment-core/tests/fixtures/header_chain_small.json,
  omega-commitment/crates/omega-commitment-core/tests/fixtures/script_registry_small.json,
  omega-commitment/crates/omega-commitment-core/tests/fixtures/stake_state_small.json,
  omega-commitment/crates/omega-commitment-core/tests/fixtures/token_policies_small.json,
  omega-commitment/crates/omega-commitment-core/tests/fixtures/tx_index_small.json,
  omega-commitment/crates/omega-commitment-core/tests/fixtures/utxo_set_small.json,
  omega-commitment/crates/omega-commitment-core/tests/golden_vectors.rs,
  omega-commitment/crates/omega-commitment-core/tests/governance_state_integration.rs,
  omega-commitment/crates/omega-commitment-core/tests/header_integration.rs,
  omega-commitment/crates/omega-commitment-core/tests/script_registry_integration.rs,
  omega-commitment/crates/omega-commitment-core/tests/stake_state_integration.rs,
  omega-commitment/crates/omega-commitment-core/tests/token_policy_integration.rs,
  omega-commitment/crates/omega-commitment-core/tests/tx_index_integration.rs,
  omega-commitment/crates/omega-commitment-core/tests/utxo_integration.rs,
  omega-commitment/crates/omega-commitment-ingest/Cargo.toml,
  omega-commitment/crates/omega-commitment-ingest/examples/probe_ledger_state_paths.rs,
  omega-commitment/crates/omega-commitment-ingest/src/cbor.rs,
  omega-commitment/crates/omega-commitment-ingest/src/governance.rs,
  omega-commitment/crates/omega-commitment-ingest/src/lib.rs,
  omega-commitment/crates/omega-commitment-ingest/src/main.rs,
  omega-commitment/crates/omega-commitment-ingest/src/script.rs,
  omega-commitment/crates/omega-commitment-ingest/src/stake.rs,
  omega-commitment/crates/omega-commitment-ingest/src/token_policy.rs,
  omega-commitment/crates/omega-commitment-ingest/src/utxo.rs,
  omega-commitment/crates/omega-commitment-ingest/tests/cli.rs,
  omega-commitment/crates/omega-commitment-ingest/tests/fixtures/governance_snapshot.cbor,
  omega-commitment/crates/omega-commitment-ingest/tests/fixtures/governance_snapshot.cbor.md,
  omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor,
  omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor.md,
  omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_minimal.cbor,
  omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_minimal.cbor.md,
  omega-commitment/crates/omega-commitment-ingest/tests/fixtures/stake_snapshot.cbor,
  omega-commitment/crates/omega-commitment-ingest/tests/fixtures/stake_snapshot.cbor.md,
  omega-commitment/crates/omega-commitment-ingest/tests/golden_ingest.rs,
  omega-commitment/crates/omega-commitment-ingest/tests/governance_ingest_integration.rs,
  omega-commitment/crates/omega-commitment-ingest/tests/qa_pipeline.rs,
  omega-commitment/crates/omega-commitment-ingest/tests/script_ingest_integration.rs,
  omega-commitment/crates/omega-commitment-ingest/tests/stake_ingest_integration.rs,
  omega-commitment/crates/omega-commitment-ingest/tests/token_policy_ingest_integration.rs,
  omega-commitment/crates/omega-commitment-ingest/tests/utxo_ingest_integration.rs,
  omega-commitment/crates/omega-utxo-snapshot/Cargo.toml,
  omega-commitment/crates/omega-utxo-snapshot/src/main.rs
]
findings-count: { p0: 0, p1: 3, p2: 1, p3: 0 }
---

# Summary

No private key material, API tokens, password assignments, `.env`/`.envrc`, shell-history files, oversized fixtures, or populated snapshot artifacts were found by the recursive `rg` passes. The root `LICENSE` is Apache License 2.0 and matches the workspace `license = "Apache-2.0"` declaration; no git dependencies, registry overrides, or outside-workspace path dependencies were found. Release readiness is blocked by floating major-only Cargo requirements, a CI workflow stored outside the repository root workflow path, and a snapshot download helper that bypasses Mithril verification.

# Findings

## F001 — Cargo dependencies are not all pinned to major.minor

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/Cargo.toml:20-25`; `omega-commitment/crates/omega-commitment-ingest/Cargo.toml:23-29`; `omega-commitment/crates/omega-commitment-bundle/Cargo.toml:23-28`; `omega-commitment/crates/omega-utxo-snapshot/Cargo.toml:14-18`; `omega-commitment/crates/omega-commitment-cli/Cargo.toml:19-20`
- **Issue:** The audit invariant requires every Cargo dependency to be pinned at least to a major.minor version. Several external crates are declared as major-only (`"1"`, `"3"`, `"4"`), so fresh resolution can move across unaudited minor releases even when no source changes.
- **Evidence:**
```text
omega-commitment/Cargo.toml:20:serde = { version = "1", features = ["derive"] }
omega-commitment/Cargo.toml:21:serde_json = "1"
omega-commitment/Cargo.toml:23:thiserror = "1"
omega-commitment/Cargo.toml:24:clap = { version = "4", features = ["derive"] }
omega-commitment/Cargo.toml:25:proptest = "1"
omega-commitment/crates/omega-commitment-ingest/Cargo.toml:23:anyhow = "1"
omega-commitment/crates/omega-commitment-ingest/Cargo.toml:29:tempfile = "3"
omega-commitment/crates/omega-commitment-bundle/Cargo.toml:23:anyhow = "1"
omega-commitment/crates/omega-commitment-bundle/Cargo.toml:28:tempfile = "3"
omega-commitment/crates/omega-utxo-snapshot/Cargo.toml:14:anyhow = "1"
omega-commitment/crates/omega-utxo-snapshot/Cargo.toml:18:tokio = { version = "1", features = ["rt-multi-thread", "macros", "fs", "io-util"] }
omega-commitment/crates/omega-commitment-cli/Cargo.toml:19:anyhow = "1"
omega-commitment/crates/omega-commitment-cli/Cargo.toml:20:tempfile = "3"
```
- **Suggested fix:** Replace major-only requirements with `~major.minor` requirements, or exact `=major.minor.patch` requirements if the release needs patch-level reproducibility; plain Cargo `"1.0"` still floats to later 1.x minors. Keep the internal `path = "../..."` workspace dependencies as-is because they do not point outside the workspace.
```toml
serde = { version = "~1.0", features = ["derive"] }
serde_json = "~1.0"
thiserror = "~1.0"
clap = { version = "~4.5", features = ["derive"] }
proptest = "~1.4"
anyhow = "~1.0"
tempfile = "~3.10"
tokio = { version = "~1.37", features = ["rt-multi-thread", "macros", "fs", "io-util"] }
```
- **Verification:** From repo root, `rg -n '^\\s*[A-Za-z0-9_-]+\\s*=\\s*"[^".]+"|version\\s*=\\s*"[^".]+"' omega-commitment/Cargo.toml omega-commitment/crates/*/Cargo.toml` should return no external dependency declarations; `rg -n 'git\\s*=|path\\s*=' omega-commitment/Cargo.toml omega-commitment/crates/*/Cargo.toml` should continue to show only internal workspace paths.

## F002 — CI workflow is nested where GitHub Actions will not run it for this repo

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/.github/workflows/ci.yml:1-30`
- **Issue:** The audit root is `/home/hoskinson/experiments`, but the only workflow file is under `omega-commitment/.github/workflows/`. For this repository layout there is no active root-level `.github/workflows` CI, so pushes and PRs to the `experiments` repo will not run the Rust checks.
- **Evidence:**
```text
omega-commitment/.github/workflows/ci.yml:1:name: ci
omega-commitment/.github/workflows/ci.yml:13:      - uses: actions/checkout@v4
omega-commitment/.github/workflows/ci.yml:26:        run: cargo fmt-check
omega-commitment/.github/workflows/ci.yml:28:        run: cargo lint
omega-commitment/.github/workflows/ci.yml:30:        run: cargo test --workspace
```
- **Suggested fix:** Move or copy the workflow to `.github/workflows/ci.yml` at the repository root and set its working directory to `omega-commitment`.
```yaml
name: ci
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
defaults:
  run:
    working-directory: omega-commitment
jobs:
  ci:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: rustup show
      - run: cargo fmt-check
      - run: cargo lint
      - run: cargo test --workspace
```
- **Verification:** From repo root, `rg --hidden --files -g '!.git/**' | rg '^\\.github/workflows/[^/]+\\.ya?ml$'` should list the root workflow, and `rg -n 'working-directory: omega-commitment' .github/workflows/ci.yml` should show the Rust commands run in the workspace.

## F003 — Snapshot helper downloads and extracts a Mithril snapshot without Mithril verification

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/scripts/download_snapshot.sh:27-55`
- **Issue:** The README describes the helper as downloading a "Mithril-attested" snapshot, but the script fetches the aggregator JSON with `curl`, trusts `locations[0]`, downloads that URL directly, and extracts the tarball. It does not invoke `mithril-client`, set verification keys, verify a certificate, or check a content digest before extraction.
- **Evidence:**
```text
omega-commitment/README.md:662:Downloads the latest Mithril-attested preview-testnet snapshot to `var/snapshots/<digest>/` (gitignored). Multi-GB; not invoked by tests.
omega-commitment/scripts/download_snapshot.sh:28:SNAPSHOT_JSON="$(curl -fsSL "$AGGREGATOR_URL/snapshots")"
omega-commitment/scripts/download_snapshot.sh:46:DOWNLOAD_URL="$(echo "$SNAPSHOT_JSON" | python3 -c "import json,sys; data=json.load(sys.stdin); print(data[0]['locations'][0])")"
omega-commitment/scripts/download_snapshot.sh:50:curl -fSL --progress-bar "$DOWNLOAD_URL" -o "$DEST_DIR/$DIGEST/snapshot.tar.gz"
omega-commitment/scripts/download_snapshot.sh:54:tar -xzf "$DEST_DIR/$DIGEST/snapshot.tar.gz" -C "$DEST_DIR/$DIGEST/"
```
- **Suggested fix:** Replace the direct `curl`/`tar` path with `mithril-client cardano-db download` and explicit verification-key setup, or rename the helper as unauthenticated debug-only and require a digest/certificate verification step before extraction.
- **Verification:** `rg -n 'mithril-client cardano-db download|GENESIS_VERIFICATION_KEY|ANCILLARY_VERIFICATION_KEY|sha256sum -c' omega-commitment/scripts/download_snapshot.sh` should show the verification path, and `rg -n 'curl -fSL --progress-bar|tar -xzf' omega-commitment/scripts/download_snapshot.sh` should no longer show an unauthenticated direct extract path.

## F004 — Headless setup runbook installs downloaded executables without checksum or signature verification

- **Severity:** P2
- **Confidence:** high
- **Location:** `omega-commitment/scripts/setup_headless_node.md:34-48`
- **Issue:** The mainnet data-capture runbook downloads `cardano-node` and `mithril-client` release tarballs, extracts them, and copies executables into `~/cardano/bin` without a documented checksum, signature, or provenance verification step. HTTPS alone leaves the reproducibility and supply-chain claim weaker than the rest of the release-readiness posture.
- **Evidence:**
```text
omega-commitment/scripts/setup_headless_node.md:35:curl -fSL -o cardano-node-10.7.1-linux-amd64.tar.gz \
omega-commitment/scripts/setup_headless_node.md:37:tar -xzf cardano-node-10.7.1-linux-amd64.tar.gz -C cn-extract --strip-components 0
omega-commitment/scripts/setup_headless_node.md:40:curl -fSL -o mithril-2617.0-linux-x64.tar.gz \
omega-commitment/scripts/setup_headless_node.md:42:mkdir -p mithril-extract && tar -xzf mithril-2617.0-linux-x64.tar.gz -C mithril-extract
omega-commitment/scripts/setup_headless_node.md:45:cp cn-extract/bin/cardano-node ../bin/
omega-commitment/scripts/setup_headless_node.md:47:cp mithril-extract/mithril-client ../bin/
```
- **Suggested fix:** Add pinned SHA256 values or official signature verification immediately after each download and before `tar`/`cp`. If upstream publishes signed checksums, verify the signature and then verify the checksums against the downloaded archives.
- **Verification:** `rg -n 'sha256sum -c|shasum -a 256|gpg --verify|cosign verify' omega-commitment/scripts/setup_headless_node.md` should show verification before extraction, and `rg -n 'tar -xzf|cp .*../bin' omega-commitment/scripts/setup_headless_node.md` should still show the install steps after the checks.
