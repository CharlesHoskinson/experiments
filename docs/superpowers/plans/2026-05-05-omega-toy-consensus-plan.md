# omega-toy-consensus Group 1 Implementation Plan

> Companion to
> `docs/superpowers/specs/2026-05-05-omega-toy-consensus-design.md`.
> 28-task plan executed across `feat/omega-toy-consensus-group1` and
> `feat/omega-toy-consensus-group1-fix`. This file records the
> task list as shipped; for the design, read the spec.

**Goal:** Bring up `omega-toy-consensus` v0.1 — a single-binary,
3-node-via-example LoganNet harness with a 2-method JSON-RPC surface,
a fixed `-32000..-32005` error code map, and the test pack from
`cardano-wiki/wiki/pages/loganet-roadmap.md` § "Test pack".

**Architecture:** See the
[design doc](../specs/2026-05-05-omega-toy-consensus-design.md).

**Tech stack:** openraft 0.9 + jsonrpsee 0.26 + omega-network +
omega-mock-ledger; turmoil 0.7 + fail 0.5 + shuttle 0.7 + proptest
1.6 + Kani for the test pack.

---

## Crate scaffold (Tasks 1-4)

- [x] **Task 1:** Workspace member `crates/omega-toy-consensus`,
  package metadata, `[lints] workspace = true`, `forbid(unsafe_code)`,
  `warn(missing_docs)` + intra-doc-link warns.
- [x] **Task 2:** `[bin]` for the run binary; `[bin]` for the kani
  proof gated by `required-features = ["kani"]`.
- [x] **Task 3:** Workspace dep additions: `anyhow`, `fail`,
  `jsonrpsee`, `schemars`, `shuttle`, `tempfile`, `tracing`,
  `tracing-subscriber`, `turmoil`. `thiserror` bumped 1→2.
- [x] **Task 4:** Per-crate `[features]`: `failpoints`, `kani`,
  `turmoil-tests`, `shuttle-tests` (the last two are gating slots
  for future skill-driven runs).

## Config + error types (Tasks 5-7)

- [x] **Task 5:** `NodeConfig`, `PeerConfig`, `RpcConfig` with
  serde + `humantime_serde` for `apply_deadline`. `PeerConfig::FromStr`
  for the `--peer` CLI flag.
- [x] **Task 6:** `NodeConfig::single_node_localhost(node_id)` —
  for doctests and benches; rejects `node_id == 0`; constructs
  `bind` via `SocketAddr::from((Ipv4Addr::LOCALHOST, port))` with
  checked `u64 -> u16` port conversion.
- [x] **Task 7:** `ConsensusError` thiserror enum with `#[from]`
  for `LedgerError` and `OmegaNetworkError`; `Storage`, `Network`,
  `RpcBind { addr, source }`, `Raft`, `Config`, `ShutdownJoin`.

## Node lifecycle (Tasks 8-11)

- [x] **Task 8:** Process-global `RAFT_REGISTRY` + `RAFT_LINK_BLOCKS`
  statics; `register_raft` / `unregister_raft` / `route_raft`.
  `pub(crate)` test hooks `clear_raft_link_blocks_for_test` and
  `partition_raft_link_for_test`.
- [x] **Task 9:** `Node::start` — open ledger, build storage parts,
  create network factory, spawn dispatcher, build openraft, register,
  initialize cluster (lowest-id node), bind RPC server with batch-cap
  middleware, return `NodeHandle`. `validate_config` runs first and
  enforces loopback bind + warns on non-empty peers.
- [x] **Task 10:** `BatchLimit` jsonrpsee middleware that returns
  `-32600 InvalidRequest` when `batch.len() > max_batch`.
- [x] **Task 11:** `NodeHandle::shutdown` — stop server, unregister
  from raft dispatcher, abort network task, signal runtime task,
  shut down openraft.

## JSON-RPC surface (Tasks 12-16)

- [x] **Task 12:** `OmegaRpc` trait with `submit_claim` and
  `get_state`; `#[rpc(server, namespace = "omega")]`.
- [x] **Task 13:** `SubmitOutcome { accepted, applied_index,
  reject_reason }` and `NodeState { node_id, role, leader_id,
  last_log_id, applied_index, nullifier_count, starstream_utxo_count }`
  with schemars `JsonSchema` derives.
- [x] **Task 14:** `OmegaRpcImpl` + `OmegaRpcShared` (Arc-shared
  raft + ledger + peers + apply_deadline).
