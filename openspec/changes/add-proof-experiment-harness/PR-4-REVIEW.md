# PR #4 review — feat: add omega mock ledger storage

Branch: `feat/omega-mock-ledger-group4`
Head: `dcdf5bd9bd828d60fd79658d33a4b2841fae4d70`
Status: OPEN, CI green, +2846/-35 across 15 files
Reviewer: Senior code reviewer, 2026-05-04
Verdict: **approve** (with notes on deferred work)

---

## Summary

PR #4 lands the SQLite-backed mock ledger that openraft 0.9 will sit on top of. The most concurrency-sensitive decision in the harness — actor-pattern writer vs per-call `spawn_blocking` — is implemented correctly: a dedicated OS thread (`std::thread::spawn`, named `omega-mock-ledger-writer`) owns the rusqlite write `Connection` and consumes a `tokio::sync::mpsc::UnboundedReceiver<WriteCmd>` via `blocking_recv`; the openraft `RaftStateMachine::apply_to_state_machine` path sends a single `WriteCmd::ApplyRaftEntries` and awaits a `oneshot` reply. There is no per-call `spawn_blocking` on the writer hot path. PRAGMAs match the spec (with `mmap_size` correctly Windows-disabled), the schema is `WITHOUT ROWID` with a composite `(sub_tree_id, leaf_index)` PK on `nullifiers`, the apply pipeline calls `omega_claim_verifier::verify` before any state mutation and uses the v2 `ClaimPublicInputs` shape with `tree_depth` + `per_sub_tree_root`, and replay is rejected with a typed `LedgerError::Replay` after a `SELECT 1` probe inside the same transaction as the insert. Tests are honest: 4.11 and 4.12 are scoped to single-node storage assertions and the open-box rationale is documented in `tasks.md` lines 67-69. Snapshot create flows through the writer channel; restore is implemented as an actor-serialised table copy and the box-4.7-open rationale (drop-live-DB-and-rename needs reader-pool teardown on Windows) is honest. `cargo doc --workspace --no-deps --document-private-items` and `cargo clippy -p omega-mock-ledger --all-targets -- -D warnings` are both clean on the 1.95.0 toolchain. All six mock-ledger tests pass locally (apply_smoke 25.45 s, concurrent_readers 5.07 s, load_heartbeat 60.06 s, restart_durability 29.20 s, schema 0.05 s, storage 0.08 s). The cardano feature compiles. This is a good PR; the open boxes are honest debt and belong to group 6 (omega-toy-consensus) plus a Windows-specific cleanup pass.

---

## Strengths

Load-bearing decisions this PR got right.

