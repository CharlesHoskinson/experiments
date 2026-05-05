//! Peer discovery and static-peer configuration.

use std::str::FromStr;

use sha3::{Digest, Sha3_256};
use thiserror::Error;

/// Errors returned while parsing discovery configuration.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DiscoveryError {
    /// Static peer strings must use `node_id=multiaddr`.
    #[error("static peer must use node_id=multiaddr")]
    MissingSeparator,
    /// The node id part of a static peer could not be parsed as `u64`.
    #[error("invalid static peer node id: {0}")]
    InvalidNodeId(String),
    /// The multiaddr part of a static peer was empty.
    #[error("static peer multiaddr is empty")]
    EmptyMultiaddr,
}

/// A configured static peer address used when mDNS is unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerAddress {
    /// Openraft node id for the peer.
    pub node_id: u64,
    /// Libp2p multiaddr string advertised for that peer.
    pub multiaddr: String,
}

impl FromStr for PeerAddress {
    type Err = DiscoveryError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (node_id, multiaddr) = value
            .split_once('=')
            .ok_or(DiscoveryError::MissingSeparator)?;
        let node_id = node_id
            .parse::<u64>()
            .map_err(|_| DiscoveryError::InvalidNodeId(node_id.to_string()))?;
        if multiaddr.is_empty() {
            return Err(DiscoveryError::EmptyMultiaddr);
        }
        Ok(Self {
            node_id,
            multiaddr: multiaddr.to_string(),
        })
    }
}

/// mDNS mode for a local harness node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MdnsMode {
    /// mDNS discovery is disabled; callers must provide static peers.
    Disabled,
    /// mDNS discovery is enabled under the supplied service name.
    Enabled {
        /// Salted service name used on the LAN.
        service_name: String,
    },
}

/// Discovery configuration consumed by the node runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryConfig {
    /// mDNS mode for the node.
    pub mdns: MdnsMode,
    /// Static peers supplied through the `--peers` fallback path.
    pub static_peers: Vec<PeerAddress>,
}

impl DiscoveryConfig {
    /// Builds a discovery configuration.
    pub fn new(
        genesis: &[u8],
        installation_salt: &[u8],
        mdns_disabled: bool,
        static_peers: Vec<PeerAddress>,
    ) -> Self {
        let mdns = if mdns_disabled {
            MdnsMode::Disabled
        } else {
            MdnsMode::Enabled {
                service_name: mdns_service_name(genesis, installation_salt),
            }
        };
        Self { mdns, static_peers }
    }
}

/// Builds the salted mDNS service name for this harness installation.
pub fn mdns_service_name(genesis: &[u8], installation_salt: &[u8]) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(b"omega-network:mdns-service:v1");
    hasher.update((genesis.len() as u64).to_be_bytes());
    hasher.update(genesis);
    hasher.update((installation_salt.len() as u64).to_be_bytes());
    hasher.update(installation_salt);
    let digest = hasher.finalize();
    let prefix = hex::encode(&digest[..8]);
    format!("_omega-experiment-{prefix}._udp.local")
}
