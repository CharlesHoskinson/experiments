---
agent: A2
lane: code
title: cbor-strictness
files-reviewed:
  - ARCHITECTURE.md
  - README.md
  - cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md
  - omega-commitment/crates/omega-utxo-snapshot/src/main.rs
  - omega-commitment/crates/omega-commitment-ingest/src/lib.rs
  - omega-commitment/crates/omega-commitment-ingest/src/main.rs
  - omega-commitment/crates/omega-commitment-ingest/src/cbor.rs
  - omega-commitment/crates/omega-commitment-ingest/src/utxo.rs
  - omega-commitment/crates/omega-commitment-ingest/src/token_policy.rs
  - omega-commitment/crates/omega-commitment-ingest/src/script.rs
  - omega-commitment/crates/omega-commitment-ingest/src/stake.rs
  - omega-commitment/crates/omega-commitment-ingest/src/governance.rs
  - omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs
  - omega-commitment/crates/omega-commitment-core/src/token_policy_leaf.rs
  - omega-commitment/crates/omega-commitment-core/src/script_registry_leaf.rs
  - omega-commitment/crates/omega-commitment-core/src/stake_state_leaf.rs
  - omega-commitment/crates/omega-commitment-core/src/governance_state_leaf.rs
  - omega-commitment/crates/omega-commitment-core/src/header_leaf.rs
  - omega-commitment/crates/omega-commitment-core/src/tx_index_leaf.rs
  - omega-commitment/crates/omega-commitment-core/src/serde_helpers.rs
  - omega-commitment/crates/omega-commitment-ingest/tests/utxo_ingest_integration.rs
  - omega-commitment/crates/omega-commitment-ingest/tests/token_policy_ingest_integration.rs
  - omega-commitment/crates/omega-commitment-ingest/tests/script_ingest_integration.rs
  - omega-commitment/crates/omega-commitment-ingest/tests/stake_ingest_integration.rs
  - omega-commitment/crates/omega-commitment-ingest/tests/governance_ingest_integration.rs
  - omega-commitment/crates/omega-commitment-ingest/tests/qa_pipeline.rs
  - omega-commitment/crates/omega-commitment-ingest/tests/golden_ingest.rs
  - omega-commitment/crates/omega-commitment-ingest/tests/cli.rs
  - omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_minimal.cbor.md
  - omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor.md
  - omega-commitment/crates/omega-commitment-ingest/tests/fixtures/stake_snapshot.cbor.md
  - omega-commitment/crates/omega-commitment-ingest/tests/fixtures/governance_snapshot.cbor.md
findings-count: { p0: 0, p1: 1, p2: 1, p3: 0 }
---

# Summary

All five current synthetic CBOR decoders end with `cbor::expect_end`, and the leaf preimage encoders use fixed-width big-endian fields rather than ad hoc JSON strings; UTXO assets are sorted before hashing and asset names remain raw bytes until hex serialization. The main strictness gap is that the only producer for real UTxO CBOR writes raw `GetUTxOWhole` bytes, while the ingest crate still only decodes the simplified synthetic fixture shape. The multi-asset parser also accepts duplicate/non-canonical CBOR map keys instead of rejecting malformed bundles.

# Findings