- **Actor-pattern writer**, `src/writer.rs:22-26` and `src/writer.rs:210-275`. The writer runs on a `std::thread::Builder::new().name("omega-mock-ledger-writer").spawn(...)`, NOT a `tokio::task::spawn` or `tokio::task::spawn_blocking`. The thread function `run_writer` loops on `rx.blocking_recv()`. The `Connection` lives entirely inside that thread — never shared with reader tasks, never wrapped in `Mutex`/`RwLock`. This is the single highest-stakes concurrency decision in the harness and it is correct.
- **One channel, all writes**, `src/writer.rs:159-201`. Every state-mutating operation (`ApplyClaim`, `Snapshot`, `RestoreSnapshot`, `CheckpointWalTruncate`, `SaveRaftMeta`, `AppendRaftLogs`, `DeleteRaftLogsSince`, `PurgeRaftLogsUpto`, `ApplyRaftEntries`, `InsertSyntheticClaim`) flows through the same `mpsc::UnboundedSender<WriteCmd>`. Snapshots serialise against ordinary writes via channel ordering — no separate mutex needed. This is exactly what `design.md:191-194` mandates ("the channel is the mutex").
- **Apply pipeline does verify-before-mutate**, `src/writer.rs:284-310`. `apply_claim_tx` calls `omega_claim_verifier::verify(commitment, &public_inputs, &proof)` before opening the SQLite transaction. Replay check (`reject_replay`) runs inside the same transaction as `insert_nullifier`/`insert_starstream_utxo`. If the verifier rejects, no state mutates; if any insert fails, the transaction rolls back as a whole.
- **v2 wire format integrated**, `src/writer.rs:284-290` and tests `tests/apply_smoke.rs:64-72`, `tests/restart_durability.rs:63-71`. The apply path reads the parsed `ClaimPublicInputs` (now carrying `tree_depth` + `per_sub_tree_root` per task 2.4.4) and hands the slice to `verify`. The verifier's `WrongSubTreeRoot` / `DepthMismatch` paths surface through `LedgerError::Verify(VerifyError)` as a proper `#[from]`.
- **Replay defence**, `src/writer.rs:339-355`. Two-step: `SELECT 1 FROM nullifiers WHERE sub_tree_id=?1 AND leaf_index=?2` inside the transaction, then `INSERT` with the composite PK. Returns `LedgerError::Replay { sub_tree_id, leaf_index }` on collision. `tests/apply_smoke.rs:147-148` exercises the rejection.
- **PRAGMAs verified at runtime, not just set**, `src/schema.rs:81-99` + `tests/schema.rs:21-26`. The crate exposes a `pragma_snapshot()` reader that asks SQLite for each PRAGMA back, and the test asserts `journal_mode=wal`, `synchronous=1`, `cache_size=-65536`, `temp_store=2`, `wal_autocheckpoint=10000`, `auto_vacuum=0`. This is round-trip evidence, not "I set the PRAGMA and trust it stuck".
- **`mmap_size` is conditional on platform**, `src/schema.rs:35-36`: `#[cfg(not(windows))] conn.execute_batch("PRAGMA mmap_size = 268435456;")?;`. Matches `design.md:303-304` exactly.
- **Schema correctness**, `src/schema.rs:42-77`. All five tables (`raft_log`, `raft_meta`, `nullifiers`, `starstream_utxos`, `genesis`) are `WITHOUT ROWID`. `nullifiers` has the composite PK `(sub_tree_id, leaf_index)` per `design.md:166-170`. No foreign keys. `starstream_utxos.utxo_id BLOB PRIMARY KEY` matches the spec's "32-byte hash of (recipient || value || ...)".
- **Reader pool sized to `num_cpus::get()`**, `src/lib.rs:100-102`. Matches `design.md:194` exactly. Reader queries route through `tokio::task::spawn_blocking` (`src/lib.rs:148-159`, `src/lib.rs:163-171`, `src/lib.rs:175-183`, `src/storage.rs:81-94`), which is the *correct* place to use `spawn_blocking` per the design — it does NOT block the writer.
- **`#[doc(hidden)]` on test-only public API**, `src/lib.rs:215-225`. `MockLedger::insert_synthetic_claim_for_test` is marked `#[doc(hidden)]` per the SKILL's rule 273-275. It needs to be `pub` for cross-crate test consumption; the doc-hidden attribute keeps it out of `cargo doc` output.
- **Crate-level `#![allow(clippy::result_large_err)]` is documented**, `src/lib.rs:1-8`. The justification names the openraft 0.9 `StorageError<u64>` size (224 bytes), the Rust 1.95 lint trigger, and the openraft 0.10 forward path. This is the comment style PR #3 introduced.
- **`pallas_validate::phase1::validate_tx` signature quoted**, `src/cardano.rs:1-18`. Verbatim quote of the alpha.6 signature (`(metx: &MultiEraTx, txix: TransactionIndex, env: &Environment, utxos: &UTxOs, cert_state: &mut CertState) -> ValidationResult`) per task 4.2's "before writing test code, quote the signature" requirement. The module is appropriately gated `#[cfg(feature = "cardano-tx-validation")]`.
- **Periodic WAL checkpoint task**, `src/lib.rs:189-205`. `spawn_wal_truncate_task` defaults to 30 s per `design.md:201`. The interval is parameterisable (`spawn_wal_truncate_task_with_interval`) which makes test-time fast. The checkpoint flows through the writer channel (`src/writer.rs:47-53`) so it's serialised against ordinary writes — no concurrent writer/`PRAGMA wal_checkpoint(TRUNCATE)` race.
- **`apply_to_state_machine` saves last-applied-log-id atomically with the entry effect**, `src/writer.rs:553-583`. After each entry's apply, `save_raft_meta(conn, "last_applied_log_id", ...)` runs on the same connection in the same transaction context. This is the openraft "monotonic last-applied" invariant; getting it wrong is a classic durability bug.

---

## Findings

### P0

None. The single highest-stakes concurrency decision (actor pattern, not per-call `spawn_blocking`) is correct, the apply pipeline does verify-before-mutate, replay defence is sound, the schema matches the spec, PRAGMAs match the spec, and v2 wire format is integrated.

### P1

