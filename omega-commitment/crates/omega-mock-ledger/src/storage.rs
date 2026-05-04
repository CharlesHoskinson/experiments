#![allow(clippy::result_large_err)]

// Openraft 0.9 fixes `StorageError<u64>` in the storage trait surface. This
// module keeps that error type unboxed so the impls match upstream exactly.

use std::fmt::Debug;
use std::io::Cursor;
use std::ops::{Bound, RangeBounds};
use std::path::PathBuf;

use omega_claim_prover::OmegaCommitment;
use omega_claim_tx::ClaimTx;
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
    pub OmegaRaftTypeConfig:
        D = LedgerCommand,
        R = LedgerResponse,
);

pub type OmegaRaftEntry = openraft::Entry<OmegaRaftTypeConfig>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LedgerCommand {
    pub block_idx: u64,
    pub commitment: OmegaCommitment,
    pub claim: ClaimTx,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LedgerResponse {
    pub accepted: bool,
    pub error: Option<String>,
}

impl LedgerResponse {
    pub fn accepted() -> Self {
        Self {
            accepted: true,
            error: None,
        }
    }

    pub fn rejected(error: String) -> Self {
        Self {
            accepted: false,
            error: Some(error),
        }
    }
}

#[derive(Clone)]
pub struct MockLedgerStorage {
    ledger: MockLedger,
}

impl MockLedgerStorage {
    pub fn new(ledger: MockLedger) -> Self {
        Self { ledger }
    }

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

    async fn apply_to_state_machine(
        &mut self,
        entries: &[OmegaRaftEntry],
    ) -> Result<Vec<LedgerResponse>, StorageError<u64>> {
        let mut last_membership = None;
        for entry in entries {
            if let EntryPayload::Membership(membership) = &entry.payload {
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
