//! JSON-RPC server: trait, implementation shell, and bind helpers.

use std::sync::Arc;

use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObjectOwned;
use omega_claim_tx::ClaimTx;

use crate::rpc::types::{NodeState, SubmitOutcome};

/// JSON-RPC surface for a single LoganNet node.
///
/// Wire method names: `omega_submitClaim`, `omega_getState`.
#[rpc(server, namespace = "omega")]
pub trait OmegaRpc {
    /// Submits a single claim transaction.
    ///
    /// Returns the applied log index on success, or a structured error on
    /// rejection.
    #[method(name = "submitClaim")]
    async fn submit_claim(&self, claim: ClaimTx) -> Result<SubmitOutcome, ErrorObjectOwned>;

    /// Reads the node's current consensus and ledger state.
    ///
    /// Read-only.
    #[method(name = "getState")]
    async fn get_state(&self) -> Result<NodeState, ErrorObjectOwned>;
}

/// Concrete implementation of [`OmegaRpcServer`].
///
/// Carries shared handles to the openraft instance, the mock-ledger reader
/// pool, and the static peer list for leader-hint URL resolution.
#[derive(Clone)]
pub struct OmegaRpcImpl {
    /// Inner shared state.
    ///
    /// Cheap to clone; jsonrpsee clones per request.
    pub(crate) inner: Arc<OmegaRpcShared>,
}

/// Shared state behind the RPC impl.
///
/// Every field here is `Send + Sync` for the jsonrpsee server's `'static`
/// requirement.
pub(crate) struct OmegaRpcShared {
    /// Stable node id.
    pub(crate) node_id: u64,
    /// Raft handle.
    pub(crate) raft: openraft::Raft<omega_mock_ledger::OmegaRaftTypeConfig>,
    /// Ledger handle.
    pub(crate) ledger: Arc<omega_mock_ledger::MockLedger>,
    /// Static peers for leader-hint resolution.
    pub(crate) peers: Vec<crate::PeerConfig>,
    /// Apply deadline for client writes.
    pub(crate) apply_deadline: std::time::Duration,
}

#[async_trait]
impl OmegaRpcServer for OmegaRpcImpl {
    async fn submit_claim(&self, _claim: ClaimTx) -> Result<SubmitOutcome, ErrorObjectOwned> {
        Err(ErrorObjectOwned::owned(
            jsonrpsee::types::error::INTERNAL_ERROR_CODE,
            "submit_claim unimplemented",
            None::<()>,
        ))
    }

    async fn get_state(&self) -> Result<NodeState, ErrorObjectOwned> {
        Err(ErrorObjectOwned::owned(
            jsonrpsee::types::error::INTERNAL_ERROR_CODE,
            "get_state unimplemented",
            None::<()>,
        ))
    }
}