- **P1-1: `apply_to_state_machine` is not strictly atomic across all entries.** `src/writer.rs:553-583`. The function loops over `entries` and calls `apply_claim_tx` per entry. Each `apply_claim_tx` opens its own SQLite transaction (`src/writer.rs:292`). Between entries, the writer thread's connection is in autocommit. If the process dies mid-batch, entries 0..k commit and entries k+1..n do not, but `last_applied_log_id` is updated *after* each entry, so the persisted `last_applied_log_id` matches the persisted state. That part is fine. The subtle issue: if `apply_claim_tx` for entry k returns `Err` (verifier rejects, replay collision, etc.), the function records a `LedgerResponse::rejected` but **still** writes `last_applied_log_id = entry_k.log_id`. This is correct openraft semantics (the entry was "applied" — the state machine declined to mutate, but the log index advanced), and a future replay during recovery will see entry k as already applied and not re-attempt. So the existing behaviour is correct, but it's not load-bearing-obvious from the code; a `# Soundness` block on `apply_raft_entries` explaining "rejected entries still advance `last_applied_log_id`" would close the door on a future contributor "fixing" it.
- **P1-2: `apply_raft_entries` does not capture the `Membership` payload from `EntryPayload::Membership`** when computing `last_membership` to persist. `src/storage.rs:278-284` *does* compute `last_membership` from `entries` in `apply_to_state_machine` and passes it down; `src/writer.rs:553-583` writes it after the entry loop. So this works for the openraft contract — but the placement is fragile: if a `Membership` entry comes mid-batch and a later `Normal` entry rejects, `last_applied_log_id` is the last `Normal` entry's id, while `last_membership` is the post-Membership stored value. That's still correct (both are durable, both are monotonic), just non-obvious. Recommend a comment.
- **P1-3: `restore_snapshot`'s table-copy strategy elides `genesis` versioning.** `src/writer.rs:430-456` does `DELETE FROM main.<t>; INSERT INTO main.<t> SELECT * FROM snapshot_db.<t>` for the five tables. The `raft_log` truncation here is intentional (an installed snapshot supersedes the log up to its index). Two concerns:
  1. The function deletes from `main.raft_log` then re-inserts from snapshot. If the snapshot was taken at log index N and the receiver had log entries up to N+M before the install, those M entries are silently dropped. This is the correct openraft behaviour for `install_snapshot`, but again — non-obvious from the code; a comment naming "this is openraft's installed-snapshot semantics, post-install local log is wholly replaced" would make the design choice explicit.
  2. The comment at `tasks.md:68` says "the exact drop-live-DB-and-rename restore path needs reader-pool teardown before it is safe on Windows". The implemented table-copy approach is a reasonable v0.1 fallback but it also means snapshot restore acquires a write transaction across the entire DB while readers still hold pool connections. On a busy system this could time out with `SQLITE_BUSY`. Document the trade-off in the docstring; consider a follow-up issue.
- **P1-4: Doc compliance vs `omega-rustdoc-style` SKILL is incomplete.** `src/lib.rs` has a 9-line crate-level doc block (lines 9-16) that names the actor-pattern rationale, but the SKILL's "crate-level docs" section (SKILL.md:182-229) requires (1) elevator pitch, (2) design-context links to the OpenSpec change + spec, (3) tier-of-trust statement, (4) v0.1 limitations, (5) crate-specific conventions. This crate has none of (2)-(5). The crate is soundness-bearing per the SKILL's table (SKILL.md:91 — "MockLedger::open, the SQLite WITHOUT ROWID schema, the actor pattern" is named). It needs the full crate-level docstring before task 12.7 can tick green. Public items (`MockLedger::open`, `apply_claim`, `nullifier_exists`, `MockLedgerStorage`, `PragmaSnapshot`, `LedgerCommand`, `LedgerResponse`, `OmegaRaftTypeConfig`, the `LedgerError` variants) lack one-line summaries, `# Errors` blocks, and `# Soundness` blocks. `cargo doc` is clean only because the workspace does not yet have `#![warn(missing_docs)]` on this crate. Task 12.7 is the gate that catches this; do not tick task 12.7 until the doc retrofit lands.
- **P1-5: `apply_claim`'s soundness block is missing.** `src/lib.rs:131-140`. The function is the public entry point for the apply pipeline — it is the soundness boundary between an untrusted `ClaimTx` from the network and the persisted state machine. The SKILL explicitly names "`apply_pipeline`, the nullifier-collision check" as soundness-bearing (SKILL.md:89). A `# Soundness` block must answer: what attack does it close (forged proof, replay, malformed payload), what does it preserve (verifier-attests-then-state-mutates ordering, transactional all-or-nothing on the insert pair), what does it NOT preserve (consensus ordering — that's openraft's job; signature checking — C6 mocked v0.1).
- **P1-6: `nullifier_exists` returning false is not a *negative* assertion.** `src/lib.rs:142-159`. This function reads from a snapshot of the SQLite reader pool. Under WAL it sees a consistent point-in-time, but a writer mid-flight can have an uncommitted insert; the read returns "false" while the write is pending. For a Raft state machine query this is fine (every node observes the same applied state once `apply` returns), but a CLI user calling `nullifier_exists` to "check before submit" can race. The function as documented is fine; the future-CLI hazard is a P2 docstring issue but I'm flagging it as P1 because the docstring is currently zero lines long (no warning, no semantics).

### P2

