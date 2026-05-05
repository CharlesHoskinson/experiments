---
date: 2026-05-05
kind: design-doc
topic: omega-toy-consensus Group 1 — keystone LoganNet binary + library
status: drafted (v1)
revisions:
  - v1: initial design — L3 scope (lib + binary + minimal JSON-RPC), 7c failure injection
---

# Design: `omega-toy-consensus` Group 1

## Why

LoganNet's three landed crates (`omega-claim-tx`, `omega-claim-verifier`, `omega-mock-ledger`) plus the just-merged `omega-network` give us every part except the conductor: the thing that brings up an openraft node, mounts the mock ledger as its state machine, plugs the libp2p transport into openraft's `RaftNetworkFactory`, and exposes a wire surface so external callers (tests, future Goblins, the planned `omega-tui`) can submit a claim and observe the result. Without this crate, `cargo run --bin omega-toy-consensus -- --node-id 1 --peers ...` does not exist and LoganNet has no end-to-end smoke test.

This spec defines `omega-toy-consensus` Group 1: a library + binary that boots a 3-node Raft cluster on real libp2p sockets, accepts a single claim over a minimal JSON-RPC surface, drives it through openraft to consensus, applies it via the mock-ledger writer actor, and returns the applied log index to the caller. Group 1 explicitly defers snapshot / leader-change / membership-change scenarios to Group 2, but covers them under failure injection (the test pack must prove the Group 1 surface degrades correctly when those events happen during a normal submit).

## Principles

1. **Conductor, not orchestra.** This crate owns wiring and lifecycles only. Consensus rules live in openraft. State-machine rules live in `omega-mock-ledger`. Verification lives in `omega-claim-verifier`. Transport lives in `omega-network`. Every line in `omega-toy-consensus` should be either (a) bringing up one of those four, (b) routing a request between them, or (c) exposing them to external callers via the JSON-RPC surface or the run-binary.
2. **Stateless wire surface.** The JSON-RPC surface does not maintain client sessions or proxy requests server-side. A non-leader server returns `−32000 NotLeader` with the leader's RPC URL in `data`. The client retries against the named URL. This matches Ethereum execution-API LB conventions and keeps each node's wire surface a pure function of its current state.
3. **Test pack first-class.** Every soundness-bearing seam in this crate gets a fixture in the rust-test pack. The orchestrator's G2 gate (STATUS=GREEN) must run before any "ready to commit" claim. The plan lists each test-pack skill (turmoil, failpoints, shuttle-loom, kani, proptest) as its own task with the exact invocation.
4. **Documentation gates equal test gates.** `cargo doc --workspace --no-deps --document-private-items` must finish clean, and every public item must satisfy `omega-rustdoc-style` (Errors / Soundness / Examples blocks where applicable). PR review blocks on docs the same way it blocks on tests.
5. **Localhost-only in Group 1.** RPC binds `127.0.0.1:8001/8002/8003` per node. No TLS, no auth, no rate limiting. Documented in the README and `--help` as a v0.1 limitation. Auth lands with `omega-api` (Goblins openspec change).

## Crate boundary

```
                ┌──────────────────────────────────────────────────────────┐
                │                      External callers                    │
                │   (tests, omega-tui, future omega-api → Goblins)         │
                └──────────────────────────┬───────────────────────────────┘
                                           │ JSON-RPC over HTTP
                                           ▼
                ┌──────────────────────────────────────────────────────────┐
                │                  omega-toy-consensus                     │
                │                                                          │
                │   ┌──────────┐   ┌────────────┐   ┌──────────────────┐   │
                │   │ rpc      │   │ Node       │   │ run binary       │   │
                │   │ jsonrpsee│◀─▶│ (lib root) │◀─▶│ CLI + bring-up   │   │
                │   └──────────┘   └─────┬──────┘   └──────────────────┘   │
                │                        │                                 │
                │            ┌───────────┼───────────┐                     │
                │            ▼           ▼           ▼                     │
                └────┬───────────┬───────────────┬───────────────┬─────────┘
                     │           │               │               │
                     ▼           ▼               ▼               ▼
              ┌─────────┐ ┌─────────────┐ ┌──────────────┐ ┌──────────────┐
              │openraft │ │omega-       │ │omega-network │ │omega-claim-  │
              │   0.9   │ │mock-ledger  │ │libp2p RR     │ │verifier      │
              └─────────┘ └─────────────┘ └──────────────┘ └──────────────┘
```

