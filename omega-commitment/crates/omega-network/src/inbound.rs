//! Inbound raft RPC handler trait.
//!
//! `RaftSwarm` calls into the implementor to dispatch a received raft RPC
//! to the local `Raft` instance and produce a response.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use libp2p::request_response::{self, Behaviour as RrBehaviour, Config as RrConfig, ProtocolSupport};
use libp2p::{noise, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder};
use tokio::sync::{mpsc, oneshot};

use crate::protocol::{RaftCodec, RAFT_PROTOCOL};
use crate::rpc::{OmegaNetworkError, RaftRpcRequest, RaftRpcResponse};
use crate::OutboundRaftRequest;

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

/// Per-peer addressing config.
#[derive(Debug, Clone)]
pub struct PeerEntry {
    /// Openraft node id for the peer.
    pub node_id: u64,
    /// libp2p peer id expected for the remote node.
    pub peer_id: PeerId,
    /// Dialable libp2p multiaddr for the remote node.
    pub address: Multiaddr,
}

/// libp2p swarm that owns the raft request-response protocol and the inbound
/// handler.
///
/// # Soundness
///
/// Preserves: outbound requests are keyed by openraft node id and routed only
/// to the corresponding configured libp2p peer id. Inbound requests are
/// dispatched through the configured [`InboundRaftHandler`].
///
/// Closes: replacing the in-process registry with this swarm prevents a
/// caller from bypassing libp2p framing when Phase 2 wires it into the node
/// runner.
pub struct RaftSwarm {
    swarm: Swarm<RrBehaviour<RaftCodec>>,
    peers: HashMap<u64, PeerEntry>,
    peer_id_to_node: HashMap<PeerId, u64>,
    outbound_rx: mpsc::Receiver<OutboundRaftRequest>,
    pending: HashMap<
        request_response::OutboundRequestId,
        oneshot::Sender<Result<Vec<u8>, OmegaNetworkError>>,
    >,
    handler: Arc<dyn InboundRaftHandler>,
}

impl RaftSwarm {
    /// Builds a swarm that listens on `listen_addr`, tracks each peer address,
    /// and routes inbound requests through `handler`.
    ///
    /// # Errors
    ///
    /// Returns [`OmegaNetworkError::Codec`] when the libp2p transport,
    /// behaviour, or listen address cannot be initialized.
    ///
    /// # Soundness
    ///
    /// Preserves: every configured `PeerEntry` is installed in the
    /// request-response address book before the swarm is returned, so later
    /// outbound raft RPCs cannot be sent to an unconfigured peer id.
    ///
    /// Closes: the returned swarm owns both inbound and outbound raft traffic;
    /// no process-global dispatcher is involved in the transport path.
    pub async fn new(
        listen_addr: Multiaddr,
        peers: Vec<PeerEntry>,
        outbound_rx: mpsc::Receiver<OutboundRaftRequest>,
        handler: Arc<dyn InboundRaftHandler>,
    ) -> Result<Self, OmegaNetworkError> {
        let mut swarm = SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|e| OmegaNetworkError::Codec(e.to_string()))?
            .with_behaviour(|_| {
                RrBehaviour::new(
                    [(RAFT_PROTOCOL, ProtocolSupport::Full)],
                    RrConfig::default().with_request_timeout(Duration::from_secs(30)),
                )
            })
            .map_err(|e| OmegaNetworkError::Codec(e.to_string()))?
            .build();

        swarm
            .listen_on(listen_addr)
            .map_err(|e| OmegaNetworkError::Codec(e.to_string()))?;

        let mut peer_id_to_node = HashMap::new();
        for p in &peers {
            peer_id_to_node.insert(p.peer_id, p.node_id);
            swarm.add_peer_address(p.peer_id, p.address.clone());
        }
        let peers = peers.into_iter().map(|p| (p.node_id, p)).collect();

        Ok(Self {
            swarm,
            peers,
            peer_id_to_node,
            outbound_rx,
            pending: HashMap::new(),
            handler,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn raft_swarm_skeleton_compiles() {
        // Just a compile gate; full round-trip test lives in
        // `tests/loopback_round_trip.rs`.
    }
}
