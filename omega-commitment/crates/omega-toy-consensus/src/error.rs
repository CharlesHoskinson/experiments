//! Consensus error type for [`crate::start`] and [`crate::Node`].

use thiserror::Error;

/// Errors produced during node bring-up, runtime, and shutdown.
#[derive(Debug, Error)]
pub enum ConsensusError {
    /// SQLite open / schema init / writer-actor start failed.
    #[error("storage: {0}")]
    Storage(#[from] omega_mock_ledger::LedgerError),

    /// libp2p bind, dial, or RPC factory init failed.
    #[error("network: {0}")]
    Network(#[from] omega_network::rpc::OmegaNetworkError),

    /// JSON-RPC HTTP server failed to bind the configured address.
    #[error("rpc bind on {addr}: {source}")]
    RpcBind {
        /// The address that failed to bind.
        addr: std::net::SocketAddr,
        /// Underlying TCP bind error.
        source: std::io::Error,
    },

    /// openraft initialisation, run-loop, or shutdown failed.
    #[error("raft: {0}")]
    Raft(String),

    /// Configuration parse / validation failed before bring-up.
    #[error("config: {0}")]
    Config(String),

    /// Identity-file read, write, generate, or decode failed.
    ///
    /// Distinct from [`Config`](Self::Config) because operators reading
    /// the error need to distinguish "your config is malformed" from
    /// "the libp2p identity keypair on disk could not be loaded or
    /// created" (filesystem permission, disk full, decode error).
    #[error("identity: {0}")]
    Identity(String),

    /// Shutdown was requested but the runtime task did not join cleanly.
    #[error("shutdown join: {0}")]
    ShutdownJoin(String),
}