`omega-toy-consensus` does NOT:
- Reimplement Raft (openraft owns it).
- Reimplement state-machine apply (mock-ledger's `WriterHandle::apply_claim` owns it).
- Reimplement claim verification (verifier owns it; mock-ledger calls verifier; consensus crate must not duplicate).
- Define the libp2p transport (omega-network owns it via `LibP2pNetworkFactory`).

## File layout

```
crates/omega-toy-consensus/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs              crate-level docs + re-exports
│   ├── node.rs             Node struct + ::start() + lifecycle
│   ├── config.rs           NodeConfig + cluster topology + parsing
│   ├── rpc/
│   │   ├── mod.rs          OmegaRpc trait (jsonrpsee proc-macro)
│   │   ├── server.rs       OmegaRpcImpl struct + server bind + builder
│   │   ├── types.rs        SubmitOutcome, NodeState, NodeRole, error mapping
│   │   └── error.rs        ErrorObjectOwned construction; code constants
│   ├── routing.rs          NotLeader translation; openraft ForwardToLeader → JSON-RPC −32000
│   ├── bin/
│   │   └── omega-toy-consensus.rs   run binary; clap CLI
│   └── error.rs            ConsensusError enum
├── tests/
│   ├── single_leader_emerges.rs                turmoil 3-node, single leader
│   ├── single_claim_roundtrip.rs               turmoil 3-node, submit → applied
│   ├── leader_forwarding.rs                    submit to follower, get −32000 + retry succeeds
│   ├── partition_minority_no_commit.rs         turmoil partition; minority rejects
│   ├── failpoint_drop_appendentries.rs         failpoints, drop one append; eventually applies
│   ├── failpoint_byzantine_replay.rs           failpoints, replay vote; raft term safety
│   ├── shuttle_writer_handshake.rs             shuttle-loom on rpc → writer-handle channel
│   ├── leader_change_during_submit.rs          turmoil + failpoints, force re-election mid-submit
│   ├── snapshot_install_during_submit.rs       turmoil, trigger install, in-flight submit retries
│   ├── proptest_rpc_inputs.rs                  proptest CBOR/JSON adversarial inputs to submit
│   └── batch_limits.rs                         JSON-RPC batch cap (25 req / 1 MiB)
├── kani-proofs/
│   └── snapshot_install_state_machine.rs       kani bounded check, ≤4-state replay
├── benches/
│   └── bench_submit_p50.rs                     criterion, single-claim apply latency
└── examples/
    └── three_node_local.rs                     spin up 3 in-process nodes for ad-hoc dev
```

## Cargo.toml dependencies

Workspace-pinned (consume via `workspace = true`):

| Dep | Purpose |
|---|---|
| `openraft = "0.9"` | Already pinned — Raft core |
| `omega-mock-ledger` (path) | State machine + storage |
| `omega-network` (path) | Libp2p `RaftNetworkFactory` |
| `omega-claim-tx` (path) | Wire-format claim types |
| `omega-claim-verifier` (path) | Indirect (via mock-ledger), required for tests |
| `tokio = { version = "1", features = ["rt-multi-thread", "macros", "signal"] }` | Async runtime |
| `jsonrpsee = { version = "0.26", features = ["server", "macros", "client"] }` | JSON-RPC server + macro + test client |
| `schemars = "0.8"` | JSON Schema generation for RPC types |
| `serde = { version = "1", features = ["derive"] }` | |
| `serde_json = "1"` | |
| `clap = { version = "4", features = ["derive"] }` | Run-binary CLI |
| `anyhow = "1"` | Run-binary error reporting |
| `thiserror = "2"` | Library error types |
| `tracing = "0.1"` | Structured logs |
| `tracing-subscriber = "0.3"` | Run-binary log init |

dev-dependencies:

| Dep | Purpose |
|---|---|
| `turmoil = "0.7"` | Network simulation; 3-node fixtures |
| `fail = { version = "0.5", features = ["failpoints"] }` | Failure injection |
| `shuttle = "0.7"` | Concurrency-schedule exploration |
| `proptest = { workspace = true }` | Property tests |
| `tokio-test = "0.4"` | Async test helpers |
| `tempfile = { workspace = true }` | Per-test SQLite paths |
| `criterion = "0.5"` | Bench harness |

`[lints]` block: `workspace = true` (matches sibling crates).

## Public API — Rust library surface

```rust
// crates/omega-toy-consensus/src/lib.rs

pub use config::{NodeConfig, PeerConfig, RpcConfig};
pub use error::ConsensusError;
pub use node::{Node, NodeHandle};
pub use rpc::types::{NodeRole, NodeState, SubmitOutcome};

/// Boots a single Raft node, mounts the mock ledger, binds the JSON-RPC server,
/// returns a handle for graceful shutdown.
///
/// # Errors
/// - [`ConsensusError::Storage`] — SQLite open / schema init failed.
/// - [`ConsensusError::Network`] — libp2p bind / dial failed.
/// - [`ConsensusError::RpcBind`] — JSON-RPC HTTP bind failed.
/// - [`ConsensusError::Raft`] — openraft initialization rejected the membership.
///
/// # Soundness
/// Bring-up is idempotent on storage: the writer-actor lifecycle (see
/// `omega-mock-ledger`'s crate-level `# Soundness` block) is preserved.
/// Bring-up does NOT verify cluster identity — operators must ensure
/// `--cluster-id` matches across peers.
pub async fn start(config: NodeConfig) -> Result<NodeHandle, ConsensusError>;
```

`Node` and `NodeHandle` (in `node.rs`):

```rust
pub struct Node {
    raft: openraft::Raft<OmegaRaftTypeConfig>,
    ledger_writer: omega_mock_ledger::WriterHandle,
    network: omega_network::LibP2pNetwork,
    rpc_handle: jsonrpsee::server::ServerHandle,
    config: NodeConfig,
}

pub struct NodeHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    join: tokio::task::JoinHandle<Result<(), ConsensusError>>,
}