- **P2-1: `payload_value` is fragile.** `src/writer.rs:400-407`. Reads a u64 BE from bytes 8..16 of the leaf payload. With the v0.1 leaf-size restriction (≤ 64 bytes total preimage), all test payloads are 16 bytes, so payload[8..16] is valid. But the function silently returns 0 for `payload.len() < 16`, which means a 0-byte or 7-byte payload produces a zero-value Starstream UTxO without complaint. Either return `Result<u64, LedgerError>` and surface the malformed-payload error, or document that the function intentionally returns 0 for short payloads. Currently it's lossy.
- **P2-2: `starstream_utxo_id` collision domain is single-node-deterministic, not chain-deterministic.** `src/lib.rs:232-246`. Includes `block_idx` and `ordinal` (the per-batch ordinal) in the preimage, plus the recipient and the leaf payload. Different nodes processing the same Raft entry will get the same `block_idx` and `ordinal` (Raft replicates entries, not per-node clocks), so the UTxO id is replicated-deterministic. Good. Less obvious: the domain tag is `b"omega:mock-ledger:v1:utxo"`, not `b"omega:v2:..."`, so this UTxO id space is intentionally distinct from the leaf-hash domain. Document that.
- **P2-3: `pragma_snapshot` swallows the `mmap_size` error path on Windows.** `src/schema.rs:86`: `mmap_size: pragma_i64(conn, "mmap_size").unwrap_or(0)`. On Windows the PRAGMA was never set, so the read returns 0 and we report "0" as the snapshot. That's correct behaviour (mmap is intentionally disabled there) but the test `tests/schema.rs` does not assert anything about `mmap_size`, so a regression where Linux *also* fails to set mmap would slip past. Add `#[cfg(not(windows))] assert_eq!(pragmas.mmap_size, 268_435_456);` to `tests/schema.rs`.
- **P2-4: `LedgerError` is `#[non_exhaustive]` (good) but missing variants for snapshot lifecycle.** `src/lib.rs:46-69`. There's no `Snapshot { ... }` variant — snapshot failures fall through `Sqlite` / `Io`. Not wrong, but the apply pipeline distinguishes `Replay` from `Sqlite`, and snapshot is similarly important; consider a typed variant for clearer error reporting up the openraft stack.
- **P2-5: Tests use process-id + thread-name for uniqueness.** Every test file has `temp_db_path(name)` that interpolates `std::process::id()` and `std::thread::current().name()`. Single-threaded `cargo test --test foo`-style runs are fine, but parallel test runners (e.g. `cargo nextest`) sharing the same process id but different thread names will each get a distinct path — also fine. The tests do `let _ = std::fs::remove_file(&path)` at start which means a stale file from a previous crashed run is ignored. There's no cleanup at end (no `Drop` on the temp file). Not a correctness bug; left-over files accumulate in `%TEMP%` over many CI runs. Consider `tempfile::TempDir`.
- **P2-6: `tests/apply_smoke.rs` is the *only* test that exercises the snapshot create + restore round-trip end-to-end.** Lines 118-141 do a `snapshot` → write more → `restore_snapshot` → assert restored count. That's good coverage for the happy path, but there's no "snapshot during concurrent readers", no "snapshot fails partway through", no "restore from a snapshot generated by an older schema version". For v0.1 these are reasonable to defer; flag them in tasks.md or follow-up issues.

---

## Spec / contract checklist

### A. Actor-pattern correctness

**Pass.** The writer is a dedicated OS thread.

- `src/writer.rs:22-26`: `thread::Builder::new().name("omega-mock-ledger-writer".to_string()).spawn(move || run_writer(path, conn, rx))` — dedicated `std::thread::spawn`, not a tokio task.
- `src/writer.rs:210-211`: `fn run_writer(path: PathBuf, mut conn: Connection, mut rx: mpsc::UnboundedReceiver<WriteCmd>)` followed by `while let Some(cmd) = rx.blocking_recv()` — synchronous loop, owns the `Connection` by value.
- `src/writer.rs:13-16`: `WriterHandle { tx: mpsc::UnboundedSender<WriteCmd> }` — Tokio mpsc, the handle is `Clone`-able to multiple producers.
- `src/writer.rs:35-44` (and every other `apply_*`/`save_*` method): each call sends a `WriteCmd::*` carrying a `oneshot::Sender` for the reply, then `await`s the oneshot — exactly the pattern named in `design.md:191-194`.
- The apply path `MockLedgerStorage::apply_to_state_machine` (`src/storage.rs:274-290`) calls `self.ledger.writer.apply_raft_entries(entries.to_vec(), last_membership).await` — no `spawn_blocking` on the writer hot path.
- `Connection` is owned by the writer thread and never escapes; readers use a separate `r2d2_sqlite::SqliteConnectionManager` pool (`src/lib.rs:96-102`).
- Snapshot commands flow through the same channel (`src/writer.rs:55-72`, `src/writer.rs:164-171`, `src/writer.rs:224-232`) — no separate mutex.

