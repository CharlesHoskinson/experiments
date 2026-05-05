//! JSON-RPC application error code constants and constructors.
//!
//! See spec `docs/superpowers/specs/2026-05-05-omega-toy-consensus-design.md`
//! section "Error code map (JSON-RPC application range)" for the contract.

use jsonrpsee::types::ErrorObjectOwned;
use serde::Serialize;

/// `-32000` - non-leader; `data` carries leader hint.
pub const CODE_NOT_LEADER: i32 = -32000;
/// `-32001` - proof verification failed.
pub const CODE_VERIFY: i32 = -32001;
/// `-32002` - CBOR decode / structural failure.
pub const CODE_INVALID_CLAIM: i32 = -32002;
/// `-32003` - nullifier already present.
pub const CODE_REPLAY: i32 = -32003;
/// `-32004` - writer actor unavailable (transient).
pub const CODE_WRITER_CLOSED: i32 = -32004;
/// `-32005` - apply did not complete in deadline.
pub const CODE_TIMEOUT: i32 = -32005;

/// Hint sent in `data` for `-32000 NotLeader`.
#[derive(Debug, Serialize)]
pub struct NotLeaderHint {
    /// Leader's u64 id, when openraft knows it.
    pub leader_id: Option<u64>,
    /// Public RPC URL of the leader, when this node knows it.
    pub leader_rpc_url: Option<String>,
}

/// Builds a `-32000 NotLeader` error.
///
/// # Soundness
///
/// The hint preserves stateless leader forwarding: this server does not proxy
/// writes and only reports the leader id and RPC URL it currently knows. This
/// closes the stale-proxy class where a follower silently forwards a client
/// write after leadership has moved. It does not prove the hinted URL is still
/// leader by the time a client retries; clients must treat absent or stale
/// hints as "leader unknown" and retry against any peer.
pub fn not_leader(leader_id: Option<u64>, leader_rpc_url: Option<String>) -> ErrorObjectOwned {
    let hint = NotLeaderHint {
        leader_id,
        leader_rpc_url,
    };
    ErrorObjectOwned::owned(
        CODE_NOT_LEADER,
        "not leader",
        Some(serde_json::to_value(&hint).expect("hint serialises")),
    )
}

/// Builds a `-32001 Verify` error.
pub fn verify(detail: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        CODE_VERIFY,
        "proof verification failed",
        Some(serde_json::json!({ "verify_error": detail.into() })),
    )
}

/// Builds a `-32002 InvalidClaim` error.
pub fn invalid_claim(detail: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        CODE_INVALID_CLAIM,
        "invalid claim",
        Some(serde_json::json!({ "detail": detail.into() })),
    )
}

/// Builds a `-32003 Replay` error.
pub fn replay(sub_tree_id: u32, leaf_index: u64) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        CODE_REPLAY,
        "claim replays an existing nullifier",
        Some(serde_json::json!({
            "sub_tree_id": sub_tree_id,
            "leaf_index": leaf_index,
        })),
    )
}

/// Builds a `-32004 WriterClosed` error.
pub fn writer_closed() -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        CODE_WRITER_CLOSED,
        "writer actor unavailable",
        Some(serde_json::json!({ "retryable": true })),
    )
}

/// Builds a `-32005 Timeout` error.
pub fn timeout(deadline_ms: u32) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        CODE_TIMEOUT,
        "apply deadline elapsed",
        Some(serde_json::json!({ "deadline_ms": deadline_ms })),
    )
}
