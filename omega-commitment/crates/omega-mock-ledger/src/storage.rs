#![allow(clippy::result_large_err)]

// Openraft 0.9 fixes `StorageError<u64>` in the storage trait surface. This
// module keeps that error type unboxed so the impls match upstream exactly.

use std::fmt::Debug;
use std::io::Cursor;
use std::ops::{Bound, RangeBounds};
use std::path::PathBuf;

use omega_claim_prover::{OmegaCommitment, ProofEnvelope};
use omega_claim_tx::{ClaimTx, ProofBytes};
use omega_claim_verifier::VerifyError;
use openraft::storage::Adaptor;
use openraft::{
    EntryPayload, ErrorSubject, ErrorVerb, LogId, LogState, RaftLogReader, RaftSnapshotBuilder,
    RaftStorage, Snapshot, SnapshotMeta, StorageError, StoredMembership, Vote,
};
use rusqlite::OptionalExtension;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::writer::RaftLogRow;
use crate::{sqlite_i64, MockLedger};

openraft::declare_raft_types!(
    #[doc = "Openraft type configuration for the mock-ledger state machine."]
    pub OmegaRaftTypeConfig:
        D = LedgerCommand,
        R = LedgerResponse,
);

/// Raft log entry type used by the mock-ledger storage adapter.
pub type OmegaRaftEntry = openraft::Entry<OmegaRaftTypeConfig>;

/// Command replicated through openraft before entering the ledger apply path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LedgerCommand {
    /// Mock block index associated with the claim application.
    pub block_idx: u64,
    /// Published commitment that the proof is verified against.
    pub commitment: OmegaCommitment,
    /// Typed claim envelope to verify and apply.
    pub claim: ClaimTx,
}

impl LedgerCommand {
    /// Builds a replicated apply command from a submitted claim transaction.
    ///
    /// The JSON-RPC Group 1 surface accepts `ClaimTx` directly. The proof
    /// envelope inside the claim carries the published commitment, so this
    /// constructor extracts that commitment before the command enters raft.
    ///
    /// # Errors
    ///
    /// Returns [`crate::LedgerError::Verify`] when the claim's proof bytes do
    /// not decode as a proof envelope.
    pub fn apply_claim(claim: ClaimTx) -> Result<Self, crate::LedgerError> {
        let envelope: ProofEnvelope = postcard::from_bytes(&claim_proof(&claim).0)
            .map_err(|_| crate::LedgerError::Verify(VerifyError::InvalidProof))?;
        Ok(Self {
            block_idx: 0,
            commitment: envelope.commitment,
            claim,
        })
    }
}

fn claim_proof(claim: &ClaimTx) -> &ProofBytes {
    match claim {
        ClaimTx::Utxo(claim) => &claim.proof,
        ClaimTx::Collection(claim) => &claim.proof,
    }
}

/// Response returned after a replicated ledger command is applied.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LedgerResponse {
    /// Whether the command mutated ledger state.
    pub accepted: bool,
    /// Structured rejection class when `accepted` is false.
    pub reject: Option<LedgerReject>,
    /// Rejection text when `accepted` is false.
    pub error: Option<String>,
}

/// Serializable rejection class returned through raft apply responses.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LedgerReject {
    /// The proof verifier rejected the claim.
    Verify,
    /// The claim envelope was internally inconsistent.
    InvalidClaim,
    /// The claim reused an already-applied nullifier.
    Replay,
    /// The writer actor closed while applying the command.
    WriterClosed,
    /// Any other ledger-side failure.
    Internal,
}

impl LedgerResponse {
    /// Builds a successful apply response.
    pub fn accepted() -> Self {
        Self {
            accepted: true,
            reject: None,
            error: None,
        }
    }

    /// Builds a rejected apply response with a displayable reason.
    pub fn rejected(error: crate::LedgerError) -> Self {
        let reject = match &error {
            crate::LedgerError::Verify(_) => LedgerReject::Verify,
            crate::LedgerError::InvalidClaim(_) => LedgerReject::InvalidClaim,
            crate::LedgerError::Replay { .. } => LedgerReject::Replay,
            crate::LedgerError::WriterClosed | crate::LedgerError::WriterReplyCanceled => {
                LedgerReject::WriterClosed
            }
            _ => LedgerReject::Internal,
        };
        Self {
            accepted: false,
            reject: Some(reject),
            error: Some(error.to_string()),
        }
    }
}

