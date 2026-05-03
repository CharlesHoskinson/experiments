//! omega-commitment-bundle: assembles the canonical Ω-Commitment tuple
//! `(blake3_bundle_root, sha3_bundle_root)` from the seven sub-tree
//! commitments per the dual-hash decision (2026-05-01).
//!
//! As of the 2026-05-03 audit reframing, the SHA3 bundle root is a
//! drift-detection signal computed over the same v1 Blake3 leaf
//! hashes — NOT a binding-agility hedge against a Blake3 break.
//! See `bundle::verify` and the module docs in `bundle.rs` for the
//! per-sub-tree `item_count` check that closes A1/F003. The truly-
//! independent SHA3 tree (separate per-leaf SHA3 hashing) is tracked
//! as a v2.0 follow-up.
//!
//! # Public errors
//!
//! Public APIs in this crate return [`BundleError`], a `thiserror`-
//! derived enum. Internal helpers continue to use `anyhow::Error`
//! during recompute, but they cross the public boundary as either a
//! typed variant ([`BundleError::Io`], [`BundleError::Json`],
//! [`BundleError::Recompute`], [`BundleError::Mismatch`],
//! [`BundleError::DuplicateSemanticKey`], [`BundleError::SchemaVersionMismatch`])
//! or as the catch-all [`BundleError::Other`]. This closes A5/F001 of
//! the 2026-05-03 audit.

pub mod bundle;
pub mod recompute;
pub mod sub_tree_id;

use std::path::PathBuf;
use thiserror::Error;

/// Public error type for `omega-commitment-bundle`.
///
/// Variants are stable (added under SemVer minor); never reorder or
/// remove. Downstream consumers may match on these variants to drive
/// retries, exit codes, or user-facing messages without parsing the
/// stringified `Display` impl.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BundleError {
    /// Filesystem failure while reading a sub-tree input file or
    /// writing the bundle output.
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// JSON parse / serialize failure for either a sub-tree input or
    /// the published bundle record.
    #[error("json error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    /// A sub-tree's recompute step rejected the input (bad shape,
    /// duplicate canonical payload, encoder overflow, ...).
    #[error("cannot recompute sub-tree {sub_tree}: {source}")]
    Recompute {
        sub_tree: String,
        #[source]
        source: anyhow::Error,
    },

    /// Verification mismatch between a published bundle and the
    /// re-computed bundle. `field` is one of `blake3_bundle_root`,
    /// `sha3_bundle_root`, `schema_version`, `sub_trees.len`, or a
    /// per-sub-tree label (e.g. `item_count[utxo]`).
    #[error(
        "bundle verification mismatch on {field}: published={published}, recomputed={recomputed}"
    )]
    Mismatch {
        field: String,
        published: String,
        recomputed: String,
    },

    /// A sub-tree's canonical-index sequence is non-monotonic (closes
    /// A1/F005). Returned by the per-sub-tree root builders when two
    /// distinct payloads collide on the same canonical position OR
    /// when the same canonical key appears twice.
    #[error("duplicate semantic key in sub-tree {sub_tree_id} at index {index}")]
    DuplicateSemanticKey { sub_tree_id: u8, index: u64 },

    /// The bundle on disk was written under a schema version this
    /// build does not understand.
    #[error("bundle schema version mismatch: published={published}, current={current}")]
    SchemaVersionMismatch { published: u32, current: u32 },

    /// Catch-all for the long tail of internal errors that have not
    /// yet been promoted to a typed variant. Wrapping `anyhow::Error`
    /// preserves the recompute helpers' existing `?`-propagation
    /// without forcing every internal call site to refactor.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl BundleError {
    /// Convenience constructor for I/O errors that carry the offending
    /// path. Preserved separately from the `From<std::io::Error>` impl
    /// so that callers always supply the path context.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Convenience constructor for JSON errors that carry the offending
    /// path.
    pub fn json(path: impl Into<PathBuf>, source: serde_json::Error) -> Self {
        Self::Json {
            path: path.into(),
            source,
        }
    }
}