The single highest-stakes contract in this PR is satisfied.

### B. PRAGMA correctness

**Pass.** All seven PRAGMAs from `design.md:148-154` and `spec.md:59`:

- `journal_mode=WAL` — `src/schema.rs:26` + `tests/schema.rs:21`.
- `synchronous=NORMAL` (=1) — `src/schema.rs:27` + `tests/schema.rs:22`.
- `cache_size=-65536` — `src/schema.rs:28` + `tests/schema.rs:23`.
- `mmap_size=268435456` Linux/macOS only — `src/schema.rs:35-36` (gated `#[cfg(not(windows))]`). Matches `design.md:303-304`.
- `temp_store=MEMORY` (=2) — `src/schema.rs:29` + `tests/schema.rs:24`.
- `wal_autocheckpoint=10000` — `src/schema.rs:30` + `tests/schema.rs:25`.
- `auto_vacuum=NONE` (=0) — `src/schema.rs:31` + `tests/schema.rs:26`.

P2-3 above flags that `mmap_size` does not have a positive Linux assertion; minor.

### C. Schema correctness

**Pass.** `src/schema.rs:42-77`:

- `raft_log` `WITHOUT ROWID`, PK `log_idx`, columns `(log_idx, term, payload)`. ✓
- `raft_meta` `WITHOUT ROWID`, PK `k TEXT`, columns `(k, v)`. ✓
- `nullifiers` `WITHOUT ROWID`, composite PK `(sub_tree_id, leaf_index)`, columns `(sub_tree_id, leaf_index, block_idx)`. ✓ — this is the "composite PK on nullifiers" the QA review explicitly praised.
- `starstream_utxos` `WITHOUT ROWID`, PK `utxo_id BLOB`, columns `(utxo_id, recipient, value, asset_blob, datum, script_ref, block_idx, spent_in)`. ✓
- `genesis` `WITHOUT ROWID`, PK `k TEXT`, columns `(k, v)`. ✓
- No foreign keys. ✓
- `tests/schema.rs:28-36` asserts every table exists.

### D. Apply pipeline correctness (verify-before-mutate)

**Pass.** `src/writer.rs:277-311`:

```
parse_claim(claim)? — split into public_inputs, witnesses, proof
verify(commitment, &public_inputs, &proof)? — Plonky3 STARK verify
let tx = conn.transaction()? — open SQLite transaction
for public in &public_inputs: reject_replay(&tx, public)? — SELECT 1 probe
for (public, witness) zipped: insert_nullifier + insert_starstream_utxo
tx.commit()?
```

The verifier runs *before* any state mutation. If `verify` fails, no transaction opens. If a replay is detected, the transaction's inserts never run and the rollback is implicit on `tx` drop. `tests/apply_smoke.rs:147-148` asserts that a replayed claim returns `LedgerError::Replay { .. }` and `tests/apply_smoke.rs:114-130` asserts a successful first apply produces exactly one nullifier row + one starstream-utxo row.

### E. v2 wire format

**Pass.** The apply pipeline reads the v2-shape `ClaimPublicInputs` (with `tree_depth` + `per_sub_tree_root`) and passes the slice to `verify`. Evidence:

- `src/writer.rs:284-290`: `parse_claim(claim)?` produces `public_inputs: Vec<ClaimPublicInputs>` from the typed `omega_claim_tx` types.
- `omega-claim-tx/src/lib.rs:237-244` (verified above): `ClaimPublicInputs` carries `tree_depth: u8` and `per_sub_tree_root: Hash` per task 2.4.4.
- `tests/apply_smoke.rs:64-72` and `tests/restart_durability.rs:63-71` construct `ClaimPublicInputs` with both fields populated from `tree.depth() as u8` and `tree.root()`.
- The verifier `omega_claim_verifier::verify` (signature confirmed at `omega-claim-verifier/src/lib.rs:262-265`) takes `&[ClaimPublicInputs]` and surfaces `WrongSubTreeRoot` / `DepthMismatch` per the verifier's `# Soundness` block (SKILL.md:120-138, mirrored in the verifier's lib.rs).

No v1-shape `ClaimPublicInputs` construction exists in this PR.

### F. Nullifier replay defence

**Pass.** `src/writer.rs:339-355` (`reject_replay`) issues `SELECT 1 FROM nullifiers WHERE sub_tree_id=?1 AND leaf_index=?2` against the *transaction* (not a separate read), and on a hit returns `LedgerError::Replay { sub_tree_id, leaf_index }` (a typed variant of the `#[non_exhaustive]` `LedgerError` enum). The subsequent `INSERT INTO nullifiers` (`src/writer.rs:357-368`) does NOT use `INSERT OR ABORT` — it relies on the explicit probe — but the composite PK guarantees a duplicate insert would fail with a UNIQUE constraint violation surfaced as `LedgerError::Sqlite(...)`. The probe-then-insert pattern is what `spec.md:69-71` mandates ("the apply pipeline returns `ApplyError::NullifierExists` and the SQLite transaction rolls back leaving the table unchanged"). `tests/apply_smoke.rs:147-148`: `let err = ledger.apply_claim(8, &commitment, claim).await.unwrap_err(); assert!(matches!(err, LedgerError::Replay { .. }));`.

