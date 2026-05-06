//! Node lifecycle.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, OnceLock};

use jsonrpsee::server::middleware::rpc::{
    Batch, Notification, Request, RpcServiceBuilder, RpcServiceT,
};
use jsonrpsee::server::{
    BatchRequestConfig, MethodResponse, ServerBuilder, ServerConfig, ServerHandle,
};
use jsonrpsee::types::{ErrorCode, Id};
use omega_network::rpc::{
    decode_cbor, encode_cbor, OmegaNetworkError, RaftRpcRequest, RaftRpcResponse,
};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::rpc::server::{OmegaRpcImpl, OmegaRpcServer, OmegaRpcShared};
use crate::{ConsensusError, NodeConfig};

type Raft = openraft::Raft<omega_mock_ledger::OmegaRaftTypeConfig>;

static RAFT_REGISTRY: OnceLock<Mutex<BTreeMap<u64, Raft>>> = OnceLock::new();
static RAFT_LINK_BLOCKS: OnceLock<Mutex<BTreeSet<(u64, u64)>>> = OnceLock::new();

fn raft_registry() -> &'static Mutex<BTreeMap<u64, Raft>> {
    RAFT_REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn raft_link_blocks() -> &'static Mutex<BTreeSet<(u64, u64)>> {
    RAFT_LINK_BLOCKS.get_or_init(|| Mutex::new(BTreeSet::new()))
}

/// Live LoganNet node.
///
/// See [`crate::start`] for the public entry point.
pub struct Node;

/// Handle for graceful shutdown of a running [`Node`].
pub struct NodeHandle {
    node_id: u64,
    shutdown_tx: oneshot::Sender<()>,
    server_handle: ServerHandle,
    network_join: JoinHandle<()>,
    join: JoinHandle<Result<(), ConsensusError>>,
    raft: Raft,
}

impl Node {
    /// Brings the node up.
    ///
    /// Mounts SQLite plus the writer actor, creates the openraft instance,
    /// registers the local raft RPC target, binds JSON-RPC, then initializes a
    /// fresh static cluster when this node has the lowest configured id.
    ///
    /// # Errors
    ///
    /// - [`ConsensusError::Storage`] - SQLite open, schema initialization, or
    ///   writer actor startup failed.
    /// - [`ConsensusError::Raft`] - openraft construction, initial membership,
    ///   or raft dispatcher registry locking failed.
    /// - [`ConsensusError::RpcBind`] - the JSON-RPC server failed to bind the
    ///   configured address.
    ///
    /// # Soundness
    ///
    /// Preserves: every submitted claim reaches the mock-ledger through
    /// openraft's state-machine apply path, preserving the ledger
    /// verify-before-mutate invariant.
    ///
    /// Closes: direct RPC access never exposes the writer actor, only the raft
    /// client-write API.
    ///
    /// Fails on: storage open errors, raft initialization errors, registry
    /// locking errors, RPC bind errors, and malformed static node
    /// configuration.
    pub async fn start(config: NodeConfig) -> Result<NodeHandle, ConsensusError> {
        let ledger = Arc::new(omega_mock_ledger::MockLedger::open(&config.data_dir)?);
        let storage = omega_mock_ledger::MockLedgerStorage::new((*ledger).clone());
        let (log_store, state_machine) = storage.openraft_parts();

        let (network_factory, outbound_rx) = omega_network::LibP2pNetworkFactory::with_capacity(
            omega_network::DEFAULT_OUTBOUND_CAPACITY,
        );
        let network_join = spawn_network_dispatcher(config.node_id, outbound_rx);

        let raft_config = openraft::Config {
            cluster_name: config.cluster_id.clone(),
            heartbeat_interval: 250,
            election_timeout_min: 1500,
            election_timeout_max: 3000,
            ..Default::default()
        };
        let raft = openraft::Raft::new(
            config.node_id,
            Arc::new(raft_config),
            network_factory,
            log_store,
            state_machine,
        )
        .await
        .map_err(|error| ConsensusError::Raft(error.to_string()))?;

        register_raft(config.node_id, raft.clone())?;

        if should_initialize_cluster(&config, &raft) {
            let members = initial_members(&config);
            raft.initialize(members)
                .await
                .map_err(|error| ConsensusError::Raft(error.to_string()))?;
        }

        let shared = Arc::new(OmegaRpcShared {
            node_id: config.node_id,
            raft: raft.clone(),
            ledger: ledger.clone(),
            peers: config.peers.clone(),
            apply_deadline: config.apply_deadline,
        });
        let rpc_impl = OmegaRpcImpl { inner: shared };
        let server_config = ServerConfig::builder()
            .max_request_body_size(config.rpc.max_request_bytes)
            .max_subscriptions_per_connection(0)
            .set_batch_request_config(BatchRequestConfig::Unlimited)
            .build();
        let max_batch = usize::from(config.rpc.max_batch);
        let rpc_middleware =
            RpcServiceBuilder::new().layer_fn(move |service| BatchLimit { service, max_batch });
        let server = ServerBuilder::with_config(server_config)
            .set_rpc_middleware(rpc_middleware)
            .build(config.rpc.bind)
            .await
            .map_err(|error| ConsensusError::RpcBind {
                addr: config.rpc.bind,
                source: error,
            })?;
        let server_handle = server.start(rpc_impl.into_rpc());

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let join: JoinHandle<Result<(), ConsensusError>> = tokio::spawn(async move {
            let _ = shutdown_rx.await;
            Ok(())
        });

        Ok(NodeHandle {
            node_id: config.node_id,
            shutdown_tx,
            server_handle,
            network_join,
            join,
            raft,
        })
    }
}

