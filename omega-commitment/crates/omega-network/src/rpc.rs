//! CBOR Raft RPC envelope carried by libp2p request-response payloads.
//!
//! The envelope wraps openraft's three RPC pairs (`AppendEntries`,
//! `RequestVote`, `InstallSnapshot`) as a single tagged enum so the libp2p
//! request-response actor can ship one payload type per direction. CBOR is
//! produced via `ciborium` with two hard caps the receiver enforces before
//! deserialisation: a maximum envelope byte length and a maximum nesting
//! depth.

use omega_mock_ledger::OmegaRaftTypeConfig;
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum envelope size accepted by [`decode_cbor`] before deserialisation.
///
/// 16 MiB is sized to comfortably hold an `AppendEntriesRequest` carrying a
/// few thousand `LedgerCommand` entries. Inbound payloads larger than this
/// are rejected without allocating the buffer for ciborium, which closes the
/// `Vec::with_capacity(huge_advertised_len)` preallocation DoS surface.
pub const MAX_RAFT_RPC_BYTES: usize = 16 * 1024 * 1024;

/// Maximum nesting depth accepted by [`decode_cbor`].
///
/// CBOR's stack-recursive decoder happily walks deeply nested arrays/maps
/// until it overflows the OS thread stack. 64 levels of nesting is far
/// beyond anything openraft's RPC types or the snapshot frame enum require.
pub const MAX_CBOR_RECURSION: usize = 64;

/// Errors returned by the network codec and request-response adapter.
#[derive(Debug, Error)]
pub enum OmegaNetworkError {
    /// CBOR encoding or decoding failed. The wrapped string carries the
    /// underlying ciborium diagnostic; the structured ciborium error type is
    /// not exposed because it is generic over the reader's error.
    #[error("cbor codec error: {0}")]
    Codec(String),
    /// An inbound or outbound payload exceeded [`MAX_RAFT_RPC_BYTES`].
    #[error("cbor payload too large: {actual} > {max}")]
    Oversize {
        /// Observed payload byte length.
        actual: usize,
        /// Maximum byte length the codec accepts.
        max: usize,
    },
    /// The request-response actor is no longer accepting outbound requests.
    #[error("request-response actor is closed")]
    OutboundClosed,
    /// The request-response actor's outbound queue was full and the request
    /// was dropped to apply backpressure.
    #[error("request-response actor outbound queue full")]
    OutboundFull,
    /// The request-response actor dropped the reply channel.
    #[error("request-response actor dropped the reply channel")]
    ReplyDropped,
    /// The request-response round-trip exceeded the openraft `RPCOption`
    /// deadline.
    #[error("request-response round-trip timed out")]
    Timeout,
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
///
/// `#[serde(rename_all = "snake_case")]` aligns the wire variant tag with the
/// human-readable `variant_name()` strings so wire dumps and diagnostic
/// errors share one vocabulary.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
#[serde(rename_all = "snake_case")]
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
///
/// # Errors
///
/// - [`OmegaNetworkError::Codec`] — ciborium failed to encode the value.
///
/// # Examples
///
/// ```
/// use omega_network::rpc::encode_cbor;
/// let bytes = encode_cbor(&"hello").unwrap();
/// assert!(!bytes.is_empty());
/// ```
pub fn encode_cbor<T>(value: &T) -> Result<Vec<u8>, OmegaNetworkError>
where
    T: Serialize,
{
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes)
        .map_err(|error| OmegaNetworkError::Codec(error.to_string()))?;
    Ok(bytes)
}

/// Decodes a CBOR value, rejecting payloads larger than
/// [`MAX_RAFT_RPC_BYTES`] or nested deeper than [`MAX_CBOR_RECURSION`].
///
/// # Errors
///
/// - [`OmegaNetworkError::Oversize`] — payload exceeded [`MAX_RAFT_RPC_BYTES`].
///   The check runs *before* any ciborium allocation.
/// - [`OmegaNetworkError::Codec`] — ciborium failed to decode (malformed
///   CBOR, type mismatch, recursion limit hit).
///
/// # Soundness
///
/// The envelope-byte check and recursion-limit decoder together close two
/// classes of byzantine-peer DoS: (a) a peer claiming a giant array length
/// forcing `Vec::with_capacity` preallocation, and (b) a peer sending a
/// deeply-nested CBOR document overflowing the decoder's stack. Both bounds
/// must be enforced before the call site dispatches the decoded value to
/// openraft.
///
/// # Examples
///
/// ```
/// use omega_network::rpc::{decode_cbor, encode_cbor};
/// let bytes = encode_cbor(&42_u32).unwrap();
/// let back: u32 = decode_cbor(&bytes).unwrap();
/// assert_eq!(back, 42);
/// ```
pub fn decode_cbor<T>(bytes: &[u8]) -> Result<T, OmegaNetworkError>
where
    T: DeserializeOwned,
{
    if bytes.len() > MAX_RAFT_RPC_BYTES {
        return Err(OmegaNetworkError::Oversize {
            actual: bytes.len(),
            max: MAX_RAFT_RPC_BYTES,
        });
    }
    ciborium::de::from_reader_with_recursion_limit(bytes, MAX_CBOR_RECURSION)
        .map_err(|error| OmegaNetworkError::Codec(error.to_string()))
}