- [x] **Task 15:** `submit_claim`: `LedgerCommand::apply_claim` →
  `Raft::client_write` under `apply_deadline` → translate
  `ClientWriteError` and `LedgerError` via `routing.rs`. The
  `LedgerResponse::accepted = false` path returns `SubmitOutcome`
  with `applied_index = Some(idx)`.
- [x] **Task 16:** `get_state`: read metrics + ledger counters.
  `last_log_id` is sourced from `metrics.last_applied`'s typed
  `LogId` so `term` and `index` refer to the same entry.

## Error code map (Task 17)

- [x] **Task 17:** `rpc::error::CODE_*` constants and constructors
  (`not_leader`, `verify`, `invalid_claim`, `replay`,
  `writer_closed`, `timeout`). Each constructor carries a
  `# Soundness` block naming the invariant it preserves and the
  attack class it closes. See the spec's
  ["Error code map"](../specs/2026-05-05-omega-toy-consensus-design.md#error-code-map-json-rpc-application-range).

## Routing translators (Task 18)

- [x] **Task 18:** `translate_client_write_error` and
  `translate_ledger_error` in `routing.rs` — single source of truth
  for openraft → JSON-RPC and `LedgerError` → JSON-RPC. Each carries
  a `# Soundness` block.

## CLI + example (Tasks 19-20)

- [x] **Task 19:** `bin/omega-toy-consensus.rs` — clap subcommand
  `Run` with `--node_id`, `--data_dir`, `--listen`, `--peer`
  (repeatable), `--rpc`, `--cluster_id`, `--apply_deadline_secs`.
  Validates `node_id != 0` before bring-up. Tracing subscriber +
  Ctrl-C shutdown.
- [x] **Task 20:** `examples/three_node_local.rs` — three nodes in
  one tokio runtime, three peers each. Tempdirs per node.

## Mock-ledger glue (Tasks 21-22)

- [x] **Task 21:** `LedgerCommand::apply_claim(ClaimTx)` constructor
  in `omega-mock-ledger`; decodes proof envelope, extracts
  commitment.
- [x] **Task 22:** `LedgerResponse` + `LedgerReject` enum
  (Verify | InvalidClaim | Replay | WriterClosed | Internal).
  `LedgerResponse::rejected(LedgerError)` maps the error class to
  the right `LedgerReject` variant. Encoding for
  `last_applied_log_id` aligned to ciborium (was postcard; reader
  was already ciborium — silent bug fix).

## Test pack (Tasks 23-28)

- [x] **Task 23:** Turmoil tests — `single_leader_emerges`,
  `single_claim_roundtrip`, `leader_forwarding`,
  `partition_minority_does_not_commit`,
  `majority_with_http_partition_continues_to_commit` (renamed from
  `partitioned_majority_continues_to_commit` to acknowledge that
  the test only partitions HTTP, not raft RPC, since raft is
  in-process — see the loganet-roadmap "Group 3" entry for the real
  partition test).
- [x] **Task 24:** Failpoint tests — `drop_first_appendentries…`,
  `term_does_not_regress_under_vote_replay_failpoint` (renamed
  from `replayed_old_vote_does_not_regress_term` to acknowledge
  that the test only checks term monotonicity), `writer_closed_…`.
- [x] **Task 25:** Snapshot test —
  `submit_then_wait_then_submit_converges` (renamed from
  `snapshot_install_mid_submit_keeps_state_consistent` to
  acknowledge that v0.1 has no public snapshot trigger; see Group 2).
- [x] **Task 26:** Proptest input fuzzer — 32 iterations,
  `documented_code` allowlist includes `-32603 InternalError`.
- [x] **Task 27:** Kani — toy state-machine harness shipped as a
  structural placeholder; called out as such in the test source and
  in the loganet-roadmap.
- [x] **Task 28:** Shuttle — generic mpsc handshake model shipped
  as a structural placeholder; called out as such.

## Verification

- `cargo build -p omega-toy-consensus --bin omega-toy-consensus` —
  pinned to 1.95.0 via `rust-toolchain.toml`.
- `cargo test -p omega-toy-consensus --no-fail-fast`.
- `cargo test -p omega-toy-consensus --features failpoints --no-fail-fast`.
- `cargo doc -p omega-toy-consensus --no-deps --document-private-items`.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo fmt --check`.
- WSL `bash skills/local/rust-test-kani/scripts/kani-bound.sh
  omega-toy-consensus` — runs the toy harness; reports
  `VERIFICATION:- SUCCESSFUL` on the toy property.