#[derive(Clone)]
struct BatchLimit<S> {
    service: S,
    max_batch: usize,
}

impl<S> RpcServiceT for BatchLimit<S>
where
    S: Send
        + Sync
        + Clone
        + RpcServiceT<
            MethodResponse = MethodResponse,
            BatchResponse = MethodResponse,
            NotificationResponse = MethodResponse,
        > + 'static,
{
    type BatchResponse = MethodResponse;
    type MethodResponse = MethodResponse;
    type NotificationResponse = MethodResponse;

    fn call<'a>(
        &self,
        request: Request<'a>,
    ) -> impl std::future::Future<Output = Self::MethodResponse> + Send + 'a {
        let service = self.service.clone();
        async move { service.call(request).await }
    }

    fn batch<'a>(
        &self,
        batch: Batch<'a>,
    ) -> impl std::future::Future<Output = Self::BatchResponse> + Send + 'a {
        let service = self.service.clone();
        let max_batch = self.max_batch;
        async move {
            if batch.len() > max_batch {
                MethodResponse::error(Id::Null, ErrorCode::InvalidRequest)
            } else {
                service.batch(batch).await
            }
        }
    }

    fn notification<'a>(
        &self,
        notification: Notification<'a>,
    ) -> impl std::future::Future<Output = Self::NotificationResponse> + Send + 'a {
        let service = self.service.clone();
        async move { service.notification(notification).await }
    }
}

impl NodeHandle {
    /// Initiates graceful shutdown.
    ///
    /// Stops new JSON-RPC requests, unregisters the node from the in-process
    /// raft dispatcher, signals the runtime task, and shuts down openraft.
    ///
    /// # Errors
    ///
    /// - [`ConsensusError::ShutdownJoin`] - runtime task panicked.
    /// - [`ConsensusError::Raft`] - openraft shutdown returned an error.
    ///
    /// # Soundness
    ///
    /// Preserves: shutdown stops accepting external submits before removing
    /// the node from raft RPC dispatch.
    ///
    /// Closes: stale peers cannot route new raft RPCs to a dropped handle after
    /// unregister completes.
    ///
    /// Fails on: join failures or openraft shutdown failures.
    pub async fn shutdown(self) -> Result<(), ConsensusError> {
        let _ = self.server_handle.stop();
        unregister_raft(self.node_id);
        self.network_join.abort();
        let _ = self.shutdown_tx.send(());
        self.join
            .await
            .map_err(|error| ConsensusError::ShutdownJoin(error.to_string()))??;
        self.raft
            .shutdown()
            .await
            .map_err(|error| ConsensusError::Raft(error.to_string()))?;
        Ok(())
    }
}

