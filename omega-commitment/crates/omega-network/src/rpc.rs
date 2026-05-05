//! CBOR Raft RPC envelope carried by libp2p request-response payloads.

use omega_mock_ledger::OmegaRaftTypeConfig;
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned by the network codec and request-response adapter.
#[derive(Debug, Error)]
pub enum OmegaNetworkError {
    /// CBOR encoding or decoding failed.
    #[error("cbor codec error: {0}")]
    Codec(String),
    /// The request-response actor is no longer accepting outbound requests.
    #[error("request-response actor is closed")]
    OutboundClosed,
    /// The request-response actor dropped the reply channel.
    #[error("request-response actor dropped the reply channel")]
    ReplyDropped,
    /// A response variant did not match the request variant.
    #[error("wrong raft response variant: expected {expected}, got {actual}")]
    WrongResponse {
        /// Expected response variant.
        expected: &'static str,
        /// Actual response variant.
        actual: &'static str,
    },
}

/// Raft RPC request variants sent as CBOR request-response payloads.
#[derive(Debug, Serialize, Deserialize)]
pub enum RaftRpcRequest {
    /// Openraft AppendEntries request.
    AppendEntries(Box<AppendEntriesRequest<OmegaRaftTypeConfig>>),
    /// Openraft RequestVote request.
    Vote(VoteRequest<u64>),
    /// Openraft InstallSnapshot request.
    InstallSnapshot(Box<InstallSnapshotRequest<OmegaRaftTypeConfig>>),
}

impl RaftRpcRequest {
    /// Returns the stable variant name for diagnostics.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::AppendEntries(_) => "append_entries",
            Self::Vote(_) => "vote",
            Self::InstallSnapshot(_) => "install_snapshot",
        }
    }
}

/// Raft RPC response variants sent as CBOR request-response payloads.
#[derive(Debug, Serialize, Deserialize)]
pub enum RaftRpcResponse {
    /// Openraft AppendEntries response.
    AppendEntries(AppendEntriesResponse<u64>),
    /// Openraft RequestVote response.
    Vote(VoteResponse<u64>),
    /// Openraft InstallSnapshot response.
    InstallSnapshot(InstallSnapshotResponse<u64>),
}

impl RaftRpcResponse {
    /// Returns the stable variant name for diagnostics.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::AppendEntries(_) => "append_entries",
            Self::Vote(_) => "vote",
            Self::InstallSnapshot(_) => "install_snapshot",
        }
    }
}

/// Encodes a serializable value as CBOR.
pub fn encode_cbor<T>(value: &T) -> Result<Vec<u8>, OmegaNetworkError>
where
    T: Serialize,
{
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes)
        .map_err(|error| OmegaNetworkError::Codec(error.to_string()))?;
    Ok(bytes)
}

/// Decodes a CBOR value.
pub fn decode_cbor<T>(bytes: &[u8]) -> Result<T, OmegaNetworkError>
where
    T: DeserializeOwned,
{
    ciborium::from_reader(bytes).map_err(|error| OmegaNetworkError::Codec(error.to_string()))
}