#[derive(Clone)]
/// Openraft storage facade backed by [`MockLedger`].
pub struct MockLedgerStorage {
    ledger: MockLedger,
}

impl MockLedgerStorage {
    /// Wraps an opened ledger for use as openraft storage.
    pub fn new(ledger: MockLedger) -> Self {
        Self { ledger }
    }

    /// Splits this storage into openraft's log-store and state-machine parts.
    ///
    /// openraft 0.9 exposes split storage traits through
    /// [`openraft::storage::Adaptor`]. Both returned adaptors share the same
    /// underlying writer actor and reader pool.
    pub fn openraft_parts(
        self,
    ) -> (
        Adaptor<OmegaRaftTypeConfig, Self>,
        Adaptor<OmegaRaftTypeConfig, Self>,
    ) {
        Adaptor::new(self)
    }

    async fn read_meta<T>(&self, key: &'static str) -> Result<Option<T>, StorageError<u64>>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let pool = self.ledger.readers.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(sto_read)?;
            let value = conn
                .query_row("SELECT v FROM raft_meta WHERE k=?1", [key], |row| {
                    row.get::<_, Vec<u8>>(0)
                })
                .optional()
                .map_err(sto_read)?;
            value.map(|bytes| decode(&bytes)).transpose()
        })
        .await
        .map_err(sto_read)?
    }
}

impl RaftLogReader<OmegaRaftTypeConfig> for MockLedgerStorage {
    async fn try_get_log_entries<RB: RangeBounds<u64> + Clone + Debug + Send>(
        &mut self,
        range: RB,
    ) -> Result<Vec<OmegaRaftEntry>, StorageError<u64>> {
        let (start, end) = range_bounds(range)?;
        if let Some(end) = end {
            if end <= start {
                return Ok(Vec::new());
            }
        }

        let pool = self.ledger.readers.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(sto_read)?;
            let mut entries = Vec::new();
            if let Some(end) = end {
                let mut stmt = conn
                    .prepare(
                        "SELECT payload FROM raft_log
                         WHERE log_idx >= ?1 AND log_idx < ?2
                         ORDER BY log_idx ASC",
                    )
                    .map_err(sto_read)?;
                let rows = stmt
                    .query_map(
                        [
                            sqlite_i64("log_idx", start).map_err(sto_read)?,
                            sqlite_i64("log_idx", end).map_err(sto_read)?,
                        ],
                        |row| row.get::<_, Vec<u8>>(0),
                    )
                    .map_err(sto_read)?;
                for row in rows {
                    entries.push(decode(&row.map_err(sto_read)?)?);
                }
            } else {
                let mut stmt = conn
                    .prepare(
                        "SELECT payload FROM raft_log
                         WHERE log_idx >= ?1
                         ORDER BY log_idx ASC",
                    )
                    .map_err(sto_read)?;
                let rows = stmt
                    .query_map([sqlite_i64("log_idx", start).map_err(sto_read)?], |row| {
                        row.get::<_, Vec<u8>>(0)
                    })
                    .map_err(sto_read)?;
                for row in rows {
                    entries.push(decode(&row.map_err(sto_read)?)?);
                }
            }
            Ok(entries)
        })
        .await
        .map_err(sto_read)?
    }
}

impl RaftStorage<OmegaRaftTypeConfig> for MockLedgerStorage {
    type LogReader = Self;
    type SnapshotBuilder = Self;

