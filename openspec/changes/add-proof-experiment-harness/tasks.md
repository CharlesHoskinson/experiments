# Tasks

Implementation order. Each task is gated on the previous one passing `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`. The harness builds inside the existing `omega-commitment/` workspace as new sibling crates.

## 1. omega-claim-tx

- [x] 1.1 Create `crates/omega-claim-tx/` with `Cargo.toml` declaring `serde`, `minicbor`, `thiserror`, `omega-commitment-core` (path dep) workspace deps.
- [x] 1.2 Define `ClaimTx`, `ClaimUtxo`, `ClaimCollection`, `ClaimPublicInputs`, `ClaimWitness`, `ProofBytes` types with `#[derive(Serialize, Deserialize)]`.
- [x] 1.3 Implement CBOR encode + decode via `minicbor` (parity with omega-commitment-ingest).
- [x] 1.4 Add `proptest` round-trip tests: encode then decode equals original for both variants.
- [x] 1.5 Add `cbor_size_within_bounds` test asserting a 1024-leaf `ClaimCollection` encodes within 32 MiB.
- [x] 1.6 Document the wire format in `crates/omega-claim-tx/README.md` with byte layout.

## 2. omega-claim-prover

- [x] 2.1 Pin Plonky3 in workspace `[workspace.dependencies]` once: `plonky3 = { git = "https://github.com/Plonky3/Plonky3", rev = "<40-char-hash captured at first build>" }`. Every harness crate consumes via `workspace = true` (single source of truth — no per-crate independent rev).
- [x] 2.2 Create `crates/omega-claim-prover/` with deps on `p3-uni-stark`, `p3-baby-bear`, `p3-poseidon2`, `p3-poseidon2-air`, `p3-blake3-air`, `p3-merkle-tree`, `p3-symmetric`, `p3-fri`, `p3-challenger`, `p3-commit`, `p3-field`, `p3-matrix`, `p3-dft` — all via `workspace = true`.
- [x] 2.3 Define `OmegaMembershipAir` implementing `p3_air::Air<AB>`. Trace columns: sub_tree_id, leaf_index_be, payload buffer (capped so the full v1 leaf preimage is ≤ 64 bytes), intermediate Blake3 compression state, sibling-hash buffer, current-node hash. One row per Merkle path step.
- [ ] 2.4 Wire the AIR up to `p3-blake3-air` via permutation argument so leaf-hash and node-hash *compressions* are discharged by the upstream Blake3 gadget. Add an explicit comment that this constrains the compression function only; preimage gluing for ≤ 64-byte leaves is checked deterministically by the verifier (no separate AIR needed because the preimage fits in one compression block).
- Implementation note 2026-05-03: the current batch emits a separate `p3-blake3-air` compression proof inside the proof envelope and keeps deterministic preimage/path gluing in the verifier boundary. The task remains open until the membership AIR and Blake3 compression rows are joined by an actual Plonky3 permutation argument.

#### Soundness sub-tasks (PR #2 review, P0 — option 1a)

The PR-2 review (`PR-2-REVIEW.md`) found the membership AIR is content-free: it constrains only `IS_REAL_STEP`-bool, first-row `sub_tree_id`, an accumulator transition `next.acc = local.acc * 257 + next.checksum`, and last-row `acc == public[1]`. The columns holding `LEAF_INDEX_BE`, `PAYLOAD`, `BLAKE3_STATE`, `SIBLING`, `CURRENT_NODE` are written by `fill_real_row` but never constrained to equal Blake3 of any preimage, never tied to `node_hash_v2`, never forced to terminate at any sub-tree root. A malicious prover skipping `validate_witnesses` and filling those columns with arbitrary bytes produces an accepting proof for any `(sub_tree_id, leaf_index, nullifier, recipient)`. Closing this is the v0.1 soundness gate.

Option 1a — make the AIR actually prove what `OmegaMembershipAir` claims — break into:

