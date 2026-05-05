//! JSON-RPC server: trait, implementation shell, and bind helpers.

use std::sync::Arc;

use jsonrpsee::core::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::types::ErrorObjectOwned;
use omega_claim_tx::ClaimTx;
use openraft::error::RaftError;

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
    /// Replicates a claim through raft, then returns the applied log index.
    ///
    /// # Soundness
    ///
    /// Preserved: every accepted claim enters the mock-ledger state machine
    /// through `Raft::client_write`, so followers apply the same command in
    /// log order. Closed attack: followers cannot bypass the leader by calling
    /// the writer actor directly through this RPC path. Fails on: openraft
    /// forwarding, timeout, or ledger rejection, each mapped to the fixed
    /// JSON-RPC/application result vocabulary.
    async fn submit_claim(&self, claim: ClaimTx) -> Result<SubmitOutcome, ErrorObjectOwned> {
        use crate::routing::{translate_client_write_error, translate_ledger_error};

        let cmd =
            omega_mock_ledger::LedgerCommand::apply_claim(claim).map_err(translate_ledger_error)?;
        let inner = self.inner.clone();

        let result = tokio::time::timeout(inner.apply_deadline, inner.raft.client_write(cmd)).await;

        let response = match result {
            Err(_elapsed) => {
                let ms = inner.apply_deadline.as_millis().min(u32::MAX as u128) as u32;
                return Err(crate::rpc::error::timeout(ms));
            }
            Ok(Ok(write)) => write,
            Ok(Err(RaftError::APIError(err))) => {
                return Err(translate_client_write_error(err, |id| {
                    inner
                        .peers
                        .iter()
                        .find(|p| p.node_id == id)
                        .map(|p| p.rpc_url.clone())
                }));
            }
            Ok(Err(RaftError::Fatal(err))) => {
                return Err(ErrorObjectOwned::owned(
                    jsonrpsee::types::error::INTERNAL_ERROR_CODE,
                    jsonrpsee::types::error::INTERNAL_ERROR_MSG,
                    Some(serde_json::json!({ "openraft": err.to_string() })),
                ));
            }
        };

        let applied_index = Some(response.log_id.index);
        match response.data {
            omega_mock_ledger::LedgerResponse { accepted: true, .. } => Ok(SubmitOutcome {
                accepted: true,
                applied_index,
                reject_reason: None,
            }),
            omega_mock_ledger::LedgerResponse {
                accepted: false,
                reject: Some(reject),
                ..
            } => Ok(SubmitOutcome {
                accepted: false,
                applied_index,
                reject_reason: Some(reject_reason(reject).into()),
            }),
            omega_mock_ledger::LedgerResponse {
                accepted: false,
                reject: None,
                ..
            } => Ok(SubmitOutcome {
                accepted: false,
                applied_index,
                reject_reason: Some("internal".into()),
            }),
        }
    }

    /// Returns the current raft role and point-in-time ledger counters.
    ///
    /// # Soundness
    ///
    /// Preserved: this method is read-only and never touches the writer actor
    /// or raft client-write path. Closed attack: state inspection cannot submit
    /// or reorder claims. Fails on: ledger reader errors, which are translated
    /// through the fixed ledger error mapper.
    async fn get_state(&self) -> Result<NodeState, ErrorObjectOwned> {
        use crate::rpc::types::{LogIdView, NodeRole};

        let inner = self.inner.clone();
        let metrics = inner.raft.metrics().borrow().clone();

        let role = match metrics.state {
            openraft::ServerState::Leader => NodeRole::Leader,
            openraft::ServerState::Follower => NodeRole::Follower,
            openraft::ServerState::Candidate => NodeRole::Candidate,
            openraft::ServerState::Learner => NodeRole::Learner,
            _ => NodeRole::Follower,
        };

        let last_log_id = metrics
            .last_log_index
            .zip(Some(metrics.current_term))
            .map(|(index, term)| LogIdView { term, index });

        let nullifier_count = inner
            .ledger
            .nullifier_count()
            .await
            .map_err(crate::routing::translate_ledger_error)?;
        let starstream_utxo_count = inner
            .ledger
            .starstream_utxo_count()
            .await
            .map_err(crate::routing::translate_ledger_error)?;

        Ok(NodeState {
            node_id: inner.node_id,
            role,
            leader_id: metrics.current_leader,
            last_log_id,
            applied_index: metrics.last_applied.map(|log_id| log_id.index).unwrap_or(0),
            nullifier_count,
            starstream_utxo_count,
        })
    }
}

fn reject_reason(reject: omega_mock_ledger::LedgerReject) -> &'static str {
    match reject {
        omega_mock_ledger::LedgerReject::Verify => "verify",
        omega_mock_ledger::LedgerReject::InvalidClaim => "invalid",
        omega_mock_ledger::LedgerReject::Replay => "replay",
        omega_mock_ledger::LedgerReject::WriterClosed
        | omega_mock_ledger::LedgerReject::Internal => "internal",
    }
}
