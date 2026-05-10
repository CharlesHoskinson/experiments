//! Node lifecycle.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use jsonrpsee::server::middleware::rpc::{
    Batch, Notification, Request, RpcServiceBuilder, RpcServiceT,
};
use jsonrpsee::server::{
    BatchRequestConfig, MethodResponse, ServerBuilder, ServerConfig, ServerHandle,
};
use jsonrpsee::types::{ErrorCode, Id};
use omega_network::inbound::{InboundRaftHandler, PeerEntry, RaftSwarm};
use omega_network::rpc::{OmegaNetworkError, RaftRpcRequest, RaftRpcResponse};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::rpc::server::{OmegaRpcImpl, OmegaRpcServer, OmegaRpcShared};
use crate::{ConsensusError, NodeConfig};

type Raft = openraft::Raft<omega_mock_ledger::OmegaRaftTypeConfig>;

/// Live LoganNet node.
///
/// See [`crate::start`] for the public entry point.
pub struct Node;

/// Handle for graceful shutdown of a running [`Node`].
///
/// Owns the JSON-RPC server, the libp2p raft swarm task, the runtime task,
/// and the `Raft` instance. The handle's `Drop` does NOT shut anything down;
/// callers must invoke [`NodeHandle::shutdown`] for an orderly stop.
///
/// `shutdown` order matters: stop new RPC traffic, shut down openraft (which
/// drops the per-peer network senders and lets the swarm drain naturally),
/// wait for the swarm task to exit (with a timeout fallback), then signal
/// the runtime task. Reversing those steps would let an in-flight
/// client_write reach a dropped writer actor or kill the swarm before
/// in-flight inbound responses flush.
pub struct NodeHandle {
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
    /// starts the libp2p raft RPC swarm, binds JSON-RPC, then initializes a
    /// fresh static cluster when this node has the lowest configured id.
    ///
    /// # Errors
    ///
    /// - [`ConsensusError::Storage`] - SQLite open, schema initialization, or
    ///   writer actor startup failed.
    /// - [`ConsensusError::Raft`] - openraft construction or initial
    ///   membership failed.
    /// - [`ConsensusError::Network`] - raft swarm startup failed.
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
    /// Fails on: storage open errors, raft initialization errors, libp2p
    /// identity or address errors, RPC bind errors, and malformed static node
    /// configuration.
    pub async fn start(config: NodeConfig) -> Result<NodeHandle, ConsensusError> {
        validate_config(&config)?;
        let identity_keypair = load_or_create_identity(&config)?;
        let ledger = Arc::new(omega_mock_ledger::MockLedger::open(&config.data_dir)?);
        let storage = omega_mock_ledger::MockLedgerStorage::new((*ledger).clone());
        let (log_store, state_machine) = storage.openraft_parts();

        let (network_factory, outbound_rx) = omega_network::LibP2pNetworkFactory::with_capacity(
            omega_network::DEFAULT_OUTBOUND_CAPACITY,
        );

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

        let listen_addr: omega_network::Multiaddr = config
            .libp2p_listen
            .parse()
            .map_err(|e| ConsensusError::Config(format!("libp2p_listen parse: {e}")))?;
        let peer_entries = config
            .peers
            .iter()
            .map(|peer| {
                let peer_id = peer.libp2p_peer_id.parse().map_err(|e| {
                    ConsensusError::Config(format!("peer {} peer_id: {e}", peer.node_id))
                })?;
                let address = peer.libp2p_addr.parse().map_err(|e| {
                    ConsensusError::Config(format!("peer {} addr: {e}", peer.node_id))
                })?;
                Ok(PeerEntry {
                    node_id: peer.node_id,
                    peer_id,
                    address,
                })
            })
            .collect::<Result<Vec<_>, ConsensusError>>()?;
        let handler = Arc::new(OmegaInboundHandler { raft: raft.clone() });
        let mut swarm = RaftSwarm::with_keypair_and_request_timeout(
            identity_keypair,
            listen_addr,
            peer_entries,
            outbound_rx,
            handler,
            config.effective_raft_rpc_timeout(),
        )
        .await
        .map_err(ConsensusError::Network)?;
        let local_peer_id = swarm.local_peer_id();
        let bound_addr = swarm.first_listen_address().await;
        tracing::info!(
            node_id = config.node_id,
            %local_peer_id,
            %bound_addr,
            "raft libp2p swarm listening"
        );
        let network_join = tokio::spawn(async move {
            if let Err(error) = swarm.run().await {
                tracing::error!(?error, "raft libp2p swarm exited with error");
            }
        });

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

struct OmegaInboundHandler {
    raft: Raft,
}

#[async_trait]
impl InboundRaftHandler for OmegaInboundHandler {
    async fn handle(&self, request: RaftRpcRequest) -> Result<RaftRpcResponse, OmegaNetworkError> {
        match request {
            RaftRpcRequest::AppendEntries(request) => {
                let response = self
                    .raft
                    .append_entries(*request)
                    .await
                    .map_err(|error| OmegaNetworkError::Codec(error.to_string()))?;
                Ok(RaftRpcResponse::AppendEntries(response))
            }
            RaftRpcRequest::InstallSnapshot(request) => {
                let response = self
                    .raft
                    .install_snapshot(*request)
                    .await
                    .map_err(|error| OmegaNetworkError::Codec(error.to_string()))?;
                Ok(RaftRpcResponse::InstallSnapshot(response))
            }
            RaftRpcRequest::Vote(request) => {
                let response = self
                    .raft
                    .vote(request)
                    .await
                    .map_err(|error| OmegaNetworkError::Codec(error.to_string()))?;
                Ok(RaftRpcResponse::Vote(response))
            }
        }
    }
}

impl NodeHandle {
    /// Initiates graceful shutdown.
    ///
    /// Order matters and is the load-bearing invariant of this method:
    ///
    /// 1. Stop new JSON-RPC requests so no fresh `client_write` enters
    ///    the raft pipeline.
    /// 2. Shut down openraft. This drops the network factory's per-peer
    ///    senders, which closes the swarm's `outbound_rx` and lets
    ///    `RaftSwarm::run` exit naturally — in-flight inbound responses
    ///    get a chance to flush rather than being killed mid-send.
    /// 3. Wait up to `SHUTDOWN_DRAIN` for the swarm task to drain. If
    ///    it doesn't exit in that window, abort it (so a stuck swarm
    ///    cannot wedge `NodeHandle::shutdown` indefinitely).
    /// 4. Signal the runtime task and join.
    ///
    /// # Errors
    ///
    /// - [`ConsensusError::ShutdownJoin`] - runtime task panicked.
    /// - [`ConsensusError::Raft`] - openraft shutdown returned an error.
    ///
    /// # Soundness
    ///
    /// Preserves: shutdown stops accepting external submits before
    /// dropping the raft instance, and the swarm gets a graceful drain
    /// window so a peer mid-RPC sees a clean substream close instead of
    /// an aborted task.
    ///
    /// Closes: stale peers cannot route new raft RPCs through the local
    /// libp2p listener once the swarm task has exited.
    ///
    /// Fails on: openraft shutdown failures and join failures. A swarm
    /// that does not drain within `SHUTDOWN_DRAIN` is aborted; that
    /// path is silent because the abort is a fallback after the graceful
    /// path was already attempted.
    pub async fn shutdown(self) -> Result<(), ConsensusError> {
        const SHUTDOWN_DRAIN: std::time::Duration = std::time::Duration::from_secs(5);

        let _ = self.server_handle.stop();
        self.raft
            .shutdown()
            .await
            .map_err(|error| ConsensusError::Raft(error.to_string()))?;
        // Keep the JoinHandle in scope through the timeout so we can abort
        // it if the swarm doesn't drain — `tokio::time::timeout` drops the
        // inner future on Elapsed, and a dropped JoinHandle detaches
        // (continues running) rather than aborts.
        let abort_handle = self.network_join.abort_handle();
        if tokio::time::timeout(SHUTDOWN_DRAIN, self.network_join)
            .await
            .is_err()
        {
            tracing::warn!(
                "raft swarm did not drain within {:?}; aborting",
                SHUTDOWN_DRAIN
            );
            abort_handle.abort();
        }
        let _ = self.shutdown_tx.send(());
        self.join
            .await
            .map_err(|error| ConsensusError::ShutdownJoin(error.to_string()))??;
        Ok(())
    }
}

/// Validates a [`NodeConfig`] before bring-up.
///
/// Closes a PR #7-review class of operator misconfiguration:
///
/// - **N7-M1** — The spec says LoganNet v0.1 binds JSON-RPC localhost-only
///   ("no TLS, no auth, no rate limiting"). A non-loopback bind would expose
///   an unauthenticated write-RPC to the network. Reject anything not bound
///   to a loopback address with [`ConsensusError::Config`].
fn validate_config(config: &NodeConfig) -> Result<(), ConsensusError> {
    if !config.rpc.bind.ip().is_loopback() {
        return Err(ConsensusError::Config(format!(
            "rpc.bind must be a loopback address in v0.1 (got {}); the v0.1 JSON-RPC \
             surface is unauthenticated and the spec forbids non-loopback bind. \
             Bind to 127.0.0.1 or ::1.",
            config.rpc.bind
        )));
    }
    Ok(())
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

fn identity_path(config: &NodeConfig) -> PathBuf {
    config.identity_file.clone().unwrap_or_else(|| {
        if config.data_dir.extension().is_some() {
            config.data_dir.with_extension("identity.bin")
        } else {
            config.data_dir.join("identity.bin")
        }
    })
}

fn load_or_create_identity(
    config: &NodeConfig,
) -> Result<omega_network::identity::Keypair, ConsensusError> {
    let path = identity_path(config);
    match std::fs::read(&path) {
        Ok(bytes) => {
            omega_network::identity::Keypair::from_protobuf_encoding(&bytes).map_err(|error| {
                ConsensusError::Identity(format!(
                    "identity_file {} decode: {error}",
                    path.display()
                ))
            })
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let keypair = omega_network::identity::Keypair::generate_ed25519();
            let bytes = keypair.to_protobuf_encoding().map_err(|error| {
                ConsensusError::Identity(format!(
                    "identity_file {} encode: {error}",
                    path.display()
                ))
            })?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    ConsensusError::Identity(format!(
                        "identity_file {} parent: {error}",
                        path.display()
                    ))
                })?;
            }
            std::fs::write(&path, bytes).map_err(|error| {
                ConsensusError::Identity(format!("identity_file {} write: {error}", path.display()))
            })?;
            Ok(keypair)
        }
        Err(error) => Err(ConsensusError::Identity(format!(
            "identity_file {} read: {error}",
            path.display()
        ))),
    }
}
