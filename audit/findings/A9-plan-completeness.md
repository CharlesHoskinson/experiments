---
agent: A9
lane: docs
title: plan-completeness
files-reviewed: [cardano-wiki/wiki/log.md, cardano-wiki/docs/superpowers/plans/2026-05-01-ouroboros-omega-program-roadmap.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-utxo-commitment-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-block-header-accumulator-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-tx-index-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v0.3.x-hardening-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-token-policies-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-script-registry-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-stake-and-governance-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-bundle-assembly-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-cardano-ingestion-and-qa-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-ingest-mainnet-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-02-omega-v0.9.1-codex-fixes-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.1-chain-follower-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-03-experiments-repo-readme-architecture.md, cardano-wiki/docs/codex_briefings/2026-05-01-omega-codex-debug-brief.md, cardano-wiki/docs/codex_briefings/2026-05-03-omega-codex-pipeline-update-brief.md, omega-commitment/Cargo.toml, omega-commitment/README.md, omega-commitment/scripts/dump_ledger_state.sh, omega-commitment/scripts/setup_daedalus.md, omega-commitment/scripts/setup_headless_node.md, omega-commitment/crates/omega-commitment-ingest/Cargo.toml, omega-commitment/crates/omega-commitment-ingest/src/lib.rs, omega-commitment/crates/omega-commitment-ingest/src/main.rs, omega-commitment/crates/omega-utxo-snapshot/Cargo.toml, omega-commitment/crates/omega-utxo-snapshot/src/main.rs]
findings-count: { p0: 0, p1: 4, p2: 1, p3: 0 }
---

# Summary

The current workspace visibly has the v0.9.1 implementation plus the new `omega-utxo-snapshot` helper, while `wiki/log.md` still marks v1.0 real-mainnet ingestion as revised/unblocked rather than fully executed. The plan and handoff docs are not cleanly superseded by the 2026-05-03 revision: multiple executable snippets still point at the obsolete Daedalus/single-CBOR model or a future `omega-ingest --format mainnet` CLI surface that is not present in code. I found no P0s in this lane.

# Findings

## F001 — v1.0 plan still contains executable `--output-cbor` Task 2 body

- **Severity:** P1
- **Confidence:** high
- **Location:** `cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md:403-408`
- **Issue:** The 2026-05-03 revision explicitly says `cardano-cli query ledger-state` does not support `--output-cbor`, but Task 2 still gives an executable script that passes `--output-cbor`. This means the revision does not supersede the original body cleanly and a worker following the task body will run the closed path.
- **Evidence:**
```text
cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md:41:1. **`cardano-cli query ledger-state` does NOT support `--output-cbor`.**
cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md:403:# query ledger-state writes JSON by default. The --out-file argument
cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md:404:# accepts CBOR if we pass --output-cbor (cardano-cli >=10.x).
cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md:405:cardano-cli query ledger-state \
cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md:408:  --output-cbor
```
- **Suggested fix:** Replace the Task 2 body with the shipped JSON-producing `scripts/dump_ledger_state.sh` flow and move the old CBOR script to a clearly non-executable historical note, or delete the obsolete block entirely.
- **Verification:** From repo root, `rg -n "output-cbor|full Conway-era LedgerState as CBOR|Daedalus-bundled" cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md` should find no live Task 2 instructions after the fix.

## F002 — Task 14 omits the new workspace crate from the v1.0 version bump

- **Severity:** P1
- **Confidence:** high
- **Location:** `cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md:1669-1675`
- **Issue:** The revised plan adds `omega-utxo-snapshot` as a workspace member and even marks it shipped, but Task 14 still says to modify only four crate manifests and its `git add` list also omits the new crate. Executing Task 14 as written leaves `crates/omega-utxo-snapshot/Cargo.toml` at `0.9.1` while claiming a workspace `1.0.0` release.
- **Evidence:**
```text
cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md:1669:**Files:**
cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md:1670:- Modify: all four `Cargo.toml` files
cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md:1675:In each of the four crate `Cargo.toml` files: change `version = "0.9.0"` to `version = "1.0.0"`.
omega-commitment/Cargo.toml:3:members = [
omega-commitment/Cargo.toml:8:  "crates/omega-utxo-snapshot",
omega-commitment/crates/omega-utxo-snapshot/Cargo.toml:2:name = "omega-utxo-snapshot"
omega-commitment/crates/omega-utxo-snapshot/Cargo.toml:3:version = "0.9.1"
```
- **Suggested fix:** Update Task 14 to say five crate manifests, include `crates/omega-utxo-snapshot/Cargo.toml` in the version bump and `git add`, and change `0.9.0` to the current pre-v1.0 baseline `0.9.1`.
- **Verification:** `rg -n "all four|omega-utxo-snapshot/Cargo.toml|version = \"0.9.1\"" cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md omega-commitment/crates/omega-utxo-snapshot/Cargo.toml` should show the plan includes the snapshot crate and no longer instructs a four-crate bump.

