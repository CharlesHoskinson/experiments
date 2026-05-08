//! Wire types for the JSON-RPC surface.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Outcome of a single `omega_submitClaim` call.
///
/// `accepted = true` ⇒ the claim was applied to the state machine; the
/// nullifier and Starstream UTxO are durable; `applied_index` carries the
/// raft log index at which the apply committed; `reject_reason = None`.
///
/// `accepted = false` ⇒ raft committed the entry but the state machine
/// rejected the apply (the log index advanced, but no ledger mutation
/// happened); `applied_index` carries the index of the committed-but-rejected
/// entry (useful for client-side deduping and ordering); `reject_reason`
/// names the class as one of `"verify"` / `"invalid"` / `"replay"` /
/// `"internal"`.
#[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone)]
pub struct SubmitOutcome {
    /// Whether the claim was applied to the state machine.
    pub accepted: bool,
    /// Raft log index at which the apply-or-reject decision committed.
    /// Always `Some(_)` for any outcome that returns this struct (only
    /// pre-commit failures like `−32000 NotLeader` / `−32004 WriterClosed`
    /// / `−32005 Timeout` come back as JSON-RPC errors instead).
    pub applied_index: Option<u64>,
    /// Reject reason, when `!accepted`. One of `"verify"`, `"invalid"`,
    /// `"replay"`, `"internal"`.
    pub reject_reason: Option<String>,
}

/// Read-only view of node + ledger state.
#[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone)]
pub struct NodeState {
    /// Stable u64 node identifier.
    pub node_id: u64,
    /// This node's current openraft role.
    pub role: NodeRole,
    /// Leader's node id, when known.
    pub leader_id: Option<u64>,
    /// Last log id committed to local storage, when present.
    pub last_log_id: Option<LogIdView>,
    /// Last log index applied to the state machine.
    pub applied_index: u64,
    /// Number of nullifiers in the ledger.
    pub nullifier_count: u64,
    /// Number of Starstream UTxOs in the ledger.
    pub starstream_utxo_count: u64,
}

/// JSON-friendly mirror of openraft's `RaftState::role`.
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    /// Currently the leader.
    Leader,
    /// Following a leader.
    Follower,
    /// Election in progress.
    Candidate,
    /// Read-only, non-voting member.
    Learner,
}

/// JSON-friendly mirror of openraft's `LogId`.
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy, PartialEq, Eq)]
pub struct LogIdView {
    /// Raft term of the entry.
    pub term: u64,
    /// Log index of the entry.
    pub index: u64,
}

#[cfg(test)]
mod tests;
