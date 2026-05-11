//! LoganNet keystone: openraft + omega-mock-ledger + omega-network + JSON-RPC.
//!
//! # Overview
//!
//! `omega-toy-consensus` is the conductor crate of the LoganNet local 3-node
//! Raft harness. It owns wiring and lifecycle only: consensus rules stay in
//! openraft, state-machine rules stay in [`omega_mock_ledger`], verification
//! stays in `omega_claim_verifier`, and transport stays in [`omega_network`].
//! Every line in this crate is either bringing one of those four up, routing a
//! request between them, or exposing them via the JSON-RPC surface or the
//! run-binary.
//!
//! # Design context
//!
//! - Spec: [`docs/superpowers/specs/2026-05-05-omega-toy-consensus-design.md`][1]
//! - LoganNet roadmap: [`cardano-wiki/wiki/pages/loganet-roadmap.md`][2]
//! - OpenSpec change (upstream crates): [`add-proof-experiment-harness`][3]
//!
//! [1]: ../../../docs/superpowers/specs/2026-05-05-omega-toy-consensus-design.md
//! [2]: ../../../cardano-wiki/wiki/pages/loganet-roadmap.md
//! [3]: ../../../openspec/changes/add-proof-experiment-harness/
//! [4]: ../../examples/three_node_local.rs
//!
//! # Tier of trust
//!
//! Soundness-bearing wiring. This crate does not verify proofs (the verifier
//! does) and does not apply state (the writer actor does), but it is the
//! component that ensures `Raft::client_write` is the only path to apply, that
//! a non-leader returns `-32000 NotLeader` rather than silently proxying, and
//! that the writer actor's lifecycle is bounded by `Node::shutdown`.
//!
//! # v0.1 limitations
//!
//! - Localhost-only RPC (`127.0.0.1:800N`); loopback bind enforced; no TLS,
//!   no auth, no rate limiting.
//! - Two RPC methods only: `omega_submitClaim`, `omega_getState`.
//! - HTTP only; WebSocket subscriptions land with `omega-api` (Goblins).
//! - No membership change; static `--peer` topology.
//! - Raft RPC uses static libp2p request-response peers. `--peer` entries
//!   carry both the remote `PeerId` and dial address; mDNS / Kademlia
//!   discovery remains deferred.
//! - No mDNS / Kademlia discovery.
//! - Windows + 1.95.0 toolchain only; Linux/macOS CI deferred to Group 2.
//! - Toy Kani + Shuttle harnesses (see [`loganet-roadmap`][2] §
//!   "Toy verification harnesses").
//! - See [`loganet-roadmap`][2] for the full deferral table.
//!
//! # Conventions
//!
//! - Bring-up and shutdown are async; everything else is sync where possible.
//! - Errors surface via [`ConsensusError`] internally and JSON-RPC error codes
//!   `-32000..-32005` externally; mapping lives in `routing` + `rpc::error`.
//! - Soundness-bearing public items document the invariant they preserve, the
//!   attack class they close, and the failure mode left to callers.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod config;
pub mod error;
pub mod node;
pub mod routing;
pub mod rpc;

pub use config::{NodeConfig, PeerConfig, RpcConfig};
pub use error::ConsensusError;
pub use node::{Node, NodeHandle};
pub use rpc::types::{LogIdView, NodeRole, NodeState, SubmitOutcome};

/// Boots a single Raft node, mounts the mock ledger, binds the JSON-RPC
/// server, and returns a handle for graceful shutdown.
///
/// # Errors
///
/// - [`ConsensusError::Storage`] - SQLite open or schema initialisation failed.
/// - [`ConsensusError::RpcBind`] - the JSON-RPC HTTP server failed to bind
///   `config.rpc.bind`.
/// - [`ConsensusError::Raft`] - openraft rejected the initial membership.
///
/// # Soundness
///
/// Preserves: all accepted writes enter the mock-ledger through openraft's
/// replicated state-machine path after SQLite storage and the writer actor are
/// mounted.
///
/// Closes: direct localhost RPC access cannot reach the writer actor without a
/// successful `Raft::client_write`.
///
/// Fails on: cluster identity is only the `cluster_id` string supplied by the
/// operator. Mismatched deployment intent with matching strings can still form
/// quorum across logically distinct clusters.
///
/// # Examples
///
/// ```no_run
/// # async fn run() -> Result<(), omega_toy_consensus::ConsensusError> {
/// use omega_toy_consensus::{start, NodeConfig};
/// let config = NodeConfig::single_node_localhost(1)?;
/// let handle = start(config).await?;
/// handle.shutdown().await?;
/// # Ok(()) }
/// ```
pub async fn start(config: NodeConfig) -> Result<NodeHandle, ConsensusError> {
    Node::start(config).await
}