fn should_initialize_cluster(config: &NodeConfig, raft: &Raft) -> bool {
    let is_initialiser = std::iter::once(config.node_id)
        .chain(config.peers.iter().map(|peer| peer.node_id))
        .min()
        == Some(config.node_id);
    is_initialiser && raft.metrics().borrow().last_log_index.is_none()
}

fn initial_members(config: &NodeConfig) -> BTreeMap<u64, openraft::BasicNode> {
    let mut members = BTreeMap::new();
    members.insert(
        config.node_id,
        openraft::BasicNode::new(config.libp2p_listen.clone()),
    );
    for peer in &config.peers {
        members.insert(
            peer.node_id,
            openraft::BasicNode::new(peer.libp2p_addr.clone()),
        );
    }
    members
}

fn register_raft(node_id: u64, raft: Raft) -> Result<(), ConsensusError> {
    let mut registry = raft_registry()
        .lock()
        .map_err(|error| ConsensusError::Raft(format!("raft registry poisoned: {error}")))?;
    registry.insert(node_id, raft);
    Ok(())
}

fn unregister_raft(node_id: u64) {
    if let Ok(mut registry) = raft_registry().lock() {
        registry.remove(&node_id);
    }
}

fn route_raft(node_id: u64) -> Option<Raft> {
    raft_registry()
        .lock()
        .ok()
        .and_then(|registry| registry.get(&node_id).cloned())
}

pub(crate) fn clear_raft_link_blocks_for_test() {
    if let Ok(mut blocks) = raft_link_blocks().lock() {
        blocks.clear();
    }
}

pub(crate) fn partition_raft_link_for_test(a: u64, b: u64) {
    if let Ok(mut blocks) = raft_link_blocks().lock() {
        blocks.insert((a, b));
        blocks.insert((b, a));
    }
}

fn raft_link_blocked(source: u64, target: u64) -> bool {
    raft_link_blocks()
        .lock()
        .map(|blocks| blocks.contains(&(source, target)))
        .unwrap_or(true)
}

fn spawn_network_dispatcher(
    source: u64,
    mut outbound_rx: mpsc::Receiver<omega_network::OutboundRaftRequest>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(outbound) = outbound_rx.recv().await {
            let response = dispatch_raft_request(source, outbound.target, &outbound.payload).await;
            let _ = outbound.reply.send(response);
        }
    })
}

async fn dispatch_raft_request(
    source: u64,
    target: u64,
    payload: &[u8],
) -> Result<Vec<u8>, OmegaNetworkError> {
    if raft_link_blocked(source, target) {
        return Err(OmegaNetworkError::Timeout);
    }
    let raft = route_raft(target).ok_or(OmegaNetworkError::OutboundClosed)?;
    let request: RaftRpcRequest = decode_cbor(payload)?;
    let response = match request {
        RaftRpcRequest::AppendEntries(request) => {
            let response = raft
                .append_entries(*request)
                .await
                .map_err(|error| OmegaNetworkError::Codec(error.to_string()))?;
            RaftRpcResponse::AppendEntries(response)
        }
        RaftRpcRequest::InstallSnapshot(request) => {
            let response = raft
                .install_snapshot(*request)
                .await
                .map_err(|error| OmegaNetworkError::Codec(error.to_string()))?;
            RaftRpcResponse::InstallSnapshot(response)
        }
        RaftRpcRequest::Vote(request) => {
            let response = raft
                .vote(request)
                .await
                .map_err(|error| OmegaNetworkError::Codec(error.to_string()))?;
            RaftRpcResponse::Vote(response)
        }
    };
    encode_cbor(&response)
}