- [ ] 2.4.1 In `omega-claim-prover/src/lib.rs`, replace the `acc * 257 + checksum` accumulator with real per-step constraints: each `IS_REAL_STEP` row asserts `current_node = node_hash_v2(left, right)` where `(left, right)` come from `(prev_current_node, sibling)` swapped by a `BIT` column representing the path direction (least-significant bit of `leaf_index_be` shifted right by step index). The first real row asserts `current_node = leaf_hash_v2(sub_tree_id, leaf_index_be, payload)`.
- [ ] 2.4.2 Add a `path_step_index` column. Constrain it to start at 0 in the first real row, increment by 1 each step, and end at `tree_depth - 1` (where `tree_depth` becomes a public input bound from the call-site sub-tree's `item_count`). Assert the last real row's `current_node` equals `public[2]` = the per-sub-tree root pinned in genesis.
- [ ] 2.4.3 Bind `node_hash_v2` and `leaf_hash_v2` to the embedded `p3-blake3-air` compression rows via a Plonky3 permutation argument (one cross-table lookup per hash invocation). The compression-input bytes go into the lookup table; the compression output goes back. This is the gluing step the v3.1 design names but the v0.1 implementation skipped.
- [ ] 2.4.4 Extend `ClaimPublicInputs` with `tree_depth: u8` and `per_sub_tree_root: [u8; 32]`. Update `omega-claim-tx` accordingly, regenerate CBOR golden tests in `omega-claim-tx/tests/`. Update the verifier to surface these as `VerifyError::WrongSubTreeRoot` on mismatch and `VerifyError::DepthMismatch` on tree-depth mismatch.
- [ ] 2.4.5 Add `tests/soundness_negative.rs` to `omega-claim-prover/`: build a 256-leaf sub-tree, prove inclusion of leaf 42, then mutate `(payload, sibling, current_node, leaf_index_be)` columns one at a time in the trace and confirm the verifier rejects each mutation. (The current "tampered proof byte" test only catches Plonky3-layer tampering; this catches AIR-layer tampering.)
- [ ] 2.4.6 Document in the prover's `lib.rs` module header that v0.1 caps leaf preimages at 64 bytes (one Blake3 compression block); v0.2 lands `LeafPreimageAir` for variable-length leaves.
- [ ] 2.4.7 Run the existing `tests/prover_smoke.rs` + `omega-claim-verifier/tests/verifier_roundtrip.rs` against the constrained AIR; both must still pass.
- [ ] 2.4.8 Re-tick task 3.4 once 2.4.1–2.4.7 land — the verifier's "rejects tampered proof" scenario then carries real soundness, not just envelope-binding hygiene.
- [x] 2.5 Implement `prove_collection(commitment, witnesses, config) -> Result<ProofBytes, ProverError>` using `p3_uni_stark::prove`. Reject witnesses with leaf payloads that would push the preimage past 64 bytes with `ProverError::LeafTooLargeForV01`.
- [x] 2.6 Match the prover config to `var/upstream/Plonky3/examples/examples/prove_prime_field_31.rs` (BabyBear, Poseidon2 Merkle, recursive-DFT, default FRI).
- [x] 2.7 Add `tests/prover_smoke.rs`: build a 256-leaf synthetic UTxO sub-tree with `omega-commitment-core::Tree::build_v1`, prove inclusion of leaf 42, assert proof bytes non-empty.
- [ ] 2.8 Add `bench_prove_p50.rs` criterion bench measuring 1, 16, 256-leaf prove latency. Capture results into a markdown report; only after measurement is the spec.md SLA finalized.

## 3. omega-claim-verifier

- [x] 3.1 Create `crates/omega-claim-verifier/` with the same Plonky3 deps as the prover.
- [x] 3.2 Implement `verify(commitment, public_inputs, proof) -> Result<(), VerifyError>` using `p3_uni_stark::verify` with the same AIR.
- [x] 3.3 No tokio, no async, no I/O in the public surface.
- [x] 3.4 Add `tests/verifier_round_trip.rs`: prove → verify accepts; tampered proof byte → `Err(InvalidProof)`; wrong commitment → `Err(CommitmentMismatch)`.
- Implementation note 2026-05-03: verifier task 3.4 also covers a proof-envelope binding regression where public inputs are rewritten to match call arguments while the Plonky3 proof is left unchanged; this now returns `Err(InvalidProof)` because `(commitment, public_inputs)` is bound into the proof public values.
- [ ] 3.5 Add `bench_verify_p50.rs` measuring verify latency at 1, 16, 256 leaves; assert verify p50 < 500 ms on the developer laptop.
- Implementation note 2026-05-03: `bench_verify_p50.rs` exists and compiles with `cargo bench -p omega-claim-verifier --bench bench_verify_p50 --no-run`; the task remains open until the benchmark is run and the p50 assertion is backed by local measurements.

## 4. omega-mock-ledger (SQLite + state machine)

- [ ] 4.1 Create `crates/omega-mock-ledger/` with deps `rusqlite = "0.32"` (`bundled`), `r2d2 = "0.8"`, `r2d2_sqlite = "0.25"`, `openraft = "0.9"`, `tokio` (already in workspace), `omega-claim-tx`, `omega-claim-verifier`, plus `pallas-validate = "=1.0.0-alpha.6"` behind `cardano-tx-validation` Cargo feature (default off).
- [ ] 4.2 Before writing any test code that calls `pallas_validate::phase1::validate_tx`, read the function signature in `var/upstream/pallas/pallas-validate/src/phase1/mod.rs` at the pinned version and quote the actual signature into a doc comment in `omega-mock-ledger/src/cardano.rs`. If the API does not match the proposal's stated shape, scope the feature down or punt to a follow-up change.
- [ ] 4.3 Implement schema initialisation in `src/schema.rs`: open connection, set all PRAGMAs (`journal_mode=WAL`, `synchronous=NORMAL`, `cache_size=-65536`, `mmap_size=268435456` (Linux/macOS only — disabled on Windows), `temp_store=MEMORY`, `wal_autocheckpoint=10000`, `auto_vacuum=NONE`), create tables with `WITHOUT ROWID`, primary keys, no FKs.
- [ ] 4.4 Implement the **writer-actor pattern**: a dedicated OS thread (`std::thread::spawn`, NOT a tokio task) owns the rusqlite write `Connection`, loops on an `mpsc::UnboundedReceiver<WriteCmd>`, and replies via `oneshot::Sender`. The openraft state machine sends `WriteCmd`s and `await`s the oneshot reply. Reader queries borrow short-lived `Connection`s from an `r2d2_sqlite::SqliteConnectionManager` pool sized to `num_cpus::get()`. Reader-pool calls run in `tokio::task::spawn_blocking`. Document the rationale in a module-level doc comment.
- [ ] 4.5 Implement `MockLedgerStorage` matching openraft 0.9's `RaftStorage` + `RaftStateMachine` traits (verify trait names against the pinned version's `cargo doc` before claiming the impl). Persist Raft log + meta to `raft_log` / `raft_meta` tables.
- [ ] 4.6 Implement the apply pipeline (parse → verify → nullifier check → insert nullifier → emit Starstream UTxO → commit transaction).
- [ ] 4.7 Implement snapshot via `VACUUM INTO 'snapshot-<idx>.sqlite'`. The snapshot command flows through the same `mpsc` channel as writes, so it serializes cleanly against ordinary writes (no separate mutex needed). Restore drops live DB and renames the snapshot file.
- [ ] 4.8 Add a periodic `PRAGMA wal_checkpoint(TRUNCATE)` task that fires every 30 seconds from a low-priority tokio task; this caps WAL growth between automatic checkpoints under sustained load.
- [ ] 4.9 Add `tests/apply_smoke.rs`: open ledger, apply a hand-crafted accepted claim, assert nullifier row + Starstream UTxO row exist; replay rejects.
- [ ] 4.10 Add `tests/concurrent_readers.rs`: 16 reader tasks + 1 writer actor for 5 seconds; assert no `SQLITE_BUSY` and writer p99 < 50 ms.
- [ ] 4.11 Add `tests/load_heartbeat.rs`: 60-second submit storm against a 3-node cluster; assert no leader change occurred (validates the actor pattern under load — this is the test that catches "spawn_blocking thundering herd" if the actor pattern is botched).
- [ ] 4.12 Add `tests/restart_durability.rs`: submit a claim, drop all 3 nodes, recreate from the same SQLite files, assert all nullifiers + Starstream UTxOs persist and quorum re-elects.
- [ ] 4.13 Behind `--features cardano-tx-validation`: `tests/cardano_phase1.rs` exercises a hand-crafted Conway-era `MultiEraTx` through `pallas_validate::phase1::validate_tx` and asserts both accept and reject paths. Document which `pallas-traverse` `Era` variant the harness decodes by default.

