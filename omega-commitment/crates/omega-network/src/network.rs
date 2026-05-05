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

/// Outbound CBOR request-response payload destined for a single Raft peer.
pub struct OutboundRaftRequest {
    /// Target openraft node id.
    pub target: u64,
    /// Target openraft node metadata.
    pub node: BasicNode,
    /// CBOR-encoded [`RaftRpcRequest`] payload.
    pub payload: Vec<u8>,
    /// Reply channel carrying a CBOR-encoded [`RaftRpcResponse`] payload.
    pub reply: oneshot::Sender<Result<Vec<u8>, OmegaNetworkError>>,
}

/// Openraft network factory backed by a libp2p request-response actor.
#[derive(Clone)]
pub struct LibP2pNetworkFactory {
    outbound: mpsc::UnboundedSender<OutboundRaftRequest>,
}

impl LibP2pNetworkFactory {
    /// Creates a factory from the request-response actor sender.
    pub fn new(outbound: mpsc::UnboundedSender<OutboundRaftRequest>) -> Self {
        Self { outbound }
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
    outbound: mpsc::UnboundedSender<OutboundRaftRequest>,
}

impl LibP2pNetwork {
    async fn round_trip(
        &self,
        request: RaftRpcRequest,
    ) -> Result<RaftRpcResponse, OmegaNetworkError> {
        let payload = encode_cbor(&request)?;
        let (reply, reply_rx) = oneshot::channel();
        let outbound = OutboundRaftRequest {
            target: self.target,
            node: self.node.clone(),
            payload,
            reply,
        };
        self.outbound
            .send(outbound)
            .map_err(|_| OmegaNetworkError::OutboundClosed)?;
        let response = reply_rx
            .await
            .map_err(|_| OmegaNetworkError::ReplyDropped)??;
        decode_cbor(&response)
    }
}

impl RaftNetwork<OmegaRaftTypeConfig> for LibP2pNetwork {
    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<OmegaRaftTypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<u64>, RPCError<u64, BasicNode, RaftError<u64>>> {
        match self
            .round_trip(RaftRpcRequest::AppendEntries(Box::new(rpc)))
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
        _option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<u64>,
        RPCError<u64, BasicNode, RaftError<u64, openraft::error::InstallSnapshotError>>,
    > {
        match self
            .round_trip(RaftRpcRequest::InstallSnapshot(Box::new(rpc)))
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
        _option: RPCOption,
    ) -> Result<VoteResponse<u64>, RPCError<u64, BasicNode, RaftError<u64>>> {
        match self
            .round_trip(RaftRpcRequest::Vote(rpc))
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

fn rpc_error<E>(error: OmegaNetworkError) -> RPCError<u64, BasicNode, RaftError<u64, E>>
where
    E: std::error::Error + 'static,
{
    RPCError::Network(OpenRaftNetworkError::new(&error))
}
