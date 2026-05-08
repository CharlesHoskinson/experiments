//! Node configuration types.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Static configuration consumed by [`crate::start`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Stable u64 node identifier; matches the openraft `NodeId`.
    pub node_id: u64,
    /// Path to the SQLite WAL directory; created if absent.
    pub data_dir: PathBuf,
    /// Libp2p multiaddr the node listens on.
    pub libp2p_listen: String,
    /// Static peer list; 2 entries for a 3-node cluster.
    #[serde(default)]
    pub peers: Vec<PeerConfig>,
    /// JSON-RPC HTTP bind + limits.
    pub rpc: RpcConfig,
    /// Cluster identifier; must match across all peers.
    pub cluster_id: String,
    /// Apply deadline; default 5s.
    #[serde(with = "humantime_serde", default = "default_apply_deadline")]
    pub apply_deadline: Duration,
}

fn default_apply_deadline() -> Duration {
    Duration::from_secs(5)
}

/// One peer's wire-level coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    /// Stable u64 node identifier of the peer.
    pub node_id: u64,
    /// Libp2p multiaddr to dial.
    pub libp2p_addr: String,
    /// Public RPC URL used in `-32000 NotLeader` hints.
    pub rpc_url: String,
}

impl std::str::FromStr for PeerConfig {
    type Err = crate::ConsensusError;

    /// Parses `<node_id>,<libp2p_addr>,<rpc_url>`.
    ///
    /// Used by the CLI `--peer` flag.
    ///
    /// # Errors
    ///
    /// [`ConsensusError::Config`](crate::ConsensusError::Config) if the input
    /// has fewer than 3 comma-separated fields or `node_id` does not parse as
    /// `u64`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(3, ',').collect();
        if parts.len() != 3 {
            return Err(crate::ConsensusError::Config(format!(
                "peer must be `<node_id>,<libp2p>,<rpc>`, got `{s}`"
            )));
        }
        let node_id: u64 = parts[0]
            .parse()
            .map_err(|e| crate::ConsensusError::Config(format!("peer node_id: {e}")))?;
        Ok(Self {
            node_id,
            libp2p_addr: parts[1].to_string(),
            rpc_url: parts[2].to_string(),
        })
    }
}

/// JSON-RPC HTTP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    /// HTTP bind address.
    pub bind: SocketAddr,
    /// Maximum batch length; default 25.
    #[serde(default = "default_max_batch")]
    pub max_batch: u16,
    /// Maximum request body bytes; default 1 MiB.
    #[serde(default = "default_max_request_bytes")]
    pub max_request_bytes: u32,
}

fn default_max_batch() -> u16 {
    25
}

fn default_max_request_bytes() -> u32 {
    1024 * 1024
}

impl NodeConfig {
    /// Convenience: single-node localhost cluster.
    ///
    /// Used in doctests and smoke fixtures. Real bring-ups should populate
    /// `peers` with at least 2 entries.
    ///
    /// # Errors
    ///
    /// Returns [`ConsensusError::Config`](crate::ConsensusError::Config) if
    /// `node_id` is 0 (openraft requires non-zero).
    pub fn single_node_localhost(node_id: u64) -> Result<Self, crate::ConsensusError> {
        use std::net::{Ipv4Addr, SocketAddr};

        if node_id == 0 {
            return Err(crate::ConsensusError::Config(
                "node_id must be non-zero".into(),
            ));
        }
        let port: u16 = (8000_u32
            + u32::try_from(node_id)
                .map_err(|_| crate::ConsensusError::Config("node_id exceeds u32".into()))?)
        .try_into()
        .map_err(|_| crate::ConsensusError::Config("rpc port exceeds u16".into()))?;
        Ok(Self {
            node_id,
            data_dir: std::env::temp_dir().join(format!("omega-toy-consensus-{node_id}")),
            libp2p_listen: format!("/ip4/127.0.0.1/tcp/{}", 4000 + node_id),
            peers: Vec::new(),
            rpc: RpcConfig {
                bind: SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
                max_batch: 25,
                max_request_bytes: 1024 * 1024,
            },
            cluster_id: "loganet-dev".into(),
            apply_deadline: Duration::from_secs(5),
        })
    }
}

#[cfg(test)]
mod tests;
