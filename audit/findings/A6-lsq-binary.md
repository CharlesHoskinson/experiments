---
agent: A6
lane: code
title: lsq-binary
files-reviewed: [omega-commitment/crates/omega-utxo-snapshot/src/main.rs, omega-commitment/crates/omega-utxo-snapshot/Cargo.toml, omega-commitment/Cargo.toml, omega-commitment/.gitignore, omega-commitment/scripts/setup_headless_node.md, README.md, cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md, cardano-wiki/wiki/log.md, cardano-wiki/docs/codex_briefings/2026-05-03-omega-codex-pipeline-update-brief.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md]
findings-count: { p0: 0, p1: 1, p2: 2, p3: 0 }
---

# Summary

The LSQ wire-format basis checks out: repo documentation records era index `6` as Conway, the expected `GetUTxOWhole` bytes, and the pallas `protocols.rs` envelope cross-check. The source also sends `Release` before `Done` and propagates mid-query socket/query errors with contextual `anyhow` errors rather than panicking. The remaining issues are operational: the binary makes an avoidable full-buffer copy before writing, the canonical headless runbook omits that buffering/streaming warning, and the manifest does not actually pin the documented pallas-network `0.30.2` basis.

# Findings

## F001 — Full UTxO response is copied before write and LSQ release is delayed

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-utxo-snapshot/src/main.rs:102-114`
- **Issue:** The query result is already an `AnyCbor` buffer, but the code then calls `raw.to_vec()` and keeps the acquired LSQ state open until after the disk write. On multi-GB mainnet responses this creates avoidable peak RSS pressure and means a local write failure exits before `Release`/`Done`.
- **Evidence:**
  ```rust
  let raw: AnyCbor = sq
      .query(request)
      .await
      .context("GetUTxOWhole query failed")?;

  let bytes = raw.to_vec();
  let len = bytes.len();
  tokio::fs::write(&args.out, &bytes)
      .await
      .with_context(|| format!("write {}", args.out.display()))?;

  sq.send_release().await.context("LSQ release")?;
  sq.send_done().await.context("LSQ done")?;
  ```
  The same file documents that the LSQ response is already fully buffered before this point:
  ```rust
  //! Memory profile: pallas-network 0.30's `Client::query<_, AnyCbor>`
  //! buffers the entire response in memory before returning, so the binary
  //! holds the full UTxO CBOR (~multi-GB on mainnet) as a `Vec<u8>` before
  //! the single `tokio::fs::write`.
  ```
- **Suggested fix:** Move the bytes out of `AnyCbor` instead of cloning, compute `len`, then release/done before the local file write if the intent is to minimize node-side acquisition lifetime.
  ```rust
  let bytes = raw.unwrap();
  let len = bytes.len();
  sq.send_release().await.context("LSQ release")?;
  sq.send_done().await.context("LSQ done")?;
  tokio::fs::write(&args.out, bytes)
      .await
      .with_context(|| format!("write {}", args.out.display()))?;
  ```
- **Verification:** From repo root, `rg -n "raw\\.to_vec\\(|send_release|send_done|tokio::fs::write" omega-commitment/crates/omega-utxo-snapshot/src/main.rs` should show no `raw.to_vec()` copy and should show release/done before the write if that ordering is adopted.

## F002 — Headless setup runbook omits the AnyCbor memory and streaming warning

- **Severity:** P2
- **Confidence:** high
- **Location:** `omega-commitment/scripts/setup_headless_node.md:8-13`, `omega-commitment/scripts/setup_headless_node.md:154-176`, `cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md:56-68`
- **Issue:** The source and LSQ wiki page document that `AnyCbor` buffers the whole response and that streaming would require lower-level multiplexer work, but the canonical headless runbook does not. It even advertises `16+ GB RAM` while the UTXO step gives the mainnet command without a warning that the snapshot binary holds a multi-GB response in memory.
- **Evidence:**
  ```markdown
  ## System requirements

  - Linux x86_64 (or arm64 — both binaries available)
  - ~250 GB free disk for the synced ledger DB (217 GiB Mithril snapshot + headroom)
  - 16+ GB RAM (cardano-node 10.7.1's LSM UTxO backend reduces RAM relative to older releases)
  - Network: ~32 MB/s sustained for snapshot download = ~2 h wall time
  ```
  ~~~markdown
  ### 7b. UTXO source — `omega-utxo-snapshot` binary

  ```bash
  cd ~/omega-commitment
  . "$HOME/.cargo/env"
  cargo build -p omega-utxo-snapshot --release

  ./target/release/omega-utxo-snapshot \
    --socket ~/cardano/socket/node.socket \
    --network mainnet \
    --era 6 \
    --out ~/cardano/snapshots/utxo_$(date +%Y%m%d_%H%M%S).cbor
  ```
  ~~~
  The missing content is present elsewhere:
  ```markdown
  Pallas-network 0.30's `Client::query<_, AnyCbor>` **buffers the entire response in memory** before returning. The binary holds the full UTxO CBOR (multi-GB on mainnet) as a `Vec<u8>` before the single `tokio::fs::write`.
  ```
- **Suggested fix:** Add a short paragraph under Step 7b stating that `AnyCbor` buffers the entire LSQ response, mainnet runs should use the high-RAM v1.0 box or equivalent headroom, and the fallback streaming path is lower-level pallas multiplexer parsing.
- **Verification:** From repo root, `rg -n "AnyCbor|buffers the entire response|streaming path|multi-GB|122 GiB" omega-commitment/scripts/setup_headless_node.md` should find the operational warning in the runbook itself.

## F003 — pallas-network 0.30.2 is documented but not reproducibly pinned

- **Severity:** P2
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-utxo-snapshot/Cargo.toml:16-17`, `omega-commitment/.gitignore:1-3`
- **Issue:** The LSQ wire-format evidence is explicitly for pallas-network `0.30.2`, but the binary manifest uses a floating `0.30` requirement and the workspace ignores `Cargo.lock`. A fresh build can therefore resolve a different `0.30.x` than the one audited, undercutting the `queries_v16`/era-index verification basis.
- **Evidence:**
  ```toml
  pallas-network = "0.30"
  pallas-codec = "0.30"
  ```
  ```gitignore
  target/
  Cargo.lock
  *.bk
  ```
  The repo records the audited basis and prior lockfile policy:
  ```markdown
  The pallas-network 0.30.2 CBOR decoder does NOT share the Haskell ledger's Word16-VLE TxIx asymmetry bug
  ```
  ```markdown
  - Cargo.lock now tracked (was previously gitignored; v0.1.0 reviewer flagged this as appropriate for a binary-shipping workspace)
  ```
- **Suggested fix:** Either pin the LSQ binary to the audited exact crate versions:
  ```toml
  pallas-network = "=0.30.2"
  pallas-codec = "=0.30.2"
  ```
  or unignore/check in `Cargo.lock` for the binary-shipping workspace; doing both is the clearest publication posture.
- **Verification:** From repo root, `rg -n "pallas-network = \"=0\\.30\\.2\"|pallas-codec = \"=0\\.30\\.2\"|Cargo.lock" omega-commitment/crates/omega-utxo-snapshot/Cargo.toml omega-commitment/.gitignore` should show exact pallas pins and no active lockfile ignore.