    async fn save_vote(&mut self, vote: &Vote<u64>) -> Result<(), StorageError<u64>> {
        let value = encode(vote)?;
        self.ledger
            .writer
            .save_raft_meta("vote", Some(value))
            .await
            .map_err(sto_write)
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<u64>>, StorageError<u64>> {
        self.read_meta("vote").await
    }

    /// Persist the latest committed `LogId` to the `raft_meta` table.
    ///
    /// # Soundness
    ///
    /// Writes the committed cursor through the writer actor so it shares the
    /// same FIFO ordering as state-machine applies. openraft expects this to
    /// be durable before crashes; the WAL fsync at COMMIT inside the writer
    /// thread provides that durability.
    async fn save_committed(
        &mut self,
        committed: Option<LogId<u64>>,
    ) -> Result<(), StorageError<u64>> {
        let value = committed.as_ref().map(encode).transpose()?;
        self.ledger
            .writer
            .save_raft_meta("committed", value)
            .await
            .map_err(sto_write)
    }

    async fn read_committed(&mut self) -> Result<Option<LogId<u64>>, StorageError<u64>> {
        self.read_meta("committed").await
    }

    async fn get_log_state(&mut self) -> Result<LogState<OmegaRaftTypeConfig>, StorageError<u64>> {
        let last_purged_log_id = self.read_meta("last_purged_log_id").await?;
        let pool = self.ledger.readers.clone();
        let last_log_id = tokio::task::spawn_blocking(move || {
            let conn = pool.get().map_err(sto_read)?;
            let payload = conn
                .query_row(
                    "SELECT payload FROM raft_log ORDER BY log_idx DESC LIMIT 1",
                    [],
                    |row| row.get::<_, Vec<u8>>(0),
                )
                .optional()
                .map_err(sto_read)?;
            payload
                .map(|bytes| decode::<OmegaRaftEntry>(&bytes).map(|entry| entry.log_id))
                .transpose()
        })
        .await
        .map_err(sto_read)??;

        Ok(LogState {
            last_purged_log_id,
            last_log_id: last_log_id.or(last_purged_log_id),
        })
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        self.clone()
    }

    async fn append_to_log<I>(&mut self, entries: I) -> Result<(), StorageError<u64>>
    where
        I: IntoIterator<Item = OmegaRaftEntry> + Send,
    {
        let rows = entries
            .into_iter()
            .map(|entry| {
                Ok(RaftLogRow {
                    log_idx: entry.log_id.index,
                    term: entry.log_id.leader_id.term,
                    payload: encode(&entry)?,
                })
            })
            .collect::<Result<Vec<_>, StorageError<u64>>>()?;
        self.ledger
            .writer
            .append_raft_logs(rows)
            .await
            .map_err(sto_write)
    }

    async fn delete_conflict_logs_since(
        &mut self,
        log_id: LogId<u64>,
    ) -> Result<(), StorageError<u64>> {
        self.ledger
            .writer
            .delete_raft_logs_since(log_id.index)
            .await
            .map_err(sto_write)
    }

    async fn purge_logs_upto(&mut self, log_id: LogId<u64>) -> Result<(), StorageError<u64>> {
        self.ledger
            .writer
            .purge_raft_logs_upto(log_id.index, encode(&log_id)?)
            .await
            .map_err(sto_write)
    }

    async fn last_applied_state(
        &mut self,
    ) -> Result<
        (
            Option<LogId<u64>>,
            StoredMembership<u64, openraft::BasicNode>,
        ),
        StorageError<u64>,
    > {
        let last_applied = self.read_meta("last_applied_log_id").await?;
        let membership = self.read_meta("last_membership").await?.unwrap_or_default();
        Ok((last_applied, membership))
    }

    /// Apply a batch of committed Raft entries to the state machine.
    ///
    /// # Soundness
    ///
    /// Routes the entries through the writer actor (`writer::apply_raft_entries`),
    /// which calls [`omega_claim_verifier::verify`] on every claim payload before
    /// any state mutation, then opens a SQLite transaction that performs the
    /// nullifier-replay probe and the nullifier + Starstream UTxO inserts as a
    /// single unit. A malformed entry returns an `Err(LedgerResponse::…)`
    /// alongside the openraft `applied_log_id` advance so consensus does not
    /// stall behind a rejected entry, but the *state* of the rejected entry is
    /// never written. Membership entries in the batch are extracted into the
    /// optional `last_membership` blob and persisted at the end of the batch as
    /// monotonic state-machine metadata.
    async fn apply_to_state_machine(
        &mut self,
        entries: &[OmegaRaftEntry],
    ) -> Result<Vec<LedgerResponse>, StorageError<u64>> {
        let mut last_membership = None;
        for entry in entries {
            if let EntryPayload::Membership(membership) = &entry.payload {
                // The latest membership in the batch is persisted after the
                // batch applies. A later normal entry may reject without
                // invalidating this membership transition; openraft treats both
                // values as monotonic state-machine metadata.
                let stored = StoredMembership::new(Some(entry.log_id), membership.clone());
                last_membership = Some(encode(&stored)?);
            }
        }
        self.ledger
            .writer
            .apply_raft_entries(entries.to_vec(), last_membership)
            .await
            .map_err(sto_write)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.clone()
    }

    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<Cursor<Vec<u8>>>, StorageError<u64>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    /// Install a snapshot received from a leader, replacing the live state.
    ///
    /// # Soundness
    ///
    /// The snapshot bytes are written to disk under
    /// `installed_snapshot_path(...)` and then handed to the writer actor's
    /// `restore_snapshot` path, which serialises against ordinary writes via
    /// the same mpsc channel — there is no window where a reader can observe a
    /// half-replaced state because the live DB swap is fenced by the channel's
    /// FIFO order. The `meta` blob is persisted last so a crash mid-install
    /// surfaces as "no installed snapshot" rather than "snapshot meta points
    /// at half-written state".
    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<u64, openraft::BasicNode>,
        mut snapshot: Box<Cursor<Vec<u8>>>,
    ) -> Result<(), StorageError<u64>> {
        snapshot.rewind().await.map_err(sto_write)?;
        let mut bytes = Vec::new();
        snapshot.read_to_end(&mut bytes).await.map_err(sto_write)?;
        let snapshot_path = installed_snapshot_path(self.ledger.path(), &meta.snapshot_id);
        std::fs::write(&snapshot_path, bytes).map_err(sto_write)?;
        self.ledger
            .writer
            .restore_snapshot(snapshot_path.clone())
            .await
            .map_err(sto_write)?;
        self.ledger
            .writer
            .save_raft_meta("snapshot_meta", Some(encode(meta)?))
            .await
            .map_err(sto_write)?;
        Ok(())
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<OmegaRaftTypeConfig>>, StorageError<u64>> {
        let meta: Option<SnapshotMeta<u64, openraft::BasicNode>> =
            self.read_meta("snapshot_meta").await?;
        let Some(meta) = meta else {
            return Ok(None);
        };
        let path = installed_snapshot_path(self.ledger.path(), &meta.snapshot_id);
        let bytes = std::fs::read(path).map_err(sto_read)?;
        Ok(Some(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(bytes)),
        }))
    }
}

