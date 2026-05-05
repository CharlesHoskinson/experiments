//! Node configuration types.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

/// Static configuration consumed by [`crate::start`].
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// Stable u64 node identifier; matches the openraft `NodeId`.
    pub node_id: u64,
    /// Path to the SQLite WAL directory; created if absent.
    pub data_dir: PathBuf,
    /// Libp2p multiaddr the node listens on.
    pub libp2p_listen: String,
    /// Static peer list; 2 entries for a 3-node cluster.
    pub peers: Vec<PeerConfig>,
    /// JSON-RPC HTTP bind + limits.
    pub rpc: RpcConfig,
    /// Cluster identifier; must match across all peers.
    pub cluster_id: String,
    /// Apply deadline; default 5s.
    pub apply_deadline: Duration,
}

/// One peer's wire-level coordinates.
#[derive(Debug, Clone)]
pub struct PeerConfig {
    /// Stable u64 node identifier of the peer.
    pub node_id: u64,
    /// Libp2p multiaddr to dial.
    pub libp2p_addr: String,
    /// Public RPC URL used in `-32000 NotLeader` hints.
    pub rpc_url: String,
}

/// JSON-RPC HTTP server configuration.
#[derive(Debug, Clone)]
pub struct RpcConfig {
    /// HTTP bind address.
    pub bind: SocketAddr,
    /// Maximum batch length; default 25.
    pub max_batch: u16,
    /// Maximum request body bytes; default 1 MiB.
    pub max_request_bytes: u32,
}

impl NodeConfig {
    /// Convenience: single-node localhost cluster.
    ///
    /// Real bring-ups should populate `peers` with at least 2 entries.
    ///
    /// # Errors
    ///
    /// Returns [`ConsensusError::Config`](crate::ConsensusError::Config) if
    /// `node_id` is 0 (openraft requires non-zero).
    pub fn single_node_localhost(node_id: u64) -> Result<Self, crate::ConsensusError> {
        if node_id == 0 {
            return Err(crate::ConsensusError::Config(
                "node_id must be non-zero".into(),
            ));
        }
        Ok(Self {
            node_id,
            data_dir: std::env::temp_dir().join(format!("omega-toy-consensus-{node_id}")),
            libp2p_listen: format!("/ip4/127.0.0.1/tcp/{}", 4000 + node_id),
            peers: Vec::new(),
            rpc: RpcConfig {
                bind: format!("127.0.0.1:{}", 8000 + node_id)
                    .parse()
                    .unwrap(),
                max_batch: 25,
                max_request_bytes: 1024 * 1024,
            },
            cluster_id: "loganet-dev".into(),
            apply_deadline: Duration::from_secs(5),
        })
    }
}
