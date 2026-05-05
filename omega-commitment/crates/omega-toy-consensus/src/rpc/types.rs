//! Wire types for the JSON-RPC surface.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Outcome of a single `omega_submitClaim` call.
///
/// `accepted = true` means `applied_index = Some(idx)` and
/// `reject_reason = None`. `accepted = false` means `applied_index = None`
/// and `reject_reason` names the rejection class.
#[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Clone)]
pub struct SubmitOutcome {
    /// Whether the claim was applied to the state machine.
    pub accepted: bool,
    /// Raft log index at which the apply occurred, when `accepted`.
    pub applied_index: Option<u64>,
    /// Reject reason, when `!accepted`. One of `"verify"`, `"invalid"`,
    /// `"replay"`.
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