## F003 — Headless runbook claims a not-yet-existing `omega-ingest --format mainnet` path works

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/scripts/setup_headless_node.md:174-175`
- **Issue:** The canonical headless runbook says the LSQ CBOR file is consumed by `omega-ingest utxo --format mainnet`, but the current CLI has no `--format` argument and still dispatches directly to synthetic-style `ingest_utxos(&cbor)`. This overstates the state of Tasks 4 and 9; the log only unblocks the source-data path, not the parser/CLI implementation.
- **Evidence:**
```text
omega-commitment/scripts/setup_headless_node.md:174:`--output-cbor-bin`. `omega-ingest utxo --format mainnet` consumes this
omega-commitment/scripts/setup_headless_node.md:175:file unchanged.
omega-commitment/crates/omega-commitment-ingest/src/main.rs:29:    Utxo {
omega-commitment/crates/omega-commitment-ingest/src/main.rs:30:        #[arg(short, long)]
omega-commitment/crates/omega-commitment-ingest/src/main.rs:31:        input: PathBuf,
omega-commitment/crates/omega-commitment-ingest/src/main.rs:32:        #[arg(short, long)]
omega-commitment/crates/omega-commitment-ingest/src/main.rs:33:        output: PathBuf,
omega-commitment/crates/omega-commitment-ingest/src/main.rs:76:fn run_utxo(input: PathBuf, output: PathBuf) -> Result<()> {
omega-commitment/crates/omega-commitment-ingest/src/main.rs:79:    let out = ingest_utxos(&cbor)?;
```
- **Suggested fix:** Change the runbook wording to say the CBOR file is an input for the future Task 4/Task 9 mainnet parser, or implement the `--format {auto,synthetic,mainnet}` dispatch and mainnet parser before publishing this command as usable.
- **Verification:** `rg -n -- "--format|FormatArg|InputFormat|omega-ingest utxo --format mainnet" omega-commitment/scripts/setup_headless_node.md omega-commitment/crates/omega-commitment-ingest/src` should either find matching code support or no runbook claim that the command works.

## F004 — Deprecated Daedalus runbook still instructs the obsolete CBOR dump

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/scripts/setup_daedalus.md:120-140`
- **Issue:** The runbook starts with a deprecation banner pointing to the headless path, but it later tells users to dump LedgerState CBOR with `--output-cbor` and says that file is the v1.0 mainnet input. That conflicts with the 2026-05-03 revision and with the shipped `dump_ledger_state.sh`, which now produces JSON for stake/governance only.
- **Evidence:**
```text
omega-commitment/scripts/setup_daedalus.md:3:Deprecated alternative: Omega v1.0 real-data work is currently using the
omega-commitment/scripts/setup_daedalus.md:5:`scripts/setup_headless_node.md`. Keep this document for reference only unless
omega-commitment/scripts/setup_daedalus.md:120:## 6. Dump LedgerState CBOR
omega-commitment/scripts/setup_daedalus.md:133:cardano-cli query ledger-state \
omega-commitment/scripts/setup_daedalus.md:135:  --output-cbor \
omega-commitment/scripts/setup_daedalus.md:139:The output is multi-GB and may take several minutes to write. This CBOR file is
omega-commitment/scripts/dump_ledger_state.sh:3:# Dump the Conway-era LedgerState as JSON from the headless mainnet node.
omega-commitment/scripts/dump_ledger_state.sh:10:# mainnet, and the cli's `--output-cbor` flag is not supported in 10.16
```
- **Suggested fix:** Rewrite `setup_daedalus.md` as historical reference only: remove the active "Use Daedalus" and `--output-cbor` procedure, and point all current v1.0 capture instructions to `setup_headless_node.md` plus the two-stream JSON/LSQ process.
- **Verification:** `rg -n "Use Daedalus|Dump LedgerState CBOR|--output-cbor|This CBOR file" omega-commitment/scripts/setup_daedalus.md` should find only clearly marked obsolete-history text, not active procedure.

## F005 — Codex briefings disagree on workspace shape after the pipeline update

- **Severity:** P2
- **Confidence:** high
- **Location:** `cardano-wiki/docs/codex_briefings/2026-05-01-omega-codex-debug-brief.md:20-23`
- **Issue:** The older brief does carry a 2026-05-03 supersession banner, but its non-v1.0 workspace summary still says the repo is a four-crate workspace. The newer briefing and root manifest both say `omega-utxo-snapshot` is a new workspace member, so the briefings are not mutually consistent unless the old summary is also superseded or updated.
- **Evidence:**
```text
cardano-wiki/docs/codex_briefings/2026-05-01-omega-codex-debug-brief.md:3:> ⚠️ **PARTIALLY SUPERSEDED 2026-05-03**
cardano-wiki/docs/codex_briefings/2026-05-01-omega-codex-debug-brief.md:21:- A four-crate Rust workspace producing per-sub-tree commitments
cardano-wiki/docs/codex_briefings/2026-05-03-omega-codex-pipeline-update-brief.md:26:**NEW WORKSPACE MEMBER:** `crates/omega-utxo-snapshot/` (added to root `Cargo.toml` `members`).
omega-commitment/Cargo.toml:3:members = [
omega-commitment/Cargo.toml:8:  "crates/omega-utxo-snapshot",
```
- **Suggested fix:** Expand the older brief's supersession banner to include workspace topology/current-state sections, or update the workspace map and "What this code is" section to list the fifth crate while retaining the v0.9.1 baseline notes.
- **Verification:** `rg -n "four-crate|NEW WORKSPACE MEMBER|omega-utxo-snapshot" cardano-wiki/docs/codex_briefings omega-commitment/Cargo.toml` should show either one consistent current workspace count or an explicit supersession note covering the stale count.