## 5. omega-network (libp2p)

- [ ] 5.1 Create `crates/omega-network/` with `libp2p = "0.55"` (default-features off; opt-in: `tcp`, `noise`, `yamux`, `mdns`, `kad`, `request-response`). Gossipsub is intentionally NOT a feature in v0.1.
- [ ] 5.2 Implement `LibP2pNetwork` and `LibP2pNetworkFactory` matching openraft 0.9's network traits (verify trait names + signatures against the pinned `cargo doc` before claiming the impl). Carry Raft RPCs as CBOR-encoded `request_response` payloads.
- [ ] 5.3 Implement snapshot chunking with the explicit wire protocol from `design.md`: `SnapshotInit { snapshot_id, total_chunks, total_bytes, sha3_of_full }` → N × `SnapshotChunk { snapshot_id, chunk_idx, payload }` (≤ 1 MiB each, **serial transfer, single in-flight, in chunk-index order**) → `SnapshotFinalize { snapshot_id, sha3_of_full }`. Receiver writes each chunk to disk synchronously before acking, verifies `sha3_of_full` from `SnapshotFinalize` before swapping the file into place.
- [ ] 5.4 Implement snapshot abort-on-leader-change: any leader-change event during a snapshot session aborts the session on the receiver (partial chunks discarded). New leader restarts from chunk 0.
- [ ] 5.5 Use a non-default mDNS service name with a per-installation salt (e.g., `_omega-experiment-<sha3-prefix-of-genesis>._udp.local`) to avoid LAN flooding when a developer runs the harness on a shared network. Provide a `--mdns-disabled` CLI flag for further isolation.
- [ ] 5.6 Add `tests/three_node_memory_transport.rs`: 3 in-process nodes via `libp2p::core::transport::MemoryTransport`; assert leader elected within 5 s; submit claim; assert all 3 ledgers contain the nullifier.
- [ ] 5.7 Add `tests/three_node_tcp_loopback.rs` (gated on `RUN_TCP_HARNESS=1` env var, off by default): 3 nodes on real TCP loopback `127.0.0.1:{4001,4002,4003}` with Noise + Yamux + mDNS active; same assertions plus a leader-change scenario. CI runs this on a Linux runner; macOS/Windows runs are best-effort.
- [ ] 5.8 Add `tests/snapshot_leader_change.rs`: induce an `InstallSnapshot` on a lagging follower, kill the leader between chunks, assert the new leader restarts the snapshot from chunk 0 and the follower eventually catches up.
- [ ] 5.9 Add a `--peers` CLI flag fallback for environments without mDNS multicast (CI runners).