### G. `#[doc(hidden)]` on `insert_synthetic_claim_for_test`

**Pass.** `src/lib.rs:215-225`. The function is `pub` (it has to be — it's used by `tests/concurrent_readers.rs:42`, `tests/load_heartbeat.rs:43`, and `tests/apply_smoke.rs:133` from outside the crate's `src/`) and is correctly marked `#[doc(hidden)]`. The corresponding writer method `WriterHandle::insert_synthetic_claim_for_test` (`src/writer.rs:74-90`) is `pub(crate)`, which is correct — the public surface is only `MockLedger::insert_synthetic_claim_for_test`.

### H. Doc compliance per `omega-rustdoc-style`

**Partial pass.** This is P1-4 above, repeated here for the checklist:

- Crate-level docstring at `src/lib.rs:9-16` is present but covers only the actor-pattern rationale. Missing per SKILL.md:182-229: design-context links (OpenSpec change + spec.md), tier-of-trust statement (this is a soundness-bearing crate per SKILL.md:91), v0.1 limitations enumeration, conventions specific to this crate.
- Public items (`MockLedger::open`, `apply_claim`, `nullifier_exists`, `nullifier_count`, `starstream_utxo_count`, `pragma_snapshot`, `table_exists`, `path`, `checkpoint_wal_truncate`, `spawn_wal_truncate_task`, `snapshot`, `restore_snapshot`, `MockLedgerStorage::new`, `MockLedgerStorage::openraft_parts`, `LedgerCommand`, `LedgerResponse`, `OmegaRaftTypeConfig`, `PragmaSnapshot`, every `LedgerError` variant) lack docstrings entirely.
- Soundness-bearing items (per SKILL.md:91) need `# Soundness` blocks: `apply_claim`, `MockLedger::open` (the schema initialiser is the integrity boundary), `nullifier_exists` (the replay-protection probe).
- `# Errors` blocks missing on every `Result`-returning public method.

