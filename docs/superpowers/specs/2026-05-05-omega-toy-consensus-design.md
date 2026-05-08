---
date: 2026-05-05
kind: design-doc
topic: omega-toy-consensus Group 1 — keystone LoganNet binary
status: drafted (v1)
implements: PR #7 on `feat/omega-toy-consensus-group1`
---

# Design: omega-toy-consensus Group 1

## Why

LoganNet needs a single Rust crate that brings up an openraft node, mounts
the `omega-mock-ledger` state machine, and exposes a JSON-RPC surface that
clients can hit to submit verified claims and inspect node state. Group 1
ships the keystone binary and the smallest possible JSON-RPC surface
required to wring out the apply pipeline end-to-end on a developer's
machine.

The crate is the **conductor**. It owns wiring and lifecycle only:
consensus rules stay in openraft, state-machine rules stay in
`omega_mock_ledger`, verification stays in `omega_claim_verifier`, and
transport stays in `omega_network`. Every line in the new crate brings
one of those four up, routes a request between them, or exposes them via
JSON-RPC or the `omega-toy-consensus run` binary.

## Scope (Group 1)

In:

- `omega-toy-consensus` library + `omega-toy-consensus run` binary +
  `examples/three_node_local`.
- Two JSON-RPC methods only: `omega_submitClaim`, `omega_getState`.
- Fixed JSON-RPC application error code range `-32000..-32005`
  (see "Error code map" below).
- Loopback bind enforcement at config-validation time.
- Static `--peer <id>,<libp2p_addr>,<rpc_url>` membership; no membership
  change RPC.
- In-process raft dispatcher (a deliberate v0.1 simplification — see
  "Transport" below).
- Test pack: turmoil, failpoints, proptest, and structural-placeholder
  Kani + Shuttle harnesses.

Out (deferred to Group 2 onwards; see
`cardano-wiki/wiki/pages/loganet-roadmap.md`):

- Real libp2p inbound raft RPC (multi-process cluster).
- Membership change.
- TLS, auth, rate limiting.
- WebSocket subscriptions.
- mDNS / Kademlia discovery.
- Dashboard / banner UI.
- Linux + macOS CI gates.
- Real Kani harness against `MockLedger::restore_snapshot`.
- Real Shuttle/Loom model of the writer-actor handshake.

## Architecture

```
┌────────────────────────────────────────────────────────────────────┐
│                  omega-toy-consensus run binary                     │
│                  ┌──────────────────────────────────┐               │
│                  │  jsonrpsee server (HTTP, loopback)│               │
│                  │  methods: omega_submitClaim,      │               │
│                  │           omega_getState          │               │
│                  └──────────────────────────────────┘               │
│                            │                                        │
│                            ▼                                        │
│                  ┌──────────────────────────────────┐               │
│                  │  rpc::server::OmegaRpcImpl       │               │
│                  │   - calls Raft::client_write     │               │
│                  │   - reads metrics + ledger       │               │
│                  │   - error mapping via routing.rs │               │
│                  └──────────────────────────────────┘               │
│                            │                                        │
│                            ▼                                        │
│                  ┌──────────────────────────────────┐               │
│  openraft 0.9 ──▶│  Raft<OmegaRaftTypeConfig>       │◀── network    │
│                  │  log + state-machine adapter via │   factory     │
│                  │  MockLedgerStorage               │   (libp2p     │
│                  └──────────────────────────────────┘    factory    │
│                            │                            in shape;   │
│                            ▼                            dispatcher  │
│                  ┌──────────────────────────────────┐   in-process  │
│                  │  omega-mock-ledger writer actor  │   for v0.1)   │
│                  │   - SQLite + verifier            │               │
│                  └──────────────────────────────────┘               │
└────────────────────────────────────────────────────────────────────┘
```

### Transport

The conductor wires `omega_network::LibP2pNetworkFactory` so openraft
sees a real `RaftNetworkFactory`, but for v0.1 every outbound
`RaftRpcRequest` is **routed through a process-global static
registry** (`node.rs::RAFT_REGISTRY`). The CBOR encode/decode is
preserved on each side so that the future inbound libp2p actor is a
one-file change at the dispatcher boundary.

Consequence: three independent `omega-toy-consensus run` processes
**cannot** form a cluster. Only `examples/three_node_local`, which
spawns three nodes inside one tokio runtime, does. The bin emits a
`tracing::warn!` at bring-up if `peers` is non-empty, naming this
limitation explicitly.