## 6. omega-toy-consensus

- [ ] 6.1 Create `crates/omega-toy-consensus/` wiring `omega-mock-ledger` + `omega-network` into `openraft::Raft`.
- [ ] 6.2 Implement `ToyConsensusNode::start(config)` that bootstraps Raft with hard-coded membership for nodes 1/2/3.
- [ ] 6.3 Expose `submit(claim_tx)` that calls `raft.client_write` and returns the apply result.
- [ ] 6.4 Implement optional `--tap-cardano <network>` flag that subscribes to the named Cardano network's chain-sync miniprotocol via `pallas-network` and uses slot ticks as a heartbeat *hint*. Fall back to wall-clock heartbeats after 5 s of slot-stream silence so preview wobble does not trigger election storms. Handle `MsgRollback` by resetting the slot reference, not by re-electing. Set `heartbeat_interval = 250ms`, `election_timeout_min = 1s`, `election_timeout_max = 2s` explicitly in `RaftConfig`.
- [ ] 6.5 Implement a JSON-RPC submit endpoint on TCP for the CLI to talk to.
- [ ] 6.6 Add `tests/election.rs`: 3 nodes, kill the leader after first commit, assert a new leader is elected and the second commit succeeds.

## 7. omega-experiment CLI

- [ ] 7.1 Create `crates/omega-experiment/` with `clap` deps; depends on every other harness crate.
- [ ] 7.2 Implement `prove` subcommand: read commitment + leaf set, run prover, write proof to disk.
- [ ] 7.3 Implement `submit` subcommand: connect to a node's JSON-RPC endpoint, submit proof, wait for apply, print result.
- [ ] 7.4 Implement `state` subcommand: query node, print nullifier count + Starstream UTxO list.
- [ ] 7.5 Implement `bench` subcommand: run N rounds of prove+submit, report p50/p95/p99 in JSON via `--json`.
- [ ] 7.6 Add `tests/cli_smoke.rs` exercising each subcommand against the in-process 3-node test harness.