`cargo doc --workspace --no-deps --document-private-items` is clean (verified locally) only because the crate does not have `#![warn(missing_docs)]` set yet. Task 12.7 is the gate that catches this; do not tick 12.7 until the retrofit lands. Recommend: ship doc-retrofit as a follow-up PR alongside the same retrofit for `omega-claim-tx`, `omega-claim-prover`, `omega-claim-verifier` (all of which have similar gaps in this PR's view of the workspace).

### I. Test rigor — what's NOT tested

**Honest open boxes.**

- **4.11 (`tests/load_heartbeat.rs`)**: implemented as a *single-node* heartbeat-continuity test. A 60-second writer storm runs concurrently with a 250 ms tokio interval ticker; the test asserts `ticks >= 200` and `max_gap < 1_000 ms`. This is the storage-side actor-pattern stress test. It does *not* exercise leader continuity in a real 3-node cluster. The PR body and `tasks.md:67` are honest about this — the box is left open until `omega-toy-consensus` (group 6) lands real Raft on top of this storage. The single-node version *is* the test that would surface the spawn-blocking-thundering-herd bug if the actor pattern were botched (a saturated writer would starve the heartbeat ticker; the current code's `max_gap < 1_000` is a real assertion against that). 60.06 s runtime confirmed locally.
- **4.12 (`tests/restart_durability.rs`)**: implemented as a *three-storage* (not three-node) test — opens three independent SQLite ledgers, applies the same claim to each, drops, reopens, asserts each persisted the nullifier + starstream UTxO. This exercises the storage-side persistence story but does *not* exercise quorum re-election (no real openraft + libp2p). PR body and `tasks.md:67` are honest. The single-node restart half is the *correct* coverage for this PR; the cluster-level half is group 6's debt. 29.20 s runtime locally.
- **4.7 (snapshot restore)**: this *is* implemented (`src/writer.rs:430-456`, exposed via `MockLedger::restore_snapshot` and `MockLedgerStorage::install_snapshot`), and `tests/apply_smoke.rs:118-141` exercises a full create + restore round-trip. The open box is specifically about the design's named "drop live DB and rename" path — the implementation uses `ATTACH DATABASE + table-copy` instead, which the PR-body+tasks.md call out as a Windows-specific concession. Not half-implemented; alternatively-implemented with the rationale documented. P1-3 above flags the trade-off documentation gap.
- **4.13 (Cardano fixtures)**: open. The `pallas_validate::phase1::validate_tx` signature is quoted in `src/cardano.rs:7-14`, the feature compiles (`cargo build -p omega-mock-ledger --features cardano-tx-validation` clean locally), but no actual Conway-era fixture test exists. PR body honest. Acceptable to defer to a follow-up since the design (M9 in QA-REVIEW.md) names this as a known gap.

### J. Cardano tx validation

**Mostly pass — feature shell only.**

- `pallas_validate::phase1::validate_tx`'s actual signature is quoted in `src/cardano.rs:7-14` per task 4.2. ✓
- Feature gating: `Cargo.toml:11` `cardano-tx-validation = ["dep:pallas-validate"]`, default off (`default = []`). `pallas-validate` is `optional = true` at line 22. ✓ — matches `spec.md:103` ("When the feature is disabled the harness MUST compile and run without `pallas-validate` in its dependency tree").
- The feature compiles: `cargo build -p omega-mock-ledger --features cardano-tx-validation` finishes clean.
- No real Conway fixtures (4.13 open, honest).

### K. Workspace hygiene

**Pass.**

- All deps in `Cargo.toml` use `.workspace = true` for shared deps (`hex`, `num_cpus`, `openraft`, `postcard`, `r2d2`, `r2d2_sqlite`, `rusqlite`, `serde`, `thiserror`, `tokio`, optional `pallas-validate`). Path deps for in-workspace crates (`omega-claim-tx`, `omega-claim-verifier`, `omega-commitment-core`, `omega-claim-prover`).
- Workspace pin uses `=` versions (`openraft = "=0.9.24"`, `rusqlite = "=0.32.1"`, `pallas-validate = "=1.0.0-alpha.6"`, `num_cpus = "=1.17.0"`) — exact pins, no semver drift, matches QA-REVIEW.md P0 pinning advice.
- `rust-toolchain.toml` pins to `1.95.0` per PR #3; the workspace `rust-version` MSRV is 1.79 (older floor, fine).
- Compiles cleanly under 1.95.0 on Windows: `cargo clippy -p omega-mock-ledger --all-targets -- -D warnings` is clean.

### L. No regressions

**Pass.**

- `cargo doc --workspace --no-deps --document-private-items` exits 0 (verified, full output ends with "Generated `C:\experiments\omega-commitment\target\doc\omega_claim_prover\index.html` and 10 other files").
- `cargo clippy -p omega-mock-ledger --all-targets -- -D warnings` exits 0 (verified).
- All 6 mock-ledger tests pass:
  - `apply_claim_inserts_nullifier_and_utxo_then_rejects_replay` ok in 25.45 s
  - `concurrent_readers_do_not_block_actor_writes` ok in 5.07 s
  - `heartbeat_ticker_keeps_moving_during_submit_storm` ok in 60.06 s
  - `restart_preserves_nullifiers_and_starstream_utxos` ok in 29.20 s
  - `schema_initializes_wal_pragmas_and_required_tables` ok in 0.05 s
  - `raft_storage_persists_vote_and_log_entries` ok in 0.08 s

The doc-retrofit from P1-4 is *new debt*, not a regression — `cargo doc` was clean before this PR (because the crate did not exist) and is still clean (because the crate does not yet enable `#![warn(missing_docs)]`).

---

## Tasks.md audit

Group 4, ticked vs. open:

| Box | Status | Code matches claim? |
|---|---|---|
| 4.1 deps + features | ticked | Yes. `Cargo.toml` declares all of `rusqlite "=0.32.1"` (bundled), `r2d2`, `r2d2_sqlite`, `openraft "=0.9.24"`, `tokio`, `omega-claim-tx`, `omega-claim-verifier`, plus `pallas-validate "=1.0.0-alpha.6"` behind `cardano-tx-validation` feature default-off. ✓ |
| 4.2 quote `validate_tx` signature | ticked | Yes. `src/cardano.rs:7-14` quotes the alpha.6 signature verbatim. ✓ |
| 4.3 schema + PRAGMAs | ticked | Yes. `src/schema.rs` + `tests/schema.rs` round-trip. mmap_size Windows-disabled. WITHOUT ROWID + composite PK + no FKs. ✓ |
| 4.4 writer-actor pattern | ticked | Yes. `src/writer.rs:22-26` dedicated thread, mpsc + oneshot, no per-call `spawn_blocking` on writes. Reader pool sized to `num_cpus::get()` at `src/lib.rs:101`. Module-level doc comment at `src/lib.rs:9-16` documents the rationale (though P1-4 flags incomplete crate docs). ✓ |
| 4.5 RaftStorage + RaftStateMachine | ticked | Yes. `src/storage.rs:97-340` impls `RaftLogReader`, `RaftStorage`, `RaftSnapshotBuilder`. Uses `openraft::storage::Adaptor` to bridge to the split-storage interfaces. Trait names + signatures verified against the pinned `openraft = "=0.9.24"` (compiles clean). ✓ |
| 4.6 apply pipeline | ticked | Yes. `src/writer.rs:277-311` parse → verify → nullifier-probe → insert-pair → commit. ✓ |
| 4.7 snapshot create + restore | **open** | Honest. Snapshot create via `VACUUM INTO` is implemented and tested (`tests/apply_smoke.rs:118-141`); restore uses `ATTACH + table-copy` instead of "drop live DB and rename" — the design's named approach was deferred per the Windows-reader-pool teardown concern. P1-3 flags the docstring debt. |
| 4.8 periodic WAL checkpoint | ticked | Yes. `src/lib.rs:189-205` defaults to 30 s, parameterisable. Routes through writer channel. ✓ |
| 4.9 apply_smoke | ticked | Yes. `tests/apply_smoke.rs` covers happy path + replay rejection + snapshot round-trip. ✓ |
| 4.10 concurrent_readers | ticked | Yes. `tests/concurrent_readers.rs` runs 16 readers + 1 actor writer for 5 s, asserts no `SQLITE_BUSY` and writer p99 < 50 ms. ✓ |
| 4.11 load_heartbeat (cluster) | **open** | Honest. Implemented as single-node heartbeat-continuity (storage side); cluster-side is group 6 debt. Documented at `tasks.md:67`. |
| 4.12 restart_durability (cluster) | **open** | Honest. Implemented as three-storage restart (storage side); cluster-side is group 6 debt. Documented at `tasks.md:67`. |
| 4.13 Cardano fixtures | **open** | Honest. Feature compiles; signature quoted; no real Conway fixtures yet. Documented at `tasks.md:69`. |

No tautological tickings. The open boxes have explicit rationale notes in `tasks.md:67-69`.

---

## Recommendation

**approve**.

Rationale for approval despite the four open boxes:

1. **4.7** is partially implemented with a documented design concession (Windows reader-pool teardown). The functional contract is satisfied (snapshot create + restore round-trips successfully in `tests/apply_smoke.rs`); only the specific "drop live DB and rename" mechanism is deferred. This is a Windows-specific cleanup pass, not a missing feature.
2. **4.11** and **4.12** are correctly scoped. The cluster-level versions of these tests can only land after `omega-toy-consensus` (group 6) wires real openraft + libp2p on top of this storage. The single-node versions in this PR exercise the actor-pattern under load and the storage-restart durability — exactly the storage-layer guarantees this crate owns.
3. **4.13** is feature-shell only. The signature is quoted, the feature compiles, the gate is honest. Real Conway fixtures are best landed alongside the `omega-experiment` CLI integration in group 7-8.

The P0 risks I was asked to specifically check (actor pattern, PRAGMAs, schema, verify-before-mutate, v2 wire format, replay defence) are all clean. P1 findings are documentation-and-error-typing improvements — none block merge, but P1-4 (doc compliance) MUST land before task 12.7 ticks green; the doc retrofit can be a follow-up PR scoped across the soundness-bearing crates collectively. P2 findings are nice-to-haves.

### Must-fix-before-12.7-ticks (not before merge)

- P1-4: crate-level docstring + per-public-item summaries + `# Soundness` blocks on `apply_claim`, `MockLedger::open`, `nullifier_exists`, the writer-actor explanation. Land alongside the same retrofit for `omega-claim-tx` / `omega-claim-prover` / `omega-claim-verifier` so task 12.7 ticks once for the whole soundness-bearing surface.
- Add `#![warn(missing_docs)]` and `#![warn(rustdoc::broken_intra_doc_links)]` to `src/lib.rs` once the retrofit lands; CI then catches regressions.

### Recommended follow-ups (low priority)

- P1-1: comment on `apply_raft_entries` documenting "rejected entries still advance `last_applied_log_id`".
- P1-3: comment on `restore_snapshot` documenting "post-install local log is wholly replaced; this is openraft semantics" and the Windows-reader-pool teardown trade-off.
- P2-1: make `payload_value` either `Result`-returning or document the silent-zero behaviour.
- P2-3: add `#[cfg(not(windows))] assert_eq!(pragmas.mmap_size, ...)` to `tests/schema.rs`.
- P2-5: switch tests to `tempfile::TempDir` for end-of-test cleanup.

### Open issues for the cluster integration (group 6)

- 4.11 cluster: real 3-node Raft submit-storm with leader-continuity assertion.
- 4.12 cluster: real 3-node restart with quorum re-election assertion.
- 4.13: hand-crafted Conway-era `MultiEraTx` accept/reject fixtures.
- 4.7 follow-up: revisit "drop live DB and rename" once the reader-pool teardown story is settled (likely tied to a node-shutdown handler in `omega-toy-consensus`).

The PR is ready to merge. The open boxes are honest debt, not missed work.
