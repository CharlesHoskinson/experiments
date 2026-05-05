//! Openraft `RaftNetworkFactory` / `RaftNetwork` adapter backed by the
//! request-response actor seam.
//!
//! Each per-peer [`LibP2pNetwork`] holds a [`tokio::sync::mpsc::Sender`]
//! handle to the request-response actor (out of scope for this crate; lives
//! in the node runner). Sending an outbound RPC parks an
//! [`OutboundRaftRequest`] carrying a CBOR-encoded payload and a `oneshot`
//! reply channel. Round-trip latency is bounded by the openraft `RPCOption`
//! deadline; outbound queue depth is bounded by the channel capacity.
//!
//! The `target: u64` field on [`OutboundRaftRequest`] is the openraft
//! `NodeId` of the destination peer. It is **not** itself a peer
//! authenticator: when the inbound dispatcher lands in the next PR, it must
//! bind incoming requests to the libp2p `PeerId` of the connection and
//! verify the openraft envelope's claimed `node_id` matches the `PeerId`
//! bound at genesis. This crate is correct only against an honest sender;
//! soundness against a byzantine peer requires the inbound layer.

use std::time::Duration;

use omega_mock_ledger::OmegaRaftTypeConfig;
use openraft::error::{NetworkError as OpenRaftNetworkError, RPCError, RaftError};
use openraft::network::{RPCOption, RaftNetwork, RaftNetworkFactory};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::BasicNode;
use tokio::sync::{mpsc, oneshot};

use crate::rpc::{decode_cbor, encode_cbor, OmegaNetworkError, RaftRpcRequest, RaftRpcResponse};

/// Default outbound queue capacity per `LibP2pNetworkFactory`.
///
/// Sized for a 3-node cluster's heartbeat + replication traffic plus
/// short-term bursts; larger clusters should construct the factory with
/// [`LibP2pNetworkFactory::with_capacity`].
pub const DEFAULT_OUTBOUND_CAPACITY: usize = 256;

/// Outbound CBOR request-response payload destined for a single Raft peer.
pub struct OutboundRaftRequest {
    /// Target openraft node id. Not a peer authenticator on its own; see
    /// the module-level docs.
    pub target: u64,
    /// Target openraft node metadata.
    pub node: BasicNode,
    /// CBOR-encoded [`RaftRpcRequest`] payload. Bounded by
    /// [`crate::rpc::MAX_RAFT_RPC_BYTES`] before the receiver decodes it.
    pub payload: Vec<u8>,
    /// Reply channel carrying a CBOR-encoded [`RaftRpcResponse`] payload.
    pub reply: oneshot::Sender<Result<Vec<u8>, OmegaNetworkError>>,
}

/// Openraft network factory backed by a libp2p request-response actor.
///
/// The factory is `Clone` and shares one bounded mpsc sender across every
/// per-peer [`LibP2pNetwork`] it produces. Backpressure is applied via
/// `try_send`; a saturated queue surfaces as
/// [`OmegaNetworkError::OutboundFull`] rather than blocking openraft's
/// scheduler.
#[derive(Clone)]
pub struct LibP2pNetworkFactory {
    outbound: mpsc::Sender<OutboundRaftRequest>,
}

impl LibP2pNetworkFactory {
    /// Creates a factory backed by an existing bounded sender.
    pub fn new(outbound: mpsc::Sender<OutboundRaftRequest>) -> Self {
        Self { outbound }
    }

    /// Creates a factory plus its bound mpsc receiver, sized to `capacity`.
    /// The caller drives the receiver inside the request-response actor.
    pub fn with_capacity(capacity: usize) -> (Self, mpsc::Receiver<OutboundRaftRequest>) {
        let (tx, rx) = mpsc::channel(capacity);
        (Self { outbound: tx }, rx)
    }
}

impl RaftNetworkFactory<OmegaRaftTypeConfig> for LibP2pNetworkFactory {
    type Network = LibP2pNetwork;

    async fn new_client(&mut self, target: u64, node: &BasicNode) -> Self::Network {
        LibP2pNetwork {
            target,
            node: node.clone(),
            outbound: self.outbound.clone(),
        }
    }
}

/// Openraft network client for a single target peer.
pub struct LibP2pNetwork {
    target: u64,
    node: BasicNode,
    outbound: mpsc::Sender<OutboundRaftRequest>,
}