## 8. End-to-end integration test

- [ ] 8.1 Add `crates/omega-experiment/tests/e2e.rs` (or top-level `tests/e2e.rs`) that:
  - 8.1.1 Spins up 3 in-process `ToyConsensusNode`s on `MemoryTransport`.
  - 8.1.2 Generates a 256-leaf synthetic UTxO sub-tree and pins its bundle root as genesis.
  - 8.1.3 Picks a random leaf, builds witness, runs prover, submits.
  - 8.1.4 Asserts all 3 nodes' SQLite databases contain exactly one new nullifier and one new Starstream UTxO.
  - 8.1.5 Replays the same submission and asserts it is rejected with a nullifier-collision error.
  - 8.1.6 Tampers a proof byte and asserts the verifier rejects with `InvalidProof`.

## 9. README "Run a proof experiment" section

- [ ] 9.1 Add a top-level section to `README.md` titled "Run a proof experiment" placed after the "What the Plonky3 verifier proves" section.
- [ ] 9.2 Provide ≤ 8 copy-paste shell commands covering: build the workspace, generate a synthetic genesis fixture, start the 3-node quorum, run `prove`, run `submit`, run `state`, optional `bench`.
- [ ] 9.3 Document the optional `cardano-tx-validation` feature flag and how to enable the `--tap-cardano preview` mode.
- [ ] 9.4 Cross-link to `cardano-wiki/wiki/pages/omega-proof-experiment-harness.md` for the deep dive.

## 10. Wiki page + log entry

- [ ] 10.1 Write `cardano-wiki/wiki/pages/omega-proof-experiment-harness.md` summarising the harness design with provenance back to this OpenSpec change.
- [ ] 10.2 Update `cardano-wiki/wiki/index.md` adding a `[[omega-proof-experiment-harness]]` entry under the existing "Testnet Demo (T1 + T6 prototype)" category.
- [ ] 10.3 Append a `cardano-wiki/wiki/log.md` entry tagged `plan | proof-experiment-harness` with bundle-root reference and crate inventory.

## 11. CI

- [ ] 11.1 Update `.github/workflows/ci.yml` to install the Plonky3 git deps via Cargo's standard mechanism and run `cargo test --workspace --no-fail-fast`.
- [ ] 11.2 Skip the `--tap-cardano` integration test in CI (gated behind a `RUN_CARDANO_TAP=1` env var, off by default).
- [ ] 11.3 Add a separate job that builds with `--features cardano-tx-validation` to ensure the optional feature compiles.

## 12. Validation gate

- [ ] 12.1 `cargo test --workspace --no-fail-fast` green.
- [ ] 12.2 `cargo fmt --check` clean.
- [ ] 12.3 `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] 12.4 `openspec validate add-proof-experiment-harness --strict` green.
- [ ] 12.5 `omega-experiment bench --leaves 256 --samples 1000` reports measured `prove_p50_ms`, `prove_p95_ms`, `submit_p50_ms`, `submit_p95_ms`. Numbers are recorded into a markdown report and used to **finalize** the spec.md performance scenarios; the targets in spec.md are first-pass and may be lowered after measurement. p99 is reported but not gated (100-sample p99 is statistically meaningless; 1000+ samples make it informative).
- [ ] 12.6 QA review (`QA-REVIEW.md`) findings addressed or explicitly deferred with rationale captured in this task list.
