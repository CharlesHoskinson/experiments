---
agent: A3
lane: code
title: cardano-semantics
files-reviewed: [ARCHITECTURE.md, README.md, instructions.md, omega-commitment/README.md, omega-commitment/scripts/dump_ledger_state.sh, omega-commitment/crates/omega-utxo-snapshot/src/main.rs, omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs, omega-commitment/crates/omega-commitment-core/src/stake_state_leaf.rs, omega-commitment/crates/omega-commitment-core/src/governance_state_leaf.rs, omega-commitment/crates/omega-commitment-core/src/script_registry_leaf.rs, omega-commitment/crates/omega-commitment-core/src/token_policy_leaf.rs, omega-commitment/crates/omega-commitment-ingest/src/cbor.rs, omega-commitment/crates/omega-commitment-ingest/src/utxo.rs, omega-commitment/crates/omega-commitment-ingest/src/stake.rs, omega-commitment/crates/omega-commitment-ingest/src/governance.rs, omega-commitment/crates/omega-commitment-ingest/src/script.rs, omega-commitment/crates/omega-commitment-ingest/src/token_policy.rs, omega-commitment/crates/omega-commitment-ingest/src/main.rs, omega-commitment/crates/omega-commitment-ingest/src/lib.rs, omega-commitment/crates/omega-commitment-ingest/examples/probe_ledger_state_paths.rs, omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor.md, omega-commitment/crates/omega-commitment-ingest/tests/fixtures/stake_snapshot.cbor.md, omega-commitment/crates/omega-commitment-ingest/tests/fixtures/governance_snapshot.cbor.md, cardano-wiki/wiki/pages/eutxo-model.md, cardano-wiki/wiki/pages/plutus-and-smart-contracts.md, cardano-wiki/wiki/pages/cip-1694-governance.md, cardano-wiki/wiki/pages/ledger-state-json-layout.md, cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md, cardano-wiki/docs/codex_briefings/2026-05-03-omega-codex-pipeline-update-brief.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md]
findings-count: { p0: 0, p1: 5, p2: 0, p3: 0 }
---

# Summary

The current codebase still commits simplified Cardano projections rather than Conway-faithful state. The highest-risk gaps are in the stable leaf schemas: UTXO leaves collapse addresses and omit inline datum/reference-script state, stake leaves collapse Conway's DRep sum type, and governance leaves commit only one accounting pot. The two-stream v1.0 producer also snapshots each source at "latest" without a shared block/epoch boundary, which is not enough to pin Mark/Set/Go semantics.

# Findings

## F001 — UTXO leaves collapse Cardano addresses to a fixed 32-byte hash

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs:3-47`
- **Issue:** CIP-19 addresses are byte sequences with header-tagged variants: Byron/bootstrap, base, pointer, enterprise, and reward/stake address forms have different payloads and lengths. The committed UTXO leaf has only `address_hash: [u8; 32]`, and the synthetic parser requires a 32-byte address field, so a mainnet parser cannot preserve or prove the exact Cardano address variant.
- **Evidence:**
```rust
//!   (tx_id: 32 bytes) || (output_index: u32 BE) ||
//!   (address_hash: 32 bytes) || (value_lovelace: u64 BE) ||
//!   (asset_count: u32 BE) || ((id_len: u16 BE) || asset_id || (quantity: u64 BE))* ||
//!   (datum_hash: 0x00 or 0x01 || 32 bytes)
```
```rust
#[serde(with = "hex::serde")]
pub address_hash: [u8; 32],
```
`omega-commitment/crates/omega-commitment-ingest/src/utxo.rs:27-48` also documents and parses only `address (32 bytes)`. The local audit invariant explicitly requires Byron bootstrap, pointer, base/enterprise/stake variants to be handled (`instructions.md:21`).
- **Suggested fix:** Replace `address_hash` with a canonical raw Cardano address encoding, for example `address: Vec<u8>` plus an encoded length in the UTXO preimage. The mainnet parser should preserve the raw ledger address bytes and add tests/fixtures for Byron/bootstrap, pointer, base, enterprise, and reward/stake-address encodings.
- **Verification:** From repo root, `rg -n "address_hash|address \\(32 bytes\\)|Byron bootstrap|pointer addresses" omega-commitment instructions.md` should show no remaining fixed 32-byte address commitment path, and new tests should include the named CIP-19 variants.

## F002 — UTXO leaves do not commit CIP-32 inline datums or CIP-33 reference scripts

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs:3-47`
- **Issue:** The UTXO leaf commits only an optional 32-byte datum hash and has no `datum_option` sum or `script_ref` field. CIP-32 inline datums and CIP-33 reference scripts are UTXO-resident state; the current parser even consumes script credentials into the script sub-tree and explicitly does not surface them in UTXO JSON.
- **Evidence:**
```rust
//!   (asset_count: u32 BE) || ((id_len: u16 BE) || asset_id || (quantity: u64 BE))* ||
//!   (datum_hash: 0x00 or 0x01 || 32 bytes)
```
```rust
#[serde(default, with = "crate::serde_helpers::opt_hex")]
pub datum_hash: Option<[u8; 32]>,
```
`omega-commitment/crates/omega-commitment-ingest/src/utxo.rs:57-64` always emits `datum_hash: None`, and `omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor.md:38-41` says script credentials are not surfaced in UTXO output. The repo's EUTXO summary records inline datums and reference scripts as on-chain output features (`cardano-wiki/wiki/pages/eutxo-model.md:51-53`).
- **Suggested fix:** Model the ledger output as `datum_option = none | hash([u8;32]) | inline(plutus_data_cbor)` and add `script_ref = none | some(language, raw_script_cbor/hash/size as specified)`. Encode both as tagged, length-delimited fields in `Utxo::encode`, and have the LSQ mainnet parser preserve these fields from each Conway `TxOut`.
- **Verification:** `rg -n "datum_hash|datum_option|script_ref|reference scripts|inline datums" omega-commitment cardano-wiki` should show UTXO leaf fields for `datum_option` and `script_ref`, not only a datum hash and script-registry side channel.

