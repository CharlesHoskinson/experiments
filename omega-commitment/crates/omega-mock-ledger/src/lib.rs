#![forbid(unsafe_code)]
// `openraft::StorageError<u64>` is 224 bytes; the `result_large_err` lint
// (Rust 1.95+) flags every `Result<_, openraft::StorageError<_>>` site.
// The error size is fixed by openraft's API; boxing it would change the
// trait-impl shape openraft expects. Allow the lint crate-locally until
// openraft 0.10+ ships a boxed-error variant.
#![allow(clippy::result_large_err)]

//! SQLite-backed mock ledger for the proof experiment harness.
//!
//! State-changing operations use a writer actor: one dedicated OS thread owns
//! the `rusqlite::Connection`, receives `WriteCmd`s over a Tokio mpsc channel,
//! and replies over a oneshot. This keeps synchronous SQLite writes out of the
//! async runtime and avoids a per-call `spawn_blocking` herd contending for the
//! same WAL writer. Reader queries use short-lived r2d2 connections and run in
//! `spawn_blocking` when exposed through async methods.

#[cfg(feature = "cardano-tx-validation")]
pub mod cardano;
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

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LedgerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("sqlite reader pool error: {0}")]
    Pool(#[from] r2d2::Error),
    #[error("writer actor is closed")]
    WriterClosed,
    #[error("writer actor dropped the reply channel")]
    WriterReplyCanceled,
    #[error("reader task failed: {0}")]
    ReaderJoin(String),
    #[error("claim verification failed: {0}")]
    Verify(#[from] VerifyError),
    #[error("claim shape is invalid: {0}")]
    InvalidClaim(&'static str),
    #[error("codec error: {0}")]
    Codec(String),
    #[error("integer value {value} for {field} does not fit sqlite INTEGER")]
    IntegerOutOfRange { field: &'static str, value: u64 },
    #[error("claim replay for sub_tree_id={sub_tree_id}, leaf_index={leaf_index}")]
    Replay { sub_tree_id: u8, leaf_index: u64 },
}

impl From<JoinError> for LedgerError {
    fn from(error: JoinError) -> Self {
        Self::ReaderJoin(error.to_string())
    }
}

#[derive(Clone)]
pub struct MockLedger {
    path: PathBuf,
    readers: ReaderPool,
    writer: WriterHandle,
}

impl MockLedger {
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

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn pragma_snapshot(&self) -> Result<PragmaSnapshot, LedgerError> {
        let conn = self.readers.get()?;
        schema::pragma_snapshot(&conn).map_err(LedgerError::from)
    }

    pub fn table_exists(&self, table: &str) -> Result<bool, LedgerError> {
        let conn = self.readers.get()?;
        let exists = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
            params![table],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(exists != 0)
    }

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

    pub async fn checkpoint_wal_truncate(&self) -> Result<(), LedgerError> {
        self.writer.checkpoint_wal_truncate().await
    }

    pub fn spawn_wal_truncate_task(&self) -> tokio::task::JoinHandle<()> {
        self.spawn_wal_truncate_task_with_interval(Duration::from_secs(30))
    }

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

    pub async fn snapshot(&self, snapshot_id: String) -> Result<PathBuf, LedgerError> {
        self.writer.snapshot(snapshot_id).await
    }

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
