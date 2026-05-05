//! Node lifecycle.

use crate::{ConsensusError, NodeConfig};

/// Live LoganNet node.
///
/// Owns the openraft instance, the mock-ledger writer handle, the libp2p
/// network, and the JSON-RPC server.
pub struct Node {
    // Fields populated in Task 7.
}

/// Handle for graceful shutdown of a running [`Node`].
pub struct NodeHandle {
    // Fields populated in Task 8.
}

impl Node {
    /// Brings the node up.
    ///
    /// See [`crate::start`] for the public entry point.
    ///
    /// # Errors
    ///
    /// See [`ConsensusError`].
    pub async fn start(_config: NodeConfig) -> Result<NodeHandle, ConsensusError> {
        Err(ConsensusError::Config("Node::start unimplemented".into()))
    }
}

impl NodeHandle {
    /// Initiates graceful shutdown.
    ///
    /// Drains in-flight RPC submits, terminates the writer actor, releases the
    /// libp2p socket, then awaits the runtime task.
    ///
    /// # Errors
    ///
    /// - [`ConsensusError::ShutdownJoin`] - runtime task panicked or was
    ///   cancelled abnormally.
    /// - [`ConsensusError::Raft`] - openraft shutdown returned an error.
    pub async fn shutdown(self) -> Result<(), ConsensusError> {
        Ok(())
    }
}