### Loopback bind enforcement

`Node::start` calls `validate_config(&config)` before opening any
sockets. If `config.rpc.bind.ip()` is not loopback, bring-up returns
`ConsensusError::Config(..)`. The intent is to fail loud rather than
silently expose an unauthenticated write-RPC to the local network
(no TLS, no auth, no rate limiting in v0.1).

### State-machine path

`OmegaRpcServer::submit_claim` is the only path from JSON-RPC to a
ledger mutation. It:

1. Calls `LedgerCommand::apply_claim(claim)` — this decodes the
   proof envelope and extracts the published commitment. A decode
   failure here returns the corresponding JSON-RPC error directly
   (no raft entry was committed).
2. Calls `Raft::client_write(cmd)` under
   `tokio::time::timeout(apply_deadline, _)`.
3. Inspects the resulting `LedgerResponse`. The `accepted: false`
   path returns a `SubmitOutcome` with `applied_index: Some(idx)`
   (the raft log index of the committed-but-rejected entry) and a
   `reject_reason` string.

`OmegaRpcServer::get_state` is read-only and never touches the writer
actor or `client_write`.

## Error code map (JSON-RPC application range)

This is the contract referenced from
`omega-toy-consensus/src/rpc/error.rs` and `routing.rs`. It is the same
table as
[`cardano-wiki/wiki/pages/loganet-roadmap.md` § Error code map](../../../cardano-wiki/wiki/pages/loganet-roadmap.md#error-code-map-json-rpc-application-range).

| Code   | Class            | Trigger                                                                     | Data shape                                                       |
|--------|------------------|-----------------------------------------------------------------------------|------------------------------------------------------------------|
| -32000 | `NotLeader`      | This node is not leader; openraft returned `ForwardToLeader`.               | `{leader_id, leader_rpc_url}` — either may be `null`.            |
| -32001 | `Verify`         | Proof verification failed before apply.                                     | `{verify_error}` — short string.                                  |
| -32002 | `InvalidClaim`   | Claim envelope could not be converted into a `LedgerCommand`.               | `{detail}` — short string.                                        |
| -32003 | `Replay`         | Nullifier already present in the ledger.                                    | `{sub_tree_id, leaf_index}` — coordinates of the existing leaf.   |
| -32004 | `WriterClosed`   | Writer-actor channel closed; transient.                                     | `{retryable: true}`.                                              |
| -32005 | `Timeout`        | `apply_deadline` elapsed before raft returned.                              | `{deadline_ms}`.                                                  |
| -32600 | `InvalidRequest` | Batch length exceeded `RpcConfig.max_batch` (default 25).                   | none.                                                             |
| -32602 | `InvalidParams`  | Per-method jsonrpsee parameter parsing failed.                              | none.                                                             |
| -32603 | `InternalError`  | Openraft `Fatal`, membership-change rejection (none expected in Group 1), or another internal failure. | `{openraft}` or `{ledger}` — error string. |
| -32700 | `ParseError`     | Body did not parse as JSON.                                                 | none.                                                             |

`SubmitOutcome { accepted, applied_index, reject_reason }` is
returned on **commit-time rejections** (verify / invalid / replay /
internal — the entry committed to the raft log but the state machine
rejected the apply, so `applied_index` is `Some(_)`). Pre-commit
failures (`-32000`, `-32004`, `-32005`, `-32603`) come back as
JSON-RPC call errors.

## Tier of trust

Soundness-bearing wiring. The crate does not verify proofs (the
verifier does) and does not apply state (the writer actor does), but
it is the component that ensures `Raft::client_write` is the only
path to apply, that a non-leader returns `-32000 NotLeader` rather
than silently proxying, and that the writer actor's lifecycle is
bounded by `Node::shutdown`.

## v0.1 limitations

- Localhost-only RPC (`127.0.0.1:800N`); no TLS, no auth, no rate
  limiting. Loopback bind is enforced by `validate_config`.
- Two RPC methods only: `omega_submitClaim`, `omega_getState`.
- HTTP only; WebSocket subscriptions land with `omega-api` (Goblins).
- No membership change; static `--peer` topology.
- No mDNS / Kademlia discovery.
- In-process raft dispatcher (see "Transport" above).
- Windows + 1.95.0 toolchain only; Linux/macOS CI deferred.
- Toy Kani + Shuttle harnesses (see
  `cardano-wiki/wiki/pages/loganet-roadmap.md` § "Toy verification
  harnesses").
