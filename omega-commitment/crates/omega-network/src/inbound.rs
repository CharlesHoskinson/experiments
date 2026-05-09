//! Inbound raft RPC handler trait.
//!
//! `RaftSwarm` calls into the implementor to dispatch a received raft RPC
//! to the local `Raft` instance and produce a response.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use libp2p::request_response::{
    self, Behaviour as RrBehaviour, Config as RrConfig, Event as RrEvent, Message as RrMessage,
    OutboundFailure, ProtocolSupport, ResponseChannel,
};
use libp2p::swarm::SwarmEvent;
use libp2p::{noise, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder};
use tokio::sync::{mpsc, oneshot};

use crate::protocol::{RaftCodec, RAFT_PROTOCOL};
use crate::rpc::{OmegaNetworkError, RaftRpcRequest, RaftRpcResponse};
use crate::OutboundRaftRequest;

/// Default request-response timeout for raft RPCs.
///
/// Sized to comfortably outlast openraft 0.9's election cycle
/// (`election_timeout_max = 3000ms` plus one heartbeat) without leaving
/// outbound oneshots dangling longer than necessary when a peer is stuck.
/// Phase 2 will plumb this from `NodeConfig` so operators can tune per
/// deployment.
pub const DEFAULT_RAFT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

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
    /// Uses [`DEFAULT_RAFT_REQUEST_TIMEOUT`] for the libp2p request-response
    /// timeout. Use [`RaftSwarm::with_request_timeout`] to override (Phase 2
    /// will plumb this from `NodeConfig`).
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
        Self::with_request_timeout(
            listen_addr,
            peers,
            outbound_rx,
            handler,
            DEFAULT_RAFT_REQUEST_TIMEOUT,
        )
        .await
    }

    /// Like [`RaftSwarm::new`] but with an explicit request-response timeout.
    ///
    /// # Errors
    ///
    /// See [`RaftSwarm::new`].
    pub async fn with_request_timeout(
        listen_addr: Multiaddr,
        peers: Vec<PeerEntry>,
        outbound_rx: mpsc::Receiver<OutboundRaftRequest>,
        handler: Arc<dyn InboundRaftHandler>,
        request_timeout: Duration,
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
                    RrConfig::default().with_request_timeout(request_timeout),
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

    /// Returns the local libp2p peer id.
    pub fn local_peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    /// Awaits the first listen address assigned by the transport.
    ///
    /// Call exactly once, before [`RaftSwarm::run`]. After `run` is spawned
    /// the swarm has a single consumer of its event stream; calling this
    /// method concurrently with `run` would race two consumers and deadlock.
    ///
    /// # Soundness
    ///
    /// Preserves: the returned address is emitted by the libp2p `Swarm` for
    /// this node after `listen_on` succeeds, so peers can dial the actual bound
    /// port when `listen_addr` used `/tcp/0`.
    ///
    /// Closes: tests do not guess ephemeral ports; they wait for the address
    /// assigned by the transport.
    pub async fn first_listen_address(&mut self) -> Multiaddr {
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => return address,
                _ => continue,
            }
        }
    }

    /// Drives the libp2p swarm until the outbound request channel closes.
    ///
    /// # Errors
    ///
    /// Returns [`OmegaNetworkError`] when an outbound request payload cannot be
    /// decoded or an inbound response cannot be encoded for the existing
    /// `LibP2pNetwork` reply channel.
    ///
    /// # Soundness
    ///
    /// Preserves: each outbound request id is paired with one pending reply
    /// channel and removed when the response or failure arrives. Each inbound
    /// request is handed to the configured [`InboundRaftHandler`] before a
    /// response is sent on the libp2p response channel.
    ///
    /// Closes: request-response correlation does not rely on process-global
    /// state; libp2p's `OutboundRequestId` selects the matching oneshot.
    pub async fn run(mut self) -> Result<(), OmegaNetworkError> {
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    self.on_swarm_event(event).await?;
                }
                outbound = self.outbound_rx.recv() => {
                    let Some(outbound) = outbound else {
                        return Ok(());
                    };
                    self.on_outbound(outbound).await?;
                }
            }
        }
    }

    async fn on_outbound(
        &mut self,
        outbound: OutboundRaftRequest,
    ) -> Result<(), OmegaNetworkError> {
        let Some(peer) = self.peers.get(&outbound.target) else {
            let _ = outbound.reply.send(Err(OmegaNetworkError::OutboundClosed));
            return Ok(());
        };
        let request: RaftRpcRequest = crate::rpc::decode_cbor(&outbound.payload)?;
        let id = self
            .swarm
            .behaviour_mut()
            .send_request(&peer.peer_id, request);
        self.pending.insert(id, outbound.reply);
        Ok(())
    }

    async fn on_swarm_event(
        &mut self,
        event: SwarmEvent<RrEvent<RaftRpcRequest, RaftRpcResponse>>,
    ) -> Result<(), OmegaNetworkError> {
        match event {
            SwarmEvent::Behaviour(RrEvent::Message { peer, message, .. }) => match message {
                RrMessage::Request {
                    request, channel, ..
                } => {
                    self.on_inbound_request(peer, request, channel).await?;
                }
                RrMessage::Response {
                    request_id,
                    response,
                } => {
                    if let Some(tx) = self.pending.remove(&request_id) {
                        let bytes = crate::rpc::encode_cbor(&response)?;
                        let _ = tx.send(Ok(bytes));
                    }
                }
            },
            SwarmEvent::Behaviour(RrEvent::OutboundFailure {
                request_id, error, ..
            }) => {
                if let Some(tx) = self.pending.remove(&request_id) {
                    let _ = tx.send(Err(map_outbound_failure(error)));
                }
            }
            SwarmEvent::Behaviour(RrEvent::InboundFailure { error, .. }) => {
                tracing::debug!(?error, "libp2p inbound raft RPC failure");
            }
            SwarmEvent::Behaviour(RrEvent::ResponseSent { .. }) => {}
            _ => {}
        }
        Ok(())
    }

    async fn on_inbound_request(
        &mut self,
        peer: PeerId,
        request: RaftRpcRequest,
        channel: ResponseChannel<RaftRpcResponse>,
    ) -> Result<(), OmegaNetworkError> {
        // Phase 2: verify `peer` matches the openraft NodeId the envelope
        // claims, per the byzantine-binding limitation in `lib.rs`.
        let _ = self.peer_id_to_node.get(&peer);
        match self.handler.handle(request).await {
            Ok(response) => {
                let _ = self.swarm.behaviour_mut().send_response(channel, response);
            }
            Err(error) => {
                // Drop the response channel rather than fabricate a
                // wrong-typed reply. The requester sees `OutboundFailure`
                // (closed substream) and openraft retries on its normal
                // schedule; better than a `Vote` reply to an `AppendEntries`
                // request which would surface as a confusing typed mismatch.
                tracing::warn!(
                    ?error,
                    "inbound raft RPC handler errored; dropping response channel"
                );
                drop(channel);
            }
        }
        Ok(())
    }
}

/// Maps a libp2p `OutboundFailure` to the most semantically-precise
/// [`OmegaNetworkError`] variant.
fn map_outbound_failure(error: OutboundFailure) -> OmegaNetworkError {
    match error {
        OutboundFailure::Timeout => OmegaNetworkError::Timeout,
        OutboundFailure::ConnectionClosed => OmegaNetworkError::OutboundClosed,
        OutboundFailure::DialFailure => OmegaNetworkError::OutboundClosed,
        OutboundFailure::UnsupportedProtocols => OmegaNetworkError::Codec(
            "remote peer does not advertise the omega raft protocol".into(),
        ),
        OutboundFailure::Io(err) => OmegaNetworkError::Codec(err.to_string()),
    }
}
