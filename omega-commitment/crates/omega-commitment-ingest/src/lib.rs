//! omega-commitment-ingest: transforms real Cardano data (CBOR
//! LedgerState snapshots, chain history) into the per-sub-tree JSON
//! formats consumed by `omega-commitment commit`.
//!
//! v0.9.x ships UTXO + the four LedgerState-derivable sub-trees
//! (token-policy, script, stake, governance) against the v0.9.x
//! synthetic CBOR fixture corpus. The v1.0 mainnet ingestion path —
//! real cardano-cli ledger-state JSON + the omega-utxo-snapshot LSQ
//! CBOR file — is implemented per Task 4 of the v1.0 plan
//! (`docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`).
//! Header and tx-index ingestion is documented as future work
//! requiring a chain-follower (v1.1).
//!
//! # Public errors
//!
//! Public ingest APIs return [`IngestError`], a `thiserror`-derived
//! enum. Variants enumerate the failure modes a downstream consumer
//! might want to branch on: malformed CBOR ([`IngestError::Cbor`]),
//! malformed JSON ([`IngestError::Json`]), schema-violating input
//! ([`IngestError::Schema`]), truncated input ([`IngestError::Truncated`]),
//! trailing-byte garbage ([`IngestError::Trailing`]), non-canonical
//! map ordering ([`IngestError::NonCanonical`]), and the catch-all
//! [`IngestError::Other`] which preserves the existing
//! `anyhow::Error` long tail. Closes A5/F002 of the 2026-05-03 audit.

pub mod cbor;
pub mod governance;
pub mod script;
pub mod stake;
pub mod token_policy;
pub mod utxo;

use thiserror::Error;

/// Public error type for `omega-commitment-ingest`.
///
/// Variants are stable under SemVer minor; never reorder or remove.
/// Downstream consumers may match on these variants to drive retries,
/// exit codes, or user-facing messages without parsing the
/// stringified `Display` impl.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IngestError {
    /// Malformed CBOR at the byte level: bad major type, length
    /// overflow, indefinite-length where definite was required, etc.
    /// `context` describes the position or field being decoded so
    /// operators can locate the offending byte without re-running
    /// with a debugger.
    #[error("malformed CBOR ({context}): {source}")]
    Cbor {
        context: String,
        #[source]
        source: pallas_codec::minicbor::decode::Error,
    },

    /// Malformed JSON for an ingest input or output document.
    #[error("malformed JSON ({context}): {source}")]
    Json {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    /// CBOR or JSON parsed cleanly but the resulting structure
    /// violates the ingest schema (wrong arity, missing required
    /// field, value out of range, missing AccountState pots, etc.).
    #[error("schema violation ({context}): {message}")]
    Schema { context: String, message: String },

    /// Input ended mid-record. Distinct from [`Self::Cbor`] so that
    /// retry logic can distinguish "stream cut short" from "stream
    /// has malformed payload".
    #[error("input truncated ({context})")]
    Truncated { context: String },

    /// Decoder finished its expected reads but trailing bytes remain
    /// in the input buffer. Closes A2/F002 (B2): trailing garbage
    /// must never be silently accepted because two byte-different
    /// inputs would then hash to the same omega leaf.
    #[error("trailing bytes after decode ({context}): {trailing_byte_count} byte(s) of garbage")]
    Trailing {
        context: String,
        trailing_byte_count: usize,
    },

    /// Map-keyed CBOR structure (multi-asset bundle, governance facts,
    /// ...) violated the canonical ascending-key, no-duplicates
    /// requirement. Two byte-different inputs would otherwise hash to
    /// the same omega leaf.
    #[error("non-canonical map ordering ({context}): {message}")]
    NonCanonical { context: String, message: String },

    /// Catch-all for the long tail of internal failures that have not
    /// yet been promoted to a typed variant. Wrapping `anyhow::Error`
    /// preserves the existing per-module `?`-propagation without
    /// forcing every helper to refactor at once.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl IngestError {
    /// Convenience constructor for schema violations.
    pub fn schema(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Schema {
            context: context.into(),
            message: message.into(),
        }
    }

    /// Convenience constructor for non-canonical map errors.
    pub fn non_canonical(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self::NonCanonical {
            context: context.into(),
            message: message.into(),
        }
    }

    /// Convenience constructor for truncated-input errors.
    pub fn truncated(context: impl Into<String>) -> Self {
        Self::Truncated {
            context: context.into(),
        }
    }

    /// Convenience constructor for trailing-byte errors.
    pub fn trailing(context: impl Into<String>, trailing_byte_count: usize) -> Self {
        Self::Trailing {
            context: context.into(),
            trailing_byte_count,
        }
    }
}