## F003 — Stake leaves cannot encode the Conway DRep sum type

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-core/src/stake_state_leaf.rs:13-41`
- **Issue:** Conway DRep delegation is a sum type, not a raw 28-byte hash. The leaf stores only `delegated_drep: [u8; 28]`, losing key-vs-script DRep credential tags and leaving no representable value for `AlwaysAbstain` or `AlwaysNoConfidence`; comments assert those are "literal 28-byte values", but the upstream type is constructor-tagged.
- **Evidence:**
```rust
//! - `delegated_drep == [0u8; 28]` means no active DRep delegation.
//!   The canonical "always-abstain" and "always-no-confidence" DRep IDs
//!   are upstream Cardano constants and are stored as their literal
//!   28-byte values (NOT encoded as zero).
```
```rust
pub delegated_drep: CredentialHash,
```
The local governance summary lists the two default voting options (`cardano-wiki/wiki/pages/cip-1694-governance.md:41-45`), but the fixture format is still a 5-element array whose DRep field is only `delegated_drep (28 bytes)` (`omega-commitment/crates/omega-commitment-ingest/tests/fixtures/stake_snapshot.cbor.md:5-14`).
- **Suggested fix:** Introduce an explicit DRep enum in the core schema, for example `None | KeyHash([u8;28]) | ScriptHash([u8;28]) | AlwaysAbstain | AlwaysNoConfidence`, and encode it with a stable tag byte before any 28-byte payload. Do the same for stake credentials where key/script distinction matters.
- **Verification:** `rg -n "delegated_drep|AlwaysAbstain|AlwaysNoConfidence|DRep" omega-commitment cardano-wiki` should show a tagged enum in `stake_state_leaf.rs` and fixture coverage for both special constructors.

## F004 — Snapshot source is not pinned to a common epoch/block or Mark/Set/Go selection

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-utxo-snapshot/src/main.rs:80-97`
- **Issue:** The UTXO stream acquires `latest tip`, while `dump_ledger_state.sh` separately queries tip and then runs `ledger-state`; no common block point, stability depth, or epoch-boundary contract is stored with both inputs. Stake semantics also depend on which of `pstakeMark`, `pstakeSet`, and `pstakeGo` is selected, but the leaf schema has no snapshot label/epoch.
- **Evidence:**
```rust
sq.acquire(None)
    .await
    .context("acquire latest tip via LocalStateQuery")?;
```
`omega-commitment/scripts/dump_ledger_state.sh:82-116` separately queries tip and dumps ledger state, and the verified JSON layout exposes all three rolling snapshots (`cardano-wiki/wiki/pages/ledger-state-json-layout.md:47-50`). `ARCHITECTURE.md:35` says the plan pins `pstakeSet`, but the code does not enforce any selection.
- **Suggested fix:** Add an explicit snapshot manifest used by both streams: target block hash, slot, epoch, required stability depth, and selected stake snapshot (`mark|set|go`) with the rationale. Use `acquire(Some(point))` or equivalent fixed-point LSQ for UTXO and ledger-state sources, and include the selected snapshot label/epoch in the stake ingestion output or metadata committed alongside the root.
- **Verification:** `rg -n "acquire\\(None\\)|pstakeMark|pstakeSet|pstakeGo|snapshot" omega-commitment cardano-wiki ARCHITECTURE.md` should show fixed-point acquisition and a single documented/encoded Mark/Set/Go choice.

## F005 — AccountState pots are not fully committed

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-core/src/governance_state_leaf.rs:19-30`
- **Issue:** The governance fact kinds commit treasury balance, committee seats, and governance actions, but not reserves, deposits, or the fee pot. The v1.0 docs record those ledger-state fields as present (`reserves`, `treasury`, `deposited`, `fees`), yet the core schema and fixture only have a treasury kind, so the remaining accounting state is silently outside the commitment.
- **Evidence:**
```rust
//! - `0` — Treasury balance. `key` = all-zero. `value` = lovelace balance.
//! - `1` — CC seat. `key` = member's credential hash (right-padded from
//!   28 bytes to 32 with zeros). `value` = expiration epoch.
//! - `2` — Ratified gov action. `key` = action's tx_id (full 32 bytes).
//! - `3` — In-flight gov action. `key` = action's tx_id. `value` = packed
```
The v1.0 plan lists `stateBefore.esLState.utxoState.{deposited, fees, ppups}` and `stateBefore.esChainAccountState.{reserves, treasury}` as available input (`cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md:49-52`). The probe only records `reserves` and `treasury` paths (`omega-commitment/crates/omega-commitment-ingest/examples/probe_ledger_state_paths.rs:37-39`), and the governance fixture's sole accounting fact is treasury (`omega-commitment/crates/omega-commitment-ingest/tests/fixtures/governance_snapshot.cbor.md:17-24`).
- **Suggested fix:** Add governance/accounting fact kinds for reserves, deposits/deposited, current fee pot, and snapshot fee pot if needed by reward semantics. The mainnet governance parser should fail closed if any expected accounting field is missing, and fixtures/goldens should include non-zero values for every pot.
- **Verification:** `rg -n "reserves|treasury|deposited|fees|feeSS|kind" omega-commitment cardano-wiki` should show explicit committed facts and tests for each accounting pot.