impl NodeHandle {
    /// Initiates graceful shutdown: stops accepting RPCs, drains in-flight
    /// submits, terminates the writer actor, releases the libp2p socket.
    pub async fn shutdown(self) -> Result<(), ConsensusError>;
}
```

Re-exports kept tight: external callers should depend on `omega-toy-consensus` via the `start()` entry point or `Node::start()`. They should NOT need to reach into `openraft::Raft` directly — that is an implementation detail.

## Public API — JSON-RPC wire surface

Two methods, namespaced `omega`:

```rust
// crates/omega-toy-consensus/src/rpc/mod.rs

#[rpc(server, namespace = "omega")]
pub trait OmegaRpc {
    /// Submit a single claim transaction. Returns the applied log index on
    /// success, or a structured error on rejection.
    ///
    /// JSON-RPC method name: `omega_submitClaim`.
    #[method(name = "submitClaim")]
    async fn submit_claim(
        &self,
        claim: ClaimTx,
    ) -> Result<SubmitOutcome, ErrorObjectOwned>;

    /// Read the node's current consensus and ledger state. Read-only.
    ///
    /// JSON-RPC method name: `omega_getState`.
    #[method(name = "getState")]
    async fn get_state(&self) -> Result<NodeState, ErrorObjectOwned>;
}
```

Wire types (in `rpc/types.rs`):

```rust
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SubmitOutcome {
    /// Whether the claim was applied to the state machine.
    pub accepted: bool,
    /// Raft log index at which the apply occurred, when `accepted`.
    pub applied_index: Option<u64>,
    /// Reject reason, when `!accepted`. One of: "verify", "invalid", "replay".
    pub reject_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NodeState {
    pub node_id: u64,
    pub role: NodeRole,
    pub leader_id: Option<u64>,
    pub last_log_id: Option<LogIdView>,
    pub applied_index: u64,
    pub nullifier_count: u64,
    pub starstream_utxo_count: u64,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy)]