impl RaftSnapshotBuilder<OmegaRaftTypeConfig> for MockLedgerStorage {
    async fn build_snapshot(&mut self) -> Result<Snapshot<OmegaRaftTypeConfig>, StorageError<u64>> {
        let (last_log_id, last_membership) = self.last_applied_state().await?;
        let snapshot_id = match last_log_id {
            Some(log_id) => format!("{}-{}", log_id.leader_id.term, log_id.index),
            None => "empty".to_string(),
        };
        let meta = SnapshotMeta {
            last_log_id,
            last_membership,
            snapshot_id,
        };
        let snapshot_path = self
            .ledger
            .writer
            .snapshot(meta.snapshot_id.clone())
            .await
            .map_err(sto_write)?;
        let bytes = std::fs::read(&snapshot_path).map_err(sto_read)?;
        self.ledger
            .writer
            .save_raft_meta("snapshot_meta", Some(encode(&meta)?))
            .await
            .map_err(sto_write)?;
        Ok(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(bytes)),
        })
    }
}

#[allow(clippy::result_large_err)]
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, StorageError<u64>> {
    postcard::to_allocvec(value).map_err(sto_write)
}

#[allow(clippy::result_large_err)]
fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, StorageError<u64>> {
    postcard::from_bytes(bytes).map_err(sto_read)
}

#[allow(clippy::result_large_err)]
fn range_bounds<RB: RangeBounds<u64>>(range: RB) -> Result<(u64, Option<u64>), StorageError<u64>> {
    let start = match range.start_bound() {
        Bound::Included(value) => *value,
        Bound::Excluded(value) => value
            .checked_add(1)
            .ok_or_else(|| sto_read("range start overflows u64"))?,
        Bound::Unbounded => 0,
    };
    let end = match range.end_bound() {
        Bound::Included(value) => Some(
            value
                .checked_add(1)
                .ok_or_else(|| sto_read("range end overflows u64"))?,
        ),
        Bound::Excluded(value) => Some(*value),
        Bound::Unbounded => None,
    };
    Ok((start, end))
}

fn installed_snapshot_path(db_path: &std::path::Path, snapshot_id: &str) -> PathBuf {
    db_path.with_file_name(format!("snapshot-{snapshot_id}.sqlite"))
}

fn sto_read(error: impl std::fmt::Display) -> StorageError<u64> {
    StorageError::from_io_error(
        ErrorSubject::Store,
        ErrorVerb::Read,
        std::io::Error::other(error.to_string()),
    )
}

fn sto_write(error: impl std::fmt::Display) -> StorageError<u64> {
    StorageError::from_io_error(
        ErrorSubject::Store,
        ErrorVerb::Write,
        std::io::Error::other(error.to_string()),
    )
}