## F001 — Mainnet UTxO CBOR has no decoder behind the snapshot producer

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-ingest/src/utxo.rs:25-66; omega-commitment/crates/omega-utxo-snapshot/src/main.rs:97-109`
- **Issue:** `omega-utxo-snapshot` writes the raw `BlockQuery::GetUTxOWhole` response, but `ingest_utxos` only accepts the hand-crafted 4/6-field fixture format and treats the address as a fixed 32-byte hash. There is no code path that decodes the real mainnet `(TransactionInput, TransactionOutput)` array, pointer-address variable-length fields, or the pointer-address TxIx case documented as the reason for avoiding `cardano-cli`.
- **Evidence:**
  ```rust
  // omega-commitment/crates/omega-utxo-snapshot/src/main.rs:97-109
  let request = Request::LedgerQuery(LedgerQuery::BlockQuery(args.era, BlockQuery::GetUTxOWhole));
  ...
  let raw: AnyCbor = sq
      .query(request)
      .await
      .context("GetUTxOWhole query failed")?;
  
  let bytes = raw.to_vec();
  let len = bytes.len();
  tokio::fs::write(&args.out, &bytes)
  ```
  ```rust
  // omega-commitment/crates/omega-commitment-ingest/src/utxo.rs:25-33
  /// Ingest UTXOs from the simplified CBOR fixture.
  ///
  /// Fixture format (Conway-era LedgerState parsing is future work):
  ///   CBOR array of N UTXOs, each a 4-element array of:
  ///     [ tx_id (32 bytes), output_index (u64), address (32 bytes),
  ///       value_lovelace (u64) ]
  ///   or a 6-element v0.9+ array that appends:
  ///     [ multi_assets, script_credential ]
  ```
  ```rust
  // omega-commitment/crates/omega-commitment-ingest/src/utxo.rs:44-66
  let tx_id = read_32_bytes(&mut d)?;
  let output_index = u32::try_from(read_u64(&mut d)?)
      .map_err(|_| anyhow::anyhow!("output_index too large for u32"))?;
  let address_hash = read_32_bytes(&mut d)?;
  let value_lovelace = read_u64(&mut d)?;
  ...
  expect_end(&d, cbor.len())?;
  ```
  Project docs agree this is still pending: `README.md:210` says the real response shape should be "a CBOR array of 2-element arrays, each `(TransactionInput, TransactionOutput)`", and `README.md:214-218` says the crate currently implements only the synthetic path and the mainnet parser is the next task.
- **Suggested fix:** Add a separate mainnet UTxO parser module and route it through `omega-ingest --format mainnet` (or a checked auto-detect path). Decode the `GetUTxOWhole` array as `(TransactionInput, TransactionOutput)`, parse TxIx/pointer-address variable-length integers as `u64` before any range check, preserve full address/value/multi-asset bytes, and call `expect_end` on the top-level decoder. Include a regression fixture with a pointer address whose TxIx is greater than `65535`.
- **Verification:** From repo root, `rg -n "mainnet|--format|TransactionInput|TransactionOutput|expect_end" omega-commitment/crates/omega-commitment-ingest/src` should show a mainnet route and an end-of-input check; a hand-crafted/pallas fixture containing pointer-address TxIx `65536` should ingest successfully, while the same fixture with one appended byte should fail.

## F002 — Multi-asset maps accept duplicate/non-canonical keys

- **Severity:** P2
- **Confidence:** medium
- **Location:** `omega-commitment/crates/omega-commitment-ingest/src/utxo.rs:70-97; omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs:60-68`
- **Issue:** The asset-bundle parser reads each CBOR map entry and pushes an `Asset` without tracking whether `(policy_id, asset_name)` was already seen or whether map keys were canonical. The leaf encoder sorts by `asset_id`, but it does not reject or coalesce duplicate asset IDs, so malformed CBOR bundles can enter the committed preimage instead of failing strict parsing.
- **Evidence:**
  ```rust
  // omega-commitment/crates/omega-commitment-ingest/src/utxo.rs:79-95
  let n_policies = read_map_len(d)?;
  for _ in 0..n_policies {
      let policy: [u8; 28] = read_28_bytes(d)?;
      let n_assets = read_map_len(d)?;
      for _ in 0..n_assets {
          let name: Vec<u8> = read_var_bytes(d)?;
          let qty: u64 = read_u64(d)?;
          // asset_id = policy_id (28 bytes) || asset_name (variable).
          let mut asset_id = Vec::with_capacity(28 + name.len());
          asset_id.extend_from_slice(&policy);
          asset_id.extend_from_slice(&name);
          assets.push(Asset {
              asset_id,
              quantity: qty,
          });
      }
  }
  ```
  ```rust
  // omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs:60-68
  // Sort assets by asset_id for canonicality
  let mut sorted = self.assets.clone();
  sorted.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
  for a in sorted {
      let id_len =
          u16::try_from(a.asset_id.len()).map_err(|_| LeafError::AssetIdLenOverflow)?;
      out.extend_from_slice(&id_len.to_be_bytes());
      out.extend_from_slice(&a.asset_id);
      out.extend_from_slice(&a.quantity.to_be_bytes());
  ```
  The audit invariant in `instructions.md:19` explicitly asks whether CBOR serialization is canonical, including sorted map keys. Current code canonicalizes output order after parsing but does not enforce canonical input maps or reject duplicate keys.
- **Suggested fix:** Parse multi-assets through a shared helper that inserts into `BTreeMap<([u8; 28], Vec<u8>), u64>` and returns an error on duplicate insertion; use the same helper in `utxo`, `token_policy`, and `script` instead of open-coded map walks. If canonical CBOR input is required, also compare each raw key with the previous raw key and reject out-of-order map keys before emitting the sorted `Vec<Asset>`.
- **Verification:** Add an ingest unit test that builds a 6-element UTXO with the same policy/name key twice and asserts `ingest_utxos`, `ingest_token_policies`, and `ingest_scripts` all return `Err`; add a separate non-UTF8 asset name such as `[0xff, 0x00]` and assert the JSON hex still round-trips byte-for-byte.