pub enum NodeRole {
    Leader,
    Follower,
    Candidate,
    Learner,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LogIdView {
    pub term: u64,
    pub index: u64,
}
```

`ClaimTx` is re-used from `omega-claim-tx` directly. Serde derives there already give us the JSON wire shape; CBOR is reserved for the openraft RPC payload, JSON is the external surface.

`SubmitOutcome` uses string `reject_reason` rather than a tagged enum so client tooling without our types (curl / jq / Python) can read it without parsing JSON Schema.

## Error code map (JSON-RPC application range)

| Code | Symbol | When | `data` shape |
|---|---|---|---|
| −32700 | parse error | malformed JSON | (jsonrpsee built-in) |
| −32600 | invalid request | not JSON-RPC 2.0 | (jsonrpsee built-in) |
| −32601 | method not found | unknown method | (jsonrpsee built-in) |
| −32602 | invalid params | params do not match schema | (jsonrpsee built-in) |
| −32603 | internal error | unhandled panic / unexpected | (jsonrpsee built-in) |
| **−32000** | **NotLeader** | this node is not the leader | `{ leader_id: u64 \| null, leader_rpc_url: String \| null }` |
| **−32001** | **Verify** | proof verification failed | `{ verify_error: String }` (verifier error variant name) |
| **−32002** | **InvalidClaim** | CBOR decode / structural failure | `{ detail: String }` |
| **−32003** | **Replay** | nullifier already present | `{ sub_tree_id: u32, leaf_index: u64 }` |
| **−32004** | **WriterClosed** | writer actor unavailable (transient) | `{ retryable: true }` |
| **−32005** | **Timeout** | apply did not complete in deadline (default 5s) | `{ deadline_ms: u32 }` |

Symbol → code constants live in `rpc/error.rs` so the mapping is a single source of truth. `routing.rs` translates openraft `ClientWriteError::ForwardToLeader { leader_node, .. }` → `−32000`.

## Single-claim round-trip flow

```
                client
                  │
                  │ POST /  Content-Type: application/json
                  │ {"jsonrpc":"2.0","id":1,"method":"omega_submitClaim",
                  │  "params":{"claim": <ClaimTx JSON>}}
                  ▼
        ┌──────────────────────┐
        │ jsonrpsee HTTP server│  bound to 127.0.0.1:800N
        └──────────┬───────────┘
                   │
                   ▼
        ┌──────────────────────┐
        │ OmegaRpcImpl         │
        │ ::submit_claim       │
        └──────────┬───────────┘
                   │
                   │ raft.client_write(LedgerCommand::ApplyClaim { claim })
                   ▼
        ┌──────────────────────┐         not leader?
        │ openraft::Raft       │──Err(ForwardToLeader{leader})───▶ map to −32000
        └──────────┬───────────┘
                   │ leader
                   │ replicates AppendEntries via LibP2pNetwork
                   │ to followers; quorum-acks
                   ▼
        ┌──────────────────────┐
        │ apply_to_state_      │  delivered to MockLedgerStorage::apply
        │ machine              │  → forwards to WriterHandle::apply_claim
        └──────────┬───────────┘
                   │
                   ▼
        ┌──────────────────────┐
        │ writer actor:        │  parse CBOR → omega_claim_verifier::verify
        │ verify-before-mutate │      ↓ ok
        │                      │  open SQLite tx → check nullifier
        │                      │      ↓ absent
        │                      │  insert nullifier + Starstream UTxO
        │                      │  commit; reply LedgerResponse::accepted
        └──────────┬───────────┘
                   │
                   │ ClientWriteResponse{log_id, response}
                   ▼
        ┌──────────────────────┐
        │ map LedgerResponse   │  accepted=true,  applied_index=log_id.index
        │ → SubmitOutcome      │  accepted=false, reject_reason="verify"|...
        └──────────┬───────────┘
                   │
                   ▼
                client (200 OK + JSON-RPC result)
```

Leader forwarding stays client-side: a follower returns −32000 with the URL hint, the client retries against the leader. This is two HTTP round-trips in the worst case, one in the steady-state where the client knows which node is the leader.

Apply deadline is 5 seconds (configurable via `NodeConfig::apply_deadline`). Past that the call returns −32005 Timeout; the apply may still complete on the cluster (openraft does not roll back) — clients should poll `omega_getState` to confirm the index advanced.

## Configuration

`NodeConfig` (declarative; populated from CLI flags or `omega.toml`):

```rust
pub struct NodeConfig {
    pub node_id: u64,
    pub data_dir: PathBuf,                // SQLite WAL lives here
    pub libp2p_listen: Multiaddr,         // /ip4/127.0.0.1/tcp/4001 etc
    pub peers: Vec<PeerConfig>,           // 2 peers for a 3-node cluster
    pub rpc: RpcConfig,                   // bind addr, batch limits
    pub apply_deadline: Duration,         // default 5s
    pub cluster_id: String,               // matched across nodes
}

pub struct PeerConfig {
    pub node_id: u64,
    pub libp2p_addr: Multiaddr,           // dial address
    pub rpc_url: String,                  // for leader hints (http://127.0.0.1:8002)
}

pub struct RpcConfig {
    pub bind: SocketAddr,                 // 127.0.0.1:800N
    pub max_batch: u16,                   // default 25
    pub max_request_bytes: u32,           // default 1 MiB
}
```

CLI surface:

```
omega-toy-consensus run \
  --node-id 1 \
  --data-dir ./data/node1 \
  --listen /ip4/127.0.0.1/tcp/4001 \
  --peer 2,/ip4/127.0.0.1/tcp/4002,http://127.0.0.1:8002 \
  --peer 3,/ip4/127.0.0.1/tcp/4003,http://127.0.0.1:8003 \
  --rpc 127.0.0.1:8001 \
  --cluster-id loganet-dev
```

`--config <path>` overrides individual flags with a TOML file. CLI overrides win. No env-var support in Group 1 (deferred to `omega-tui`'s detection layer).

## Failure-injection scope (7c — maximal)

Each adversary lives in its own integration test under `tests/`. Listed in test-pack-skill order:

### turmoil (network simulation)

1. `single_leader_emerges` — 3 in-process nodes, 2s elapse, exactly one is leader.
2. `single_claim_roundtrip` — submit synthetic claim to leader, all 3 nodes report `applied_index=N`.
3. `partition_minority_no_commit` — partition node 1 from {2,3}, submit to node 1, returns `−32004 WriterClosed` or hangs past `apply_deadline`; cluster `{2,3}` continues to commit.
4. `partitioned_majority_continues` — same partition; submit to node 2 succeeds with `applied_index` advancing.
5. `leader_change_during_submit` — start a submit, force re-election (kill leader's network), confirm submit either:
   (a) returns `−32000 NotLeader` with new leader hint, or
   (b) returns `−32005 Timeout`, but `omega_getState` on the new leader eventually shows the claim applied.
   Both outcomes are valid; test asserts the disjunction.
6. `snapshot_install_during_submit` — trigger snapshot on follower mid-submit (compact log past `applied_index`), confirm the in-flight submit's response is consistent with the post-snapshot state.

### failpoints (IO injection)

7. `failpoint_drop_appendentries` — drop the first AppendEntries from leader to one follower; assert quorum still reached (other follower acks), claim applies, dropped follower catches up via subsequent retries.
8. `failpoint_byzantine_replay` — replay an old `VoteRequest` from a stale term; openraft must reject (term safety); assert no state mutation, no panic.
9. `failpoint_writer_closed_mid_submit` — inject `WriterHandle` channel close during `apply_to_state_machine`; assert the client gets `−32004 WriterClosed`, the cluster does not advance the applied-index past the failure (recovery on restart re-applies cleanly).

### shuttle-loom (concurrency exploration)

10. `shuttle_writer_handshake` — model the rpc-handler → writer-handle handoff under all schedules. Property: every submit either ends in `Ok(SubmitOutcome { accepted: true | false, .. })` or `Err(WriterClosed | Timeout)` — never a panic, never a silent loss. Bound: 100 schedules, 10 ops each.

### kani (bounded model checking)

11. `snapshot_install_state_machine` — Kani-bounded harness on the snapshot install machine: `pre_state ∈ {empty, populated, mid-restore} × snapshot ∈ {valid, malformed}`. Assert: post-state matches snapshot's claimed state when valid; rejects with named error when malformed. Bound `LOOP_BOUND=4`, `--default-unwind 5`. Tracked in `kani-bound.sh`.

### proptest (property tests)

12. `proptest_rpc_inputs` — fuzz JSON-RPC `params` for `submit_claim` against the shape of `ClaimTx`. Strategies: well-formed claim, claim with mutated proof byte, claim with truncated CBOR, claim with oversized leaf payload, claim with malformed nullifier. Property: server returns 200 OK with structured JSON-RPC error (one of −32001..−32003) — never 500, never panic.
13. `proptest_batch_limits` — generate batches of size 1..50 and total bytes 0..2 MiB. Property: batches ≤25 req AND ≤1 MiB succeed; batches over either cap return `−32600 invalid request` with a clear message; partial-batch errors are isolated per request.

### orchestrator (test-pack workflow)

14. The plan's final task gates "ready to commit" on `STATUS=GREEN` from the rust-test-orchestrator's report. T1 (trigger suite) runs, T2-F2/F3 fixtures (`omega-toy-consensus::raft_node` from rust-test-pack v0.1.0 release notes) get materialised here, F4 Adversary self-test still runs in CI.

## Test pack invocation map

This is the contract the Codex briefing enforces. Each row is one task in the plan, and Codex's PR cannot mark "ready to commit" until every row is GREEN.

| Test pack skill | Fixture / test file | Source skill ref |
|---|---|---|
| `rust-test-orchestrator` | drives all phases; emits final STATUS | `skills/local/rust-test-orchestrator/SKILL.md` |
| `rust-test-turmoil` | tests/single_leader_emerges.rs, tests/single_claim_roundtrip.rs, tests/partition_*, tests/leader_change_*, tests/snapshot_install_* | `skills/local/rust-test-turmoil/SKILL.md` |
| `rust-test-failpoints` | tests/failpoint_*.rs | `skills/local/rust-test-failpoints/SKILL.md` |
| `rust-test-shuttle-loom` | tests/shuttle_writer_handshake.rs | `skills/local/rust-test-shuttle-loom/SKILL.md` |
| `rust-test-kani` | kani-proofs/snapshot_install_state_machine.rs | `skills/local/rust-test-kani/SKILL.md` (S3 with `kani-bound.sh`) |
| `rust-test-proptest` | tests/proptest_rpc_inputs.rs, tests/proptest_batch_limits.rs | `skills/local/rust-test-proptest/SKILL.md` |

`rust-test-cargo-fuzz`, `rust-test-stateright`, `rust-test-madsim` not used in Group 1. (Stateright is reserved for Group 2's full leader-snapshot-membership state machine; cargo-fuzz waits until the wire surface stabilises; madsim is non-Tokio and we are Tokio-shaped.)

## Documentation gates

Every public item in `omega-toy-consensus` follows `skills/local/omega-rustdoc-style/SKILL.md`:

- Crate-level `//!` block: 5-element template (elevator pitch, design-context links, tier of trust, v0.1 limitations, conventions).
- Every `pub fn` returning `Result`: `# Errors` section enumerating each variant + condition.
- Every soundness-bearing public item: `# Soundness` block with preserves / closes / fails-on triple.
- Examples block on `start()` and `Node::submit_claim` (the helper used by tests).

`# Soundness` blocks required on:
- `omega_toy_consensus::start` (bring-up invariants)
- `Node::shutdown` (drain ordering)
- `OmegaRpcImpl::submit_claim` (calls `client_write`, does not bypass openraft)
- `routing::translate_client_write_error` (the −32000 leader-hint construction)

PR review blocks on:
- `cargo doc --workspace --no-deps --document-private-items` clean
- `cargo clippy --workspace --all-targets -- -D warnings` clean
- `cargo fmt --check` clean
- The four new `# Soundness` blocks present and following the SKILL's three-question structure
- All test-pack rows above GREEN per orchestrator report

## Acceptance gates

Concrete, mechanically checkable:

1. `cargo build -p omega-toy-consensus --bin omega-toy-consensus` succeeds on Windows + 1.95.0.
2. `cargo test -p omega-toy-consensus --no-fail-fast` — all 13 listed tests pass.
3. `cargo test -p omega-toy-consensus --features kani` runs the Kani harness clean (or `cargo kani` if the feature gate is replaced; final form decided in plan).
4. `cargo doc -p omega-toy-consensus --no-deps --document-private-items` — clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` — clean.
6. `cargo fmt --check` — clean.
7. The orchestrator report (final task) prints `STATUS: GREEN`.
8. Three nodes started via the `examples/three_node_local.rs` example reach quorum within 3s, and a `curl` POST to `http://127.0.0.1:8001/` with `omega_submitClaim` returns `{ accepted: true, applied_index: <n> }`. (Manual smoke test, recorded in PR description.)
9. PR description includes the orchestrator report + a trace of the manual smoke test.

## Out of scope (Group 2)

- Membership change (add / remove node) — Group 2.
- Multi-leader / leader handover diagnostics RPCs (`omega_clusterStatus`) — Group 2.
- WebSocket subscriptions (`omega_subscribeApplied`) — needed by Goblins, lands with `omega-api`.
- TLS / auth — lands with `omega-api`.
- Persistent peer discovery (mdns / kademlia bootstrap) — Group 2.
- The `omega-api` REST surface and OpenAPI generation — separate crate.
- The `omega-tui` dashboard — separate crate, separate spec.
- Goblins / agentic load — separate openspec change.
- Linux / macOS CI matrix — Codex stays on Windows + 1.95.0 for Group 1 to mirror PR-5 verification surface.

## Open question (deferred to plan)

- Does the kani harness gate behind a `cargo features = ["kani-proofs"]` flag, or behind an explicit `cargo kani` invocation in `kani-bound.sh`? The skill recommends the script form. Plan resolves at task-write time.

## Self-review

- **Placeholder scan:** no TBD / TODO / "fill in details". The "Open question" above is explicit and bounded.
- **Internal consistency:** crate-boundary diagram, file layout, dependency table, RPC surface, test invocation map, and acceptance gates all reference the same 13 tests + Kani proof + orchestrator report. The error-code map matches the routing.rs translation requirement. The failure-injection scope (7c) maps to the test pack invocation table row-for-row.
- **Scope check:** Group 1 is one PR. The 13 tests are a lot, but each is small (the turmoil 3-node fixture + claim helpers are reused across most of them). The Kani proof is one harness with bounded depth. JSON-RPC surface is two methods. CLI is six flags. This fits.
- **Ambiguity check:** apply-deadline 5s default, batch cap 25/1MiB, error codes −32000..−32005 — all fixed. Leader forwarding is client-side (one explicit sentence). Localhost-only is named in three places. No method names hand-wave; both wire names are pinned (`omega_submitClaim`, `omega_getState`).
