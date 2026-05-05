## PR #4 review — verdict: **approve**

Full review at `openspec/changes/add-proof-experiment-harness/PR-4-REVIEW.md` (committed alongside this comment).

### Top-line

The single highest-stakes contract — actor-pattern writer (NOT per-call `spawn_blocking`) — is implemented correctly. `src/writer.rs:22-26` does `std::thread::Builder::new().name("omega-mock-ledger-writer").spawn(...)`; the writer thread owns the `rusqlite::Connection`, loops on `rx.blocking_recv()`, and replies via `oneshot`. No per-call `spawn_blocking` on the writer hot path. Snapshots flow through the same channel as writes — no separate mutex needed.

### Spec checklist (A–L)

- **A. Actor pattern**: pass. Dedicated OS thread, no spawn_blocking on writes. `src/writer.rs:22-26`, `src/writer.rs:210-275`, `src/storage.rs:274-290`.
- **B. PRAGMAs**: pass. All seven set. `mmap_size` correctly Windows-disabled (`src/schema.rs:35-36`). Round-trip-asserted by `tests/schema.rs:21-26`.
- **C. Schema**: pass. All five tables `WITHOUT ROWID`, `nullifiers` PK is `(sub_tree_id, leaf_index)`, no FKs. `src/schema.rs:42-77`.
- **D. Verify-before-mutate**: pass. `src/writer.rs:284-310` — parse → verify → tx.open → replay-probe → insert-pair → commit.
- **E. v2 wire format**: pass. `ClaimPublicInputs` carries `tree_depth` + `per_sub_tree_root`; tests construct with both fields.
- **F. Replay defence**: pass. `SELECT 1` probe inside transaction, typed `LedgerError::Replay`. `src/writer.rs:339-355`.
- **G. `#[doc(hidden)]`**: pass. `src/lib.rs:215-225`.
- **H. Doc compliance**: **partial pass — P1 finding**. Crate-level doc covers the actor rationale but lacks design-context links, tier-of-trust, v0.1 limitations. Public items lack one-line summaries / `# Errors` / `# Soundness` blocks. Must land before task 12.7 ticks; can be a follow-up retrofit alongside the same gaps in `omega-claim-tx`/`prover`/`verifier`.
- **I. Test rigor**: open boxes are honest debt. 4.11 + 4.12 are scoped to single-node storage; cluster-level versions belong to group 6 (omega-toy-consensus). 4.7 snapshot create+restore is implemented and round-trip-tested; only the "drop live DB and rename" mechanism is deferred per a documented Windows reader-pool teardown concern. 4.13 is feature-shell only.
- **J. Cardano feature**: pass. Signature quoted at `src/cardano.rs:7-14`; feature compiles; default off; no real Conway fixtures (4.13 honest).
- **K. Workspace hygiene**: pass. All `=` exact pins; compiles clean under Rust 1.95.0 toolchain.
- **L. No regressions**: pass. `cargo doc --workspace --no-deps --document-private-items` clean; `cargo clippy -p omega-mock-ledger --all-targets -- -D warnings` clean; all 6 mock-ledger tests pass locally (apply_smoke 25.45s, concurrent_readers 5.07s, load_heartbeat 60.06s, restart_durability 29.20s, schema 0.05s, storage 0.08s).

### Findings

**P0**: none.

**P1** (must address before tick 12.7, not before merge):
- P1-1: comment that `apply_raft_entries` advances `last_applied_log_id` even on rejected entries (correct semantics, non-obvious from code).
- P1-2: comment on `last_membership` placement vs `Membership` mid-batch (correct, fragile).
- P1-3: comment on `restore_snapshot`'s table-copy approach + post-install log-replacement semantics + reader-pool concurrency trade-off.
- P1-4: doc retrofit per `omega-rustdoc-style` SKILL — crate-level docstring (design-context links, tier-of-trust, v0.1 limitations) plus per-public-item `# Soundness` / `# Errors` blocks. Bundle with the same retrofit on `omega-claim-tx`/`prover`/`verifier`.
- P1-5: `# Soundness` block on `apply_claim`.
- P1-6: docstring on `nullifier_exists` flagging the pre-commit-write-not-visible-to-readers semantics for future CLI consumers.

**P2** (nice-to-have follow-ups):
- P2-1: `payload_value` returns 0 for short payloads silently — make `Result` or document.
- P2-3: positive Linux assertion on `pragmas.mmap_size` in `tests/schema.rs`.
- P2-4: typed `LedgerError::Snapshot { ... }` variant.
- P2-5: switch tests to `tempfile::TempDir`.
- P2-6: snapshot-during-readers / partial-snapshot-failure tests.

### Tasks.md audit

Every ticked box (4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.8, 4.9, 4.10) has matching code. The four open boxes (4.7, 4.11, 4.12, 4.13) have honest rationale notes at `tasks.md:67-69`. No tautological tickings.

### Recommendation

**approve** — ready to merge.

The open boxes are honest debt:

- 4.11 + 4.12 cluster-level: blocked on group 6 (`omega-toy-consensus`).
- 4.7 drop-and-rename: Windows-specific cleanup, current `ATTACH + table-copy` works and is round-trip tested.
- 4.13: real Conway fixtures land alongside group 7 CLI integration.

P1-4 (doc retrofit) is the only thing standing between this PR and task 12.7 — recommend a follow-up PR that retrofits docs across all four soundness-bearing crates in this change in one pass.

Posted via `gh pr comment` (not `--approve` — GitHub blocks self-approval per prior convention).
