//! openraft to JSON-RPC error translation.
//!
//! This module is the single source of truth for mapping
//! [`openraft::error::ClientWriteError`] and [`omega_mock_ledger::LedgerError`]
//! into the `-32000..-32005` JSON-RPC error code space.

use jsonrpsee::types::ErrorObjectOwned;
use omega_mock_ledger::{LedgerError, OmegaRaftTypeConfig};
use openraft::error::{ClientWriteError, ForwardToLeader};

use crate::rpc::error;

/// Translates an openraft `ClientWriteError` into a JSON-RPC `ErrorObjectOwned`.
///
/// # Soundness
///
/// Preserves: the openraft client-write rejection class remains visible at the
/// JSON-RPC boundary. `ForwardToLeader` becomes `-32000` with a leader hint,
/// while membership-change errors collapse to `-32603 internal error` because
/// Group 1 has static membership and exposes no membership-change RPCs.
///
/// Closes: a follower response cannot be mistaken for a local writer failure.
///
/// Fails on: the hinted leader is not revalidated after the response leaves
/// this node, so clients must tolerate stale or absent hints.
pub fn translate_client_write_error(
    err: ClientWriteError<u64, openraft::BasicNode>,
    resolve_leader_url: impl FnOnce(u64) -> Option<String>,
) -> ErrorObjectOwned {
    match err {
        ClientWriteError::ForwardToLeader(ForwardToLeader {
            leader_id,
            leader_node: _,
        }) => {
            let leader_rpc_url = leader_id.and_then(resolve_leader_url);
            error::not_leader(leader_id, leader_rpc_url)
        }
        other => ErrorObjectOwned::owned(
            jsonrpsee::types::error::INTERNAL_ERROR_CODE,
            jsonrpsee::types::error::INTERNAL_ERROR_MSG,
            Some(serde_json::json!({ "openraft": other.to_string() })),
        ),
    }
}

/// Translates a `LedgerError` from `apply_to_state_machine` into JSON-RPC.
///
/// # Soundness
///
/// Preserves: the mock-ledger rejection class remains visible at the JSON-RPC
/// boundary. Verifier failures become `-32001`, invalid claim shape becomes
/// `-32002`, replay becomes `-32003` with nullifier coordinates, and
/// writer-channel loss becomes retryable `-32004`.
///
/// Closes: replay and malformed-claim failures cannot be reported as retryable
/// infrastructure failures.
///
/// Fails on: raw SQLite and I/O details are intentionally not exposed to
/// clients; those collapse to `-32603`.
pub fn translate_ledger_error(err: LedgerError) -> ErrorObjectOwned {
    match err {
        LedgerError::Verify(detail) => error::verify(detail.to_string()),
        LedgerError::InvalidClaim(detail) => error::invalid_claim(detail.to_string()),
        LedgerError::Replay {
            sub_tree_id,
            leaf_index,
        } => error::replay(u32::from(sub_tree_id), leaf_index),
        LedgerError::WriterClosed | LedgerError::WriterReplyCanceled => error::writer_closed(),
        other => ErrorObjectOwned::owned(
            jsonrpsee::types::error::INTERNAL_ERROR_CODE,
            jsonrpsee::types::error::INTERNAL_ERROR_MSG,
            Some(serde_json::json!({ "ledger": other.to_string() })),
        ),
    }
}

/// Type alias to keep `OmegaRaftTypeConfig` paths short in callers.
#[allow(dead_code)]
pub(crate) type RaftCfg = OmegaRaftTypeConfig;
