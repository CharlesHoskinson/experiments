//! SQLite-backed mock ledger for the proof experiment harness.
//!
//! # Overview
//!
//! `omega-mock-ledger` persists accepted claim envelopes as nullifier rows and
//! Starstream UTxO rows while exposing the storage traits expected by openraft
//! 0.9. It is the local state-machine layer that later `omega-toy-consensus`
//! nodes will replicate.
//!
//! # Design context
//!
//! - OpenSpec change: [`add-proof-experiment-harness`][1].
//! - Spec scenarios: [`Mock ledger`][2].
//! - PR-4 review record: [`PR-4-REVIEW.md`][3].
//!
//! [1]: ../../../openspec/changes/add-proof-experiment-harness/
//! [2]: ../../../openspec/changes/add-proof-experiment-harness/specs/proof-harness/spec.md
//! [3]: ../../../openspec/changes/add-proof-experiment-harness/PR-4-REVIEW.md
//!
//! # Tier of trust
//!
//! Soundness-bearing state handling. This crate does not prove Merkle
//! membership itself; it decides whether a verified claim is allowed to mutate
//! durable state. The apply path, replay check, SQLite schema, and writer actor
//! are therefore part of the protocol boundary.
//!
//! # v0.1 limitations
//!
//! - Consensus-level leader-continuity and restart-quorum tests land with
//!   `omega-toy-consensus`; this crate only exercises the storage side.
//! - Snapshot restore currently copies tables from an attached snapshot
//!   database. The exact drop-live-DB-and-rename path is deferred until the
//!   reader-pool teardown story is explicit on Windows.
//! - Cardano phase-1 validation is feature-gated and compiles, but real
//!   Conway-era accept/reject fixtures are still open.
//!
//! # Writer actor convention
//!
//! State-changing operations use a writer actor: one dedicated OS thread owns
//! the `rusqlite::Connection`, receives `WriteCmd`s over a Tokio mpsc channel,
//! and replies over a oneshot. This keeps synchronous SQLite writes out of the
//! async runtime and avoids a per-call `spawn_blocking` herd contending for the
//! same WAL writer. Reader queries use short-lived r2d2 connections and run in
//! `spawn_blocking` when exposed through async methods.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::missing_crate_level_docs)]
// `openraft::StorageError<u64>` is 224 bytes; the `result_large_err` lint
// (Rust 1.95+) flags every `Result<_, openraft::StorageError<_>>` site.
// The error size is fixed by openraft's API; boxing it would change the
// trait-impl shape openraft expects. Allow the lint crate-locally until
// openraft 0.10+ ships a boxed-error variant.
#![allow(clippy::result_large_err)]

#[cfg(feature = "cardano-tx-validation")]
pub mod cardano;
/// SQLite schema, PRAGMA setup, and schema-inspection helpers.
pub mod schema;
mod storage;
mod writer;

use std::path::{Path, PathBuf};
use std::time::Duration;

use omega_claim_prover::OmegaCommitment;
use omega_claim_tx::ClaimTx;
use omega_claim_verifier::VerifyError;
use omega_commitment_core::hash::Hash;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use thiserror::Error;
use tokio::task::JoinError;

pub use schema::PragmaSnapshot;
use writer::WriterHandle;

type ReaderPool = Pool<SqliteConnectionManager>;

pub use storage::{LedgerCommand, LedgerResponse, MockLedgerStorage, OmegaRaftTypeConfig};

