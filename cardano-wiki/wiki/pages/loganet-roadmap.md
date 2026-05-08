# LoganNet Roadmap

> **Status:** Group 1 in `feat/omega-toy-consensus-group1` (PR #7).
> Group 2 onward is deferred. This page is the canonical deferral table
> referenced from `omega-commitment/crates/omega-toy-consensus/src/lib.rs`
> and from the crate README.

LoganNet is the local 3-node openraft + omega-mock-ledger + omega-network
+ JSON-RPC harness used to wring out the Ouroboros Omega claim-apply
pipeline before any of it touches a real testnet. It exists so that
consensus, ledger, verifier, and transport pieces can be exercised
end-to-end against a single binary while the heavier components
(libp2p inbound raft actor, real network partition modelling,
membership change, dashboard UI, cross-platform CI) ship on later
groups.

## Group boundaries

### Group 1 (this PR — `omega-toy-consensus` v0.1)

In scope:

- `omega-toy-consensus` crate: bring-up + lifecycle + JSON-RPC server.
- `omega-toy-consensus run` CLI binary (single-node, in-process).
- `examples/three_node_local`: three nodes in one process, sharing the
  in-process raft dispatcher (see "Group 1 transport" below).
- Two JSON-RPC methods: `omega_submitClaim`, `omega_getState`.
- Fixed JSON-RPC application error code range `-32000..-32005`
  (see [Error code map](#error-code-map-json-rpc-application-range)).
- Loopback bind enforcement: the JSON-RPC server refuses to start on
  a non-loopback `bind` address.
- Test pack:
  - `rust-test-turmoil` — 3-node consensus scenarios over turmoil's
    simulated network for HTTP, with raft dispatch via the in-process
    static registry.
  - `rust-test-failpoints` — `omega_network::send_appendentries`,
    `omega_network::receive_vote_replay`,
    `omega_mock_ledger::writer_close`.
  - `rust-test-proptest` — JSON-RPC input + batch limit properties.
  - `rust-test-kani` — bounded toy state-machine harness for the
    snapshot install logic; see "Toy verification harnesses" below.
  - `rust-test-shuttle-loom` — generic mpsc handshake model; see
    "Toy verification harnesses".

Explicitly out of scope:

- Multi-process raft RPC. The static `RAFT_REGISTRY` in
  `omega-toy-consensus/src/node.rs` routes only within one OS
  process. Three independent `omega-toy-consensus run` processes
  cannot form a cluster; only the in-process example does. The
  `--peer <id>,<libp2p_addr>,<rpc_url>` flag exists so that
  `peers` populates leader-hint URLs and openraft's static
  membership table — the `libp2p_addr` is **not** wired into raft
  RPC at v0.1.
- Membership change RPCs.
- TLS, auth, rate limiting.
- WebSocket subscriptions (lands with `omega-api` / Goblins).
- mDNS / Kademlia peer discovery.
- Dashboard, banner, sparkline UI (lands with `omega-tui` per
  `cardano-wiki/docs/superpowers/specs/2026-05-03-loganet-cli-experience-design.md`).
- Linux / macOS CI gates (deferred to Group 2 hardening).
- Snapshot-install integration test that actually triggers a snapshot
  install across an out-of-date follower (Group 2; needs an RPC trigger
  or extended `omega_getState` snapshot fields).

### Group 2 (deferred)

- Real libp2p inbound raft request-response actor in `omega-network`.
  Removes `RAFT_REGISTRY`. `--peer <libp2p_addr>` becomes the actual
  raft transport.
- Multi-process `omega-toy-consensus run` cluster.
- Membership change RPCs (`omega_addLearner`, `omega_promoteVoter`).
- Snapshot trigger RPC + an integration test that drives an
  out-of-date follower through `install_snapshot`.
- Linux + macOS CI gates.

### Group 3 (deferred)

- Real Kani harness against `MockLedger::restore_snapshot` and the
  openraft snapshot install path (today's harness is a toy state
  machine; see "Toy verification harnesses").
- Real Shuttle / Loom model of the `WriterHandle` actor (today's
  model is a generic mpsc handshake; see "Toy verification
  harnesses").
- Strengthened `partition_majority_continues_to_commit` test (raft
  partition, not just turmoil HTTP partition; needs Group 2's real
  inbound actor first).
- Strengthened `leader_change_during_submit` assertions (no double
  apply; today's test asserts only "the call did not panic the
  transport").

### Group 4 (deferred)

- Drop in-process partition controls from `omega-toy-consensus`'s
  public surface (`pub mod test_support`); replace with
  feature-gated test-support crate.
- TLS + bearer-token auth (loopback-only is enforced today as a
  Group 1 hardening).
- Rate limiting, body-size limits beyond the current 1 MiB cap.
- Cross-cluster federation (`cluster_id` mismatch detection beyond
  string equality).

## Error code map (JSON-RPC application range)

This is the contract referenced by
`omega-toy-consensus/src/rpc/error.rs:3` and the `Errors` blocks on
`OmegaRpcServer::submit_claim` and the routing translators.

| Code   | Class               | Trigger                                                                  | Data shape                                                       |
|--------|---------------------|--------------------------------------------------------------------------|------------------------------------------------------------------|
| -32000 | `NotLeader`         | This node is not leader; openraft returned `ForwardToLeader`.            | `{leader_id, leader_rpc_url}` — either may be `null`.            |
| -32001 | `Verify`            | Proof verification failed before apply.                                  | `{verify_error}` — short string.                                  |
| -32002 | `InvalidClaim`      | Claim envelope could not be converted into a `LedgerCommand`.            | `{detail}` — short string.                                        |
| -32003 | `Replay`            | Nullifier already present in the ledger.                                 | `{sub_tree_id, leaf_index}` — coordinates of the existing leaf.   |
| -32004 | `WriterClosed`      | Writer-actor channel closed; transient.                                  | `{retryable: true}`.                                              |
| -32005 | `Timeout`           | `apply_deadline` elapsed before raft returned.                           | `{deadline_ms}`.                                                  |

Standard JSON-RPC codes also surface from this server:

| Code   | Class            | Trigger                                                                                  |
|--------|------------------|------------------------------------------------------------------------------------------|
| -32600 | `InvalidRequest` | Batch length exceeded `RpcConfig.max_batch` (default 25).                                |
| -32602 | `InvalidParams`  | Per-method jsonrpsee parameter parsing failed.                                           |
| -32603 | `InternalError`  | Openraft `Fatal`, membership-change rejection (no membership-change RPCs in Group 1), or another internal failure. |
| -32700 | `ParseError`     | Body did not parse as JSON.                                                              |

`SubmitOutcome { accepted, applied_index, reject_reason }` is returned
on **commit-time rejections** (verify / invalid / replay / internal —
the entry committed to the raft log but the state machine rejected the
apply, so `applied_index` is `Some(_)`). Pre-commit failures
(`-32000`, `-32004`, `-32005`) come back as JSON-RPC call errors.

## Group 1 transport

The conductor crate is wired to libp2p in shape — the
`LibP2pNetworkFactory` consumes outbound raft RPCs through an mpsc
channel — but the **dispatcher** (`omega-toy-consensus/src/node.rs`,
`spawn_network_dispatcher` + `dispatch_raft_request`) routes every
outbound `RaftRpcRequest` through a process-global static
`RAFT_REGISTRY` rather than over the wire. This is what makes
`examples/three_node_local` work as a 3-node cluster from a single
process. The CBOR encode/decode round-trip is preserved on each side
so that the eventual real-network drop-in is a one-file change at
the dispatcher boundary.

## Toy verification harnesses

The Kani proof
(`omega-toy-consensus/kani-proofs/snapshot_install_state_machine.rs`)
and the Shuttle model
(`omega-toy-consensus/tests/shuttle_writer_handshake.rs`) ship in
Group 1 as **structural placeholders**, not as binding verification
of the actual code. The Kani harness verifies a self-defined toy
`match` table, not `MockLedger::restore_snapshot`. The Shuttle model
exercises a generic `(u64, Sender<bool>)` mpsc handshake, not the
actual `WriterHandle::apply_claim` request/reply protocol. Both
artifacts are kept in tree to wire the gates and to capture the
right test-pack scaffolding; replacing them with real harnesses is
Group 3 work.

This is called out at the test sites and in the PR description's
test-pack table. Treat the Kani and Shuttle gate rows as
**scaffolded**, not GREEN.
