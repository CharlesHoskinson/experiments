//! JSON-RPC server.

pub mod types {
    //! Wire types for the JSON-RPC surface.

    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    /// Outcome of a single `omega_submitClaim` call.
    #[derive(Debug, Serialize, Deserialize, JsonSchema)]
    pub struct SubmitOutcome {
        /// Whether the claim was applied to the state machine.
        pub accepted: bool,
        /// Raft log index at which the apply occurred, when `accepted`.
        pub applied_index: Option<u64>,
        /// Reject reason, when `!accepted`. One of: `verify`, `invalid`,
        /// `replay`.
        pub reject_reason: Option<String>,
    }

    /// Read-only view of node + ledger state.
    #[derive(Debug, Serialize, Deserialize, JsonSchema)]
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
    #[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy)]
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
    #[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Copy)]
    pub struct LogIdView {
        /// Raft term of the entry.
        pub term: u64,
        /// Log index of the entry.
        pub index: u64,
    }
}