/// Errors returned by the mock-ledger storage and apply surfaces.
///
/// The enum is non-exhaustive because later harness phases add network,
/// snapshot, and Cardano transaction-validation surfaces.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LedgerError {
    /// Filesystem operation failed while opening or snapshotting the ledger.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// SQLite rejected a query, transaction, PRAGMA, or schema operation.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// A reader connection could not be borrowed from the r2d2 pool.
    #[error("sqlite reader pool error: {0}")]
    Pool(#[from] r2d2::Error),
    /// The dedicated writer actor has stopped accepting write commands.
    #[error("writer actor is closed")]
    WriterClosed,
    /// The writer actor accepted a command but dropped its reply channel.
    #[error("writer actor dropped the reply channel")]
    WriterReplyCanceled,
    /// A blocking reader task failed to join the Tokio runtime.
    #[error("reader task failed: {0}")]
    ReaderJoin(String),
    /// The claim verifier rejected the proof or its public inputs.
    #[error("claim verification failed: {0}")]
    Verify(#[from] VerifyError),
    /// The typed claim envelope is internally inconsistent.
    #[error("claim shape is invalid: {0}")]
    InvalidClaim(&'static str),
    /// Postcard failed to encode or decode raft metadata.
    #[error("codec error: {0}")]
    Codec(String),
    /// A u64 value cannot be represented in SQLite's signed INTEGER range.
    #[error("integer value {value} for {field} does not fit sqlite INTEGER")]
    IntegerOutOfRange {
        /// Field whose value overflowed SQLite's signed INTEGER range.
        field: &'static str,
        /// Rejected u64 value.
        value: u64,
    },
    /// A claim tried to reuse an already-applied nullifier.
    #[error("claim replay for sub_tree_id={sub_tree_id}, leaf_index={leaf_index}")]
    Replay {
        /// Sub-tree id of the replayed nullifier.
        sub_tree_id: u8,
        /// Leaf index of the replayed nullifier.
        leaf_index: u64,
    },
}

impl From<JoinError> for LedgerError {
    fn from(error: JoinError) -> Self {
        Self::ReaderJoin(error.to_string())
    }
}

#[derive(Clone)]
/// SQLite-backed local ledger plus its reader pool and writer actor.
pub struct MockLedger {
    path: PathBuf,
    readers: ReaderPool,
    writer: WriterHandle,
}

impl MockLedger {
    /// Opens or creates a ledger database and starts its writer actor.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError::Io`] when the parent directory cannot be
    /// created, [`LedgerError::Sqlite`] when schema initialization or PRAGMA
    /// setup fails, and [`LedgerError::Pool`] when the reader pool cannot be
    /// constructed.
    ///
    /// # Soundness
    ///
    /// Opening a ledger is the schema-integrity boundary. The function creates
    /// the five `WITHOUT ROWID` tables required by the harness, applies the WAL
    /// PRAGMAs, and starts exactly one writer actor that owns the write
    /// connection. If callers bypass this constructor and write to the same
    /// database with a second writer, snapshot ordering and replay checks are
    /// no longer guaranteed by this crate.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LedgerError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        {
            let conn = schema::open_initialized(&path)?;
            drop(conn);
        }

        let manager = SqliteConnectionManager::file(&path).with_init(|conn| {
            schema::apply_pragmas(conn)?;
            Ok(())
        });
        let readers = Pool::builder()
            .max_size(num_cpus::get().max(1) as u32)
            .build(manager)?;
        let writer = WriterHandle::start(path.clone())?;

        Ok(Self {
            path,
            readers,
            writer,
        })
    }

    /// Returns the database file path backing this ledger.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads SQLite PRAGMAs from the reader pool.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError::Pool`] when a reader connection cannot be
    /// borrowed, or [`LedgerError::Sqlite`] when SQLite rejects a PRAGMA query.
    pub fn pragma_snapshot(&self) -> Result<PragmaSnapshot, LedgerError> {
        let conn = self.readers.get()?;
        schema::pragma_snapshot(&conn).map_err(LedgerError::from)
    }

    /// Checks whether a table exists in the ledger database.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError::Pool`] when a reader connection cannot be
    /// borrowed, or [`LedgerError::Sqlite`] when the schema query fails.
    pub fn table_exists(&self, table: &str) -> Result<bool, LedgerError> {
        let conn = self.readers.get()?;
        let exists = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
            params![table],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(exists != 0)
    }

    /// Verifies and applies a claim envelope at a mock ledger block index.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError::WriterClosed`] or
    /// [`LedgerError::WriterReplyCanceled`] when the writer actor is gone,
    /// [`LedgerError::Verify`] when the STARK verifier rejects the claim,
    /// [`LedgerError::InvalidClaim`] when collection arity is inconsistent,
    /// [`LedgerError::Replay`] when the `(sub_tree_id, leaf_index)` nullifier
    /// already exists, [`LedgerError::IntegerOutOfRange`] when a u64 cannot be
    /// stored as SQLite INTEGER, or [`LedgerError::Sqlite`] when the
    /// transaction fails.
    ///
    /// # Soundness
    ///
    /// This is the untrusted-network-to-durable-state boundary. The writer
    /// actor parses the typed claim, calls [`omega_claim_verifier::verify`]
    /// before opening the SQLite transaction, checks every nullifier inside
    /// that transaction, then inserts the nullifier and Starstream UTxO rows as
    /// one unit. The verifier consumes the v2 [`ClaimPublicInputs`] shape — the
    /// `tree_depth` and `per_sub_tree_root` fields bind every accepted claim to
    /// a specific sub-tree at a specific depth, so a forged proof against a
    /// stale or wrong root is rejected at the verifier layer before any state
    /// mutation. The function prevents forged proofs and replayed leaves from
    /// mutating state. It does not establish consensus ordering; openraft owns
    /// that layer.
    ///
    /// [`ClaimPublicInputs`]: omega_claim_tx::ClaimPublicInputs
    ///
    /// # Limitations
    ///
    /// v0.1 mocks the C6 signature path and derives the Starstream UTxO value
    /// from the first 16 bytes of the leaf payload. Real Cardano transaction
    /// validation is behind the `cardano-tx-validation` feature and still
    /// needs fixture coverage.
    pub async fn apply_claim(
        &self,
        block_idx: u64,
        commitment: &OmegaCommitment,
        claim: ClaimTx,
    ) -> Result<(), LedgerError> {
        self.writer
            .apply_claim(block_idx, commitment.clone(), claim)
            .await
    }

    /// Returns whether a nullifier is visible to a reader connection.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError::ReaderJoin`] when the blocking reader task fails,
    /// [`LedgerError::Pool`] when a reader connection cannot be borrowed,
    /// [`LedgerError::IntegerOutOfRange`] when `leaf_index` cannot fit SQLite
    /// INTEGER, or [`LedgerError::Sqlite`] when the query fails.
    ///
    /// # Soundness
    ///
    /// The read is a point-in-time WAL snapshot. A `true` result is a durable
    /// replay signal for the observed state. A `false` result is not a
    /// reservation: a writer command already queued or mid-transaction may
    /// commit the same nullifier before a future submit reaches the apply
    /// pipeline. The transactional replay check in [`MockLedger::apply_claim`]
    /// is the authoritative guard.
    pub async fn nullifier_exists(
        &self,
        sub_tree_id: u8,
        leaf_index: u64,
    ) -> Result<bool, LedgerError> {
        let pool = self.readers.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let leaf_index = sqlite_i64("leaf_index", leaf_index)?;
            let exists = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM nullifiers WHERE sub_tree_id=?1 AND leaf_index=?2)",
                params![i64::from(sub_tree_id), leaf_index],
                |row| row.get::<_, i64>(0),
            )?;
            Ok::<_, LedgerError>(exists != 0)
        })
        .await?
    }

    /// Counts unspent Starstream UTxO rows currently visible to a reader.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError::ReaderJoin`], [`LedgerError::Pool`], or
    /// [`LedgerError::Sqlite`] if the blocking reader task, pool checkout, or
    /// query fails.
    pub async fn starstream_utxo_count(&self) -> Result<u64, LedgerError> {
        let pool = self.readers.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let count = conn.query_row("SELECT COUNT(*) FROM starstream_utxos", [], |row| {
                row.get::<_, i64>(0)
            })?;
            Ok::<_, LedgerError>(count as u64)
        })
        .await?
    }

    /// Counts nullifier rows currently visible to a reader.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerError::ReaderJoin`], [`LedgerError::Pool`], or
    /// [`LedgerError::Sqlite`] if the blocking reader task, pool checkout, or
    /// query fails.
    pub async fn nullifier_count(&self) -> Result<u64, LedgerError> {
        let pool = self.readers.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let count = conn.query_row("SELECT COUNT(*) FROM nullifiers", [], |row| {
                row.get::<_, i64>(0)
            })?;
            Ok::<_, LedgerError>(count as u64)
        })
        .await?
    }

    /// Requests a `PRAGMA wal_checkpoint(TRUNCATE)` on the writer actor.
    ///
    /// # Errors
    ///
    /// Returns writer-channel errors when the actor is unavailable, or
    /// [`LedgerError::Sqlite`] when SQLite rejects the checkpoint.
    pub async fn checkpoint_wal_truncate(&self) -> Result<(), LedgerError> {
        self.writer.checkpoint_wal_truncate().await
    }

    /// Starts the default 30-second WAL truncate loop.
    ///
    /// The task intentionally ignores individual checkpoint failures; ordinary
    /// write commands keep reporting their own errors through the writer actor.
    pub fn spawn_wal_truncate_task(&self) -> tokio::task::JoinHandle<()> {
        self.spawn_wal_truncate_task_with_interval(Duration::from_secs(30))
    }

    /// Starts a WAL truncate loop with a caller-supplied interval.
    ///
    /// Test code uses this to exercise the checkpoint path without waiting for
    /// the production 30-second cadence.
    pub fn spawn_wal_truncate_task_with_interval(
        &self,
        interval: Duration,
    ) -> tokio::task::JoinHandle<()> {
        let writer = self.writer.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                let _ = writer.checkpoint_wal_truncate().await;
            }
        })
    }

    /// Creates a SQLite snapshot file through the writer actor.
    ///
    /// # Errors
    ///
    /// Returns writer-channel errors when the actor is unavailable,
    /// [`LedgerError::Io`] when an existing snapshot cannot be removed, or
    /// [`LedgerError::Sqlite`] when `VACUUM INTO` fails.
    ///
    /// # Soundness
    ///
    /// Snapshot creation flows through the same writer channel as ordinary
    /// writes. Channel ordering is the synchronization primitive: every write
    /// before the snapshot command is included, and every later write waits
    /// until SQLite finishes `VACUUM INTO`.
    pub async fn snapshot(&self, snapshot_id: String) -> Result<PathBuf, LedgerError> {
        self.writer.snapshot(snapshot_id).await
    }

    /// Restores this ledger from a snapshot file through the writer actor.
    ///
    /// # Errors
    ///
    /// Returns writer-channel errors when the actor is unavailable,
    /// [`LedgerError::Io`] when the snapshot file cannot be removed after
    /// restore, or [`LedgerError::Sqlite`] when attach, copy, commit, or detach
    /// operations fail.
    ///
    /// # Soundness
    ///
    /// Restore is serialized with ordinary writes by the writer channel. v0.1
    /// uses `ATTACH DATABASE` plus table-copy instead of dropping the live DB
    /// and renaming the snapshot file; see the crate-level limitations for the
    /// Windows reader-pool teardown reason.
    pub async fn restore_snapshot(&self, snapshot_path: PathBuf) -> Result<(), LedgerError> {
        self.writer.restore_snapshot(snapshot_path).await
    }

    #[doc(hidden)]
    pub async fn insert_synthetic_claim_for_test(
        &self,
        sub_tree_id: u8,
        leaf_index: u64,
        block_idx: u64,
    ) -> Result<(), LedgerError> {
        self.writer
            .insert_synthetic_claim_for_test(sub_tree_id, leaf_index, block_idx)
            .await
    }
}

pub(crate) fn sqlite_i64(field: &'static str, value: u64) -> Result<i64, LedgerError> {
    i64::try_from(value).map_err(|_| LedgerError::IntegerOutOfRange { field, value })
}

pub(crate) fn starstream_utxo_id(
    recipient: &Hash,
    leaf_payload: &[u8],
    block_idx: u64,
    ordinal: u64,
) -> Hash {
    let mut bytes = Vec::with_capacity(32 + leaf_payload.len() + 16 + 24);
    bytes.extend_from_slice(b"omega:mock-ledger:v1:utxo");
    bytes.extend_from_slice(recipient);
    bytes.extend_from_slice(&block_idx.to_be_bytes());
    bytes.extend_from_slice(&ordinal.to_be_bytes());
    bytes.extend_from_slice(&(leaf_payload.len() as u64).to_be_bytes());
    bytes.extend_from_slice(leaf_payload);
    omega_commitment_core::hash::blake3_256(&bytes)
}
