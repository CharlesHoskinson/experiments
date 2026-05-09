//! Inbound raft RPC handler trait.
//!
//! `RaftSwarm` calls into the implementor to dispatch a received raft RPC
//! to the local `Raft` instance and produce a response.

use async_trait::async_trait;

use crate::rpc::{OmegaNetworkError, RaftRpcRequest, RaftRpcResponse};

/// Application-side handler for inbound raft RPCs.
///
/// `omega-toy-consensus` provides a concrete impl that calls
/// `Raft::append_entries` / `Raft::vote` / `Raft::install_snapshot` on the
/// local node and returns the response.
///
/// # Soundness
///
/// Preserves: the swarm calls `handle` exactly once per inbound request and
/// awaits the future to completion before flushing the response. Out-of-order
/// responses cannot occur because libp2p `request_response` pairs each
/// request with its own substream.
///
/// Closes: the inbound side cannot bypass the local `Raft` instance; every
/// received RPC goes through `handle`.
#[async_trait]
pub trait InboundRaftHandler: Send + Sync + 'static {
    /// Dispatches an inbound raft RPC to the local node.
    ///
    /// # Errors
    ///
    /// Returns [`OmegaNetworkError`] when the local raft dispatcher rejects
    /// the request or cannot encode the matching response.
    async fn handle(&self, request: RaftRpcRequest) -> Result<RaftRpcResponse, OmegaNetworkError>;
}