impl LibP2pNetwork {
    /// Sends one CBOR-encoded request and awaits the matching CBOR response,
    /// honouring the openraft RPC deadline.
    ///
    /// # Errors
    ///
    /// - [`OmegaNetworkError::Codec`] / [`OmegaNetworkError::Oversize`] —
    ///   the request could not be encoded or the response was rejected by
    ///   the codec's bounds.
    /// - [`OmegaNetworkError::OutboundClosed`] — the request-response actor
    ///   has shut down.
    /// - [`OmegaNetworkError::OutboundFull`] — the outbound queue is at
    ///   capacity; the caller should retry after backpressure clears.
    /// - [`OmegaNetworkError::ReplyDropped`] — the actor took the request
    ///   but dropped the reply channel without responding.
    /// - [`OmegaNetworkError::Timeout`] — the round-trip exceeded the
    ///   resolved deadline.
    async fn round_trip(
        &self,
        request: RaftRpcRequest,
        option: RPCOption,
    ) -> Result<RaftRpcResponse, OmegaNetworkError> {
        let payload = encode_cbor(&request)?;
        if payload.len() > crate::rpc::MAX_RAFT_RPC_BYTES {
            return Err(OmegaNetworkError::Oversize {
                actual: payload.len(),
                max: crate::rpc::MAX_RAFT_RPC_BYTES,
            });
        }
        let (reply, reply_rx) = oneshot::channel();
        let outbound = OutboundRaftRequest {
            target: self.target,
            node: self.node.clone(),
            payload,
            reply,
        };
        match self.outbound.try_send(outbound) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Closed(_)) => {
                return Err(OmegaNetworkError::OutboundClosed);
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                return Err(OmegaNetworkError::OutboundFull);
            }
        }
        let deadline = round_trip_deadline(&option);
        let response = match tokio::time::timeout(deadline, reply_rx).await {
            Ok(Ok(payload)) => payload?,
            Ok(Err(_)) => return Err(OmegaNetworkError::ReplyDropped),
            Err(_) => return Err(OmegaNetworkError::Timeout),
        };
        decode_cbor(&response)
    }
}

/// Resolves the per-RPC deadline from openraft's `RPCOption`.
///
/// `RPCOption::hard_ttl()` is the upper-bound deadline openraft expects the
/// network impl to honour. If it is absurdly small (under 50 ms), we fall
/// back to a 5-second envelope so a misconfigured caller cannot starve the
/// round-trip with a sub-millisecond deadline.
fn round_trip_deadline(option: &RPCOption) -> Duration {
    let hard = option.hard_ttl();
    if hard < Duration::from_millis(50) {
        Duration::from_secs(5)
    } else {
        hard
    }
}

impl RaftNetwork<OmegaRaftTypeConfig> for LibP2pNetwork {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<OmegaRaftTypeConfig>,
        option: RPCOption,
    ) -> Result<AppendEntriesResponse<u64>, RPCError<u64, BasicNode, RaftError<u64>>> {
        match self
            .round_trip(RaftRpcRequest::AppendEntries(Box::new(rpc)), option)
            .await
            .map_err(rpc_error)?
        {
            RaftRpcResponse::AppendEntries(response) => Ok(response),
            response => Err(rpc_error(OmegaNetworkError::WrongResponse {
                expected: "append_entries",
                actual: response.variant_name(),
            })),
        }
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<OmegaRaftTypeConfig>,
        option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<u64>,
        RPCError<u64, BasicNode, RaftError<u64, openraft::error::InstallSnapshotError>>,
    > {
        match self
            .round_trip(RaftRpcRequest::InstallSnapshot(Box::new(rpc)), option)
            .await
            .map_err(rpc_error)?
        {
            RaftRpcResponse::InstallSnapshot(response) => Ok(response),
            response => Err(rpc_error(OmegaNetworkError::WrongResponse {
                expected: "install_snapshot",
                actual: response.variant_name(),
            })),
        }
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<u64>,
        option: RPCOption,
    ) -> Result<VoteResponse<u64>, RPCError<u64, BasicNode, RaftError<u64>>> {
        match self
            .round_trip(RaftRpcRequest::Vote(rpc), option)
            .await
            .map_err(rpc_error)?
        {
            RaftRpcResponse::Vote(response) => Ok(response),
            response => Err(rpc_error(OmegaNetworkError::WrongResponse {
                expected: "vote",
                actual: response.variant_name(),
            })),
        }
    }
}

/// Wraps an [`OmegaNetworkError`] as an openraft `RPCError::Network`.
fn rpc_error<E>(error: OmegaNetworkError) -> RPCError<u64, BasicNode, RaftError<u64, E>>
where
    E: std::error::Error + 'static,
{
    RPCError::Network(OpenRaftNetworkError::new(&error))
}
