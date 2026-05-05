//! Writer-actor for the SQLite-backed mock ledger.
//!
//! A single dedicated OS thread (`std::thread::spawn`, **not** a tokio task)
//! owns the rusqlite write [`Connection`] for the ledger's primary database.
//! Async callers send a [`WriteCmd`] over a Tokio mpsc channel and await the
//! reply on a oneshot. This pattern is required (not just preferred) because
//! openraft's `RaftStateMachine` apply path expects the future it polls to
//! make progress under back-pressure: per-call `tokio::task::spawn_blocking`
//! would not pipeline writes and would produce a thundering herd against
//! SQLite's single-writer WAL serialisation under load. The actor pattern
//! eliminates that herd by serialising every mutating command through one
//! channel.
//!
//! The same channel carries snapshot, restore, and WAL-checkpoint commands.
//! The implicit channel ordering is the only synchronisation between writes
//! and snapshots; no separate mutex is needed.
//!
//! # Soundness
//!
//! Mutating commands route exclusively through this actor; readers borrow
//! short-lived `r2d2_sqlite::Connection`s from a pool sized to
//! `num_cpus::get()`. The single-writer property guarantees that the
//! verify-before-mutate invariant in [`crate::MockLedger::apply_claim`] holds
//! across all callers — there is no second path that could insert a
//! nullifier without first calling [`omega_claim_verifier::verify`].

use std::path::PathBuf;
use std::thread;

use omega_claim_prover::OmegaCommitment;
use omega_claim_tx::{ClaimPublicInputs, ClaimTx, ClaimWitness, ProofBytes};
use omega_claim_verifier::verify;
use rusqlite::{params, Connection, OptionalExtension};
use tokio::sync::{mpsc, oneshot};

use crate::storage::{LedgerResponse, OmegaRaftEntry};
use crate::{schema, sqlite_i64, starstream_utxo_id, LedgerError};

#[derive(Clone)]
pub(crate) struct WriterHandle {
    tx: mpsc::UnboundedSender<WriteCmd>,
}

impl WriterHandle {
    pub(crate) fn start(path: PathBuf) -> Result<Self, LedgerError> {
        let conn = schema::open_initialized(&path)?;
        let (tx, rx) = mpsc::unbounded_channel();
        thread::Builder::new()
            .name("omega-mock-ledger-writer".to_string())
            .spawn(move || run_writer(path, conn, rx))
            .map_err(LedgerError::Io)?;
        Ok(Self { tx })
    }

    pub(crate) async fn apply_claim(
        &self,
        block_idx: u64,
        commitment: OmegaCommitment,
        claim: ClaimTx,
    ) -> Result<(), LedgerError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(WriteCmd::ApplyClaim(Box::new(ApplyClaimCmd {
                block_idx,
                commitment,
                claim,
                reply,
            })))
            .map_err(|_| LedgerError::WriterClosed)?;
        rx.await.map_err(|_| LedgerError::WriterReplyCanceled)?
    }

    pub(crate) async fn checkpoint_wal_truncate(&self) -> Result<(), LedgerError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(WriteCmd::CheckpointWalTruncate { reply })
            .map_err(|_| LedgerError::WriterClosed)?;
        rx.await.map_err(|_| LedgerError::WriterReplyCanceled)?
    }

    pub(crate) async fn snapshot(&self, snapshot_id: String) -> Result<PathBuf, LedgerError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(WriteCmd::Snapshot { snapshot_id, reply })
            .map_err(|_| LedgerError::WriterClosed)?;
        rx.await.map_err(|_| LedgerError::WriterReplyCanceled)?
    }

    pub(crate) async fn restore_snapshot(&self, snapshot_path: PathBuf) -> Result<(), LedgerError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(WriteCmd::RestoreSnapshot {
                snapshot_path,
                reply,
            })
            .map_err(|_| LedgerError::WriterClosed)?;
        rx.await.map_err(|_| LedgerError::WriterReplyCanceled)?
    }

    pub(crate) async fn insert_synthetic_claim_for_test(
        &self,
        sub_tree_id: u8,
        leaf_index: u64,
        block_idx: u64,
    ) -> Result<(), LedgerError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(WriteCmd::InsertSyntheticClaim {
                sub_tree_id,
                leaf_index,
                block_idx,
                reply,
            })
            .map_err(|_| LedgerError::WriterClosed)?;
        rx.await.map_err(|_| LedgerError::WriterReplyCanceled)?
    }

    pub(crate) async fn save_raft_meta(
        &self,
        key: &'static str,
        value: Option<Vec<u8>>,
    ) -> Result<(), LedgerError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(WriteCmd::SaveRaftMeta { key, value, reply })
            .map_err(|_| LedgerError::WriterClosed)?;
        rx.await.map_err(|_| LedgerError::WriterReplyCanceled)?
    }

    pub(crate) async fn append_raft_logs(&self, rows: Vec<RaftLogRow>) -> Result<(), LedgerError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(WriteCmd::AppendRaftLogs { rows, reply })
            .map_err(|_| LedgerError::WriterClosed)?;
        rx.await.map_err(|_| LedgerError::WriterReplyCanceled)?
    }

    pub(crate) async fn delete_raft_logs_since(&self, log_idx: u64) -> Result<(), LedgerError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(WriteCmd::DeleteRaftLogsSince { log_idx, reply })
            .map_err(|_| LedgerError::WriterClosed)?;
        rx.await.map_err(|_| LedgerError::WriterReplyCanceled)?
    }

    pub(crate) async fn purge_raft_logs_upto(
        &self,
        log_idx: u64,
        last_purged: Vec<u8>,
    ) -> Result<(), LedgerError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(WriteCmd::PurgeRaftLogsUpto {
                log_idx,
                last_purged,
                reply,
            })
            .map_err(|_| LedgerError::WriterClosed)?;
        rx.await.map_err(|_| LedgerError::WriterReplyCanceled)?
    }

    pub(crate) async fn apply_raft_entries(
        &self,
        entries: Vec<OmegaRaftEntry>,
        last_membership: Option<Vec<u8>>,
    ) -> Result<Vec<LedgerResponse>, LedgerError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(WriteCmd::ApplyRaftEntries {
                entries,
                last_membership,
                reply,
            })
            .map_err(|_| LedgerError::WriterClosed)?;
        rx.await.map_err(|_| LedgerError::WriterReplyCanceled)?
    }
}

pub(crate) struct RaftLogRow {
    pub(crate) log_idx: u64,
    pub(crate) term: u64,
    pub(crate) payload: Vec<u8>,
}

enum WriteCmd {
    ApplyClaim(Box<ApplyClaimCmd>),
    CheckpointWalTruncate {
        reply: oneshot::Sender<Result<(), LedgerError>>,
    },
    Snapshot {
        snapshot_id: String,
        reply: oneshot::Sender<Result<PathBuf, LedgerError>>,
    },
    RestoreSnapshot {
        snapshot_path: PathBuf,
        reply: oneshot::Sender<Result<(), LedgerError>>,
    },
    InsertSyntheticClaim {
        sub_tree_id: u8,
        leaf_index: u64,
        block_idx: u64,
        reply: oneshot::Sender<Result<(), LedgerError>>,
    },
    SaveRaftMeta {
        key: &'static str,
        value: Option<Vec<u8>>,
        reply: oneshot::Sender<Result<(), LedgerError>>,
    },
    AppendRaftLogs {
        rows: Vec<RaftLogRow>,
        reply: oneshot::Sender<Result<(), LedgerError>>,
    },
    DeleteRaftLogsSince {
        log_idx: u64,
        reply: oneshot::Sender<Result<(), LedgerError>>,
    },
    PurgeRaftLogsUpto {
        log_idx: u64,
        last_purged: Vec<u8>,
        reply: oneshot::Sender<Result<(), LedgerError>>,
    },
    ApplyRaftEntries {
        entries: Vec<OmegaRaftEntry>,
        last_membership: Option<Vec<u8>>,
        reply: oneshot::Sender<Result<Vec<LedgerResponse>, LedgerError>>,
    },
}

struct ApplyClaimCmd {
    block_idx: u64,
    commitment: OmegaCommitment,
    claim: ClaimTx,
    reply: oneshot::Sender<Result<(), LedgerError>>,
}

fn run_writer(path: PathBuf, mut conn: Connection, mut rx: mpsc::UnboundedReceiver<WriteCmd>) {
    while let Some(cmd) = rx.blocking_recv() {
        match cmd {
            WriteCmd::ApplyClaim(cmd) => {
                let _ = cmd.reply.send(apply_claim_tx(
                    &mut conn,
                    cmd.block_idx,
                    &cmd.commitment,
                    cmd.claim,
                ));
            }
            WriteCmd::CheckpointWalTruncate { reply } => {
                let _ = reply.send(checkpoint_wal_truncate(&conn));
            }
            WriteCmd::Snapshot { snapshot_id, reply } => {
                let _ = reply.send(snapshot(&conn, &path, &snapshot_id));
            }
            WriteCmd::RestoreSnapshot {
                snapshot_path,
                reply,
            } => {
                let _ = reply.send(restore_snapshot(&mut conn, &snapshot_path));
            }
            WriteCmd::InsertSyntheticClaim {
                sub_tree_id,
                leaf_index,
                block_idx,
                reply,
            } => {
                let _ = reply.send(insert_synthetic_claim(
                    &mut conn,
                    sub_tree_id,
                    leaf_index,
                    block_idx,
                ));
            }
            WriteCmd::SaveRaftMeta { key, value, reply } => {
                let _ = reply.send(save_raft_meta(&conn, key, value.as_deref()));
            }
            WriteCmd::AppendRaftLogs { rows, reply } => {
                let _ = reply.send(append_raft_logs(&mut conn, &rows));
            }
            WriteCmd::DeleteRaftLogsSince { log_idx, reply } => {
                let _ = reply.send(delete_raft_logs_since(&conn, log_idx));
            }
            WriteCmd::PurgeRaftLogsUpto {
                log_idx,
                last_purged,
                reply,
            } => {
                let _ = reply.send(purge_raft_logs_upto(&mut conn, log_idx, &last_purged));
            }
            WriteCmd::ApplyRaftEntries {
                entries,
                last_membership,
                reply,
            } => {
                let _ = reply.send(apply_raft_entries(
                    &mut conn,
                    &entries,
                    last_membership.as_deref(),
                ));
            }
        }
    }
}

fn apply_claim_tx(
    conn: &mut Connection,
    block_idx: u64,
    commitment: &OmegaCommitment,
    claim: ClaimTx,
) -> Result<(), LedgerError> {
    let block_idx_i64 = sqlite_i64("block_idx", block_idx)?;
    let ParsedClaim {
        public_inputs,
        witnesses,
        proof,
    } = parse_claim(claim)?;

    verify(commitment, &public_inputs, &proof)?;

    let tx = conn.transaction()?;
    for public in &public_inputs {
        reject_replay(&tx, public)?;
    }

    for (ordinal, (public, witness)) in public_inputs.iter().zip(witnesses.iter()).enumerate() {
        insert_nullifier(&tx, public, block_idx_i64)?;
        insert_starstream_utxo(
            &tx,
            public,
            witness,
            block_idx,
            ordinal as u64,
            block_idx_i64,
        )?;
    }

    tx.commit()?;
    Ok(())
}

struct ParsedClaim {
    public_inputs: Vec<ClaimPublicInputs>,
    witnesses: Vec<ClaimWitness>,
    proof: ProofBytes,
}

fn parse_claim(claim: ClaimTx) -> Result<ParsedClaim, LedgerError> {
    match claim {
        ClaimTx::Utxo(claim) => Ok(ParsedClaim {
            public_inputs: vec![claim.public],
            witnesses: vec![claim.witness],
            proof: claim.proof,
        }),
        ClaimTx::Collection(claim) => {
            if claim.public.len() != claim.witness.len() {
                return Err(LedgerError::InvalidClaim("collection arity mismatch"));
            }
            Ok(ParsedClaim {
                public_inputs: claim.public,
                witnesses: claim.witness,
                proof: claim.proof,
            })
        }
    }
}

fn reject_replay(conn: &Connection, public: &ClaimPublicInputs) -> Result<(), LedgerError> {
    let leaf_index = sqlite_i64("leaf_index", public.leaf_index)?;
    let existing = conn
        .query_row(
            "SELECT 1 FROM nullifiers WHERE sub_tree_id=?1 AND leaf_index=?2",
            params![i64::from(public.sub_tree_id), leaf_index],
            |_| Ok(()),
        )
        .optional()?;
    if existing.is_some() {
        return Err(LedgerError::Replay {
            sub_tree_id: public.sub_tree_id,
            leaf_index: public.leaf_index,
        });
    }
    Ok(())
}

fn insert_nullifier(
    conn: &Connection,
    public: &ClaimPublicInputs,
    block_idx: i64,
) -> Result<(), LedgerError> {
    let leaf_index = sqlite_i64("leaf_index", public.leaf_index)?;
    conn.execute(
        "INSERT INTO nullifiers (sub_tree_id, leaf_index, block_idx) VALUES (?1, ?2, ?3)",
        params![i64::from(public.sub_tree_id), leaf_index, block_idx],
    )?;
    Ok(())
}

fn insert_starstream_utxo(
    conn: &Connection,
    public: &ClaimPublicInputs,
    witness: &ClaimWitness,
    block_idx: u64,
    ordinal: u64,
    block_idx_i64: i64,
) -> Result<(), LedgerError> {
    let utxo_id = starstream_utxo_id(
        &public.recipient_starstream_addr,
        &witness.leaf_payload,
        block_idx,
        ordinal,
    );
    let value = payload_value(&witness.leaf_payload);
    conn.execute(
        "INSERT INTO starstream_utxos
         (utxo_id, recipient, value, asset_blob, datum, script_ref, block_idx, spent_in)
         VALUES (?1, ?2, ?3, ?4, NULL, NULL, ?5, NULL)",
        params![
            utxo_id.as_slice(),
            public.recipient_starstream_addr.as_slice(),
            sqlite_i64("value", value)?,
            &[] as &[u8],
            block_idx_i64,
        ],
    )?;
    Ok(())
}

fn payload_value(payload: &[u8]) -> u64 {
    // v0.1 synthetic UTxO leaves encode the mock value in bytes 8..16. Short
    // payloads are legal proof inputs, but they do not carry a value field, so
    // the mock-ledger projection emits a zero-value Starstream row.
    if payload.len() < 16 {
        return 0;
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&payload[8..16]);
    u64::from_be_bytes(bytes)
}

fn checkpoint_wal_truncate(conn: &Connection) -> Result<(), LedgerError> {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    Ok(())
}

fn snapshot(
    conn: &Connection,
    db_path: &std::path::Path,
    snapshot_id: &str,
) -> Result<PathBuf, LedgerError> {
    let snapshot_path = db_path.with_file_name(format!("snapshot-{snapshot_id}.sqlite"));
    if snapshot_path.exists() {
        std::fs::remove_file(&snapshot_path).map_err(LedgerError::Io)?;
    }
    conn.execute(
        "VACUUM INTO ?1",
        params![snapshot_path.to_string_lossy().as_ref()],
    )?;
    Ok(snapshot_path)
}

fn restore_snapshot(
    conn: &mut Connection,
    snapshot_path: &std::path::Path,
) -> Result<(), LedgerError> {
    conn.execute(
        "ATTACH DATABASE ?1 AS snapshot_db",
        params![snapshot_path.to_string_lossy().as_ref()],
    )?;
    let tx = conn.transaction()?;
    for table in [
        "raft_log",
        "raft_meta",
        "nullifiers",
        "starstream_utxos",
        "genesis",
    ] {
        // Installing a snapshot replaces the local state image, including
        // raft-log rows after the snapshot index. That is openraft install
        // semantics: the leader will send any required post-snapshot entries
        // after restore. v0.1 uses table-copy instead of drop-and-rename so
        // Windows reader-pool handles can stay alive during the operation.
        tx.execute(&format!("DELETE FROM main.{table}"), [])?;
        tx.execute(
            &format!("INSERT INTO main.{table} SELECT * FROM snapshot_db.{table}"),
            [],
        )?;
    }
    tx.commit()?;
    conn.execute_batch("DETACH DATABASE snapshot_db;")?;
    std::fs::remove_file(snapshot_path).map_err(LedgerError::Io)?;
    Ok(())
}

fn insert_synthetic_claim(
    conn: &mut Connection,
    sub_tree_id: u8,
    leaf_index: u64,
    block_idx: u64,
) -> Result<(), LedgerError> {
    let leaf_index_i64 = sqlite_i64("leaf_index", leaf_index)?;
    let block_idx_i64 = sqlite_i64("block_idx", block_idx)?;
    let recipient = [0xC7; 32];
    let mut payload = Vec::with_capacity(16);
    payload.extend_from_slice(&leaf_index.to_be_bytes());
    payload.extend_from_slice(&block_idx.to_be_bytes());
    let utxo_id = starstream_utxo_id(&recipient, &payload, block_idx, leaf_index);

    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO nullifiers (sub_tree_id, leaf_index, block_idx) VALUES (?1, ?2, ?3)",
        params![i64::from(sub_tree_id), leaf_index_i64, block_idx_i64],
    )?;
    tx.execute(
        "INSERT INTO starstream_utxos
         (utxo_id, recipient, value, asset_blob, datum, script_ref, block_idx, spent_in)
         VALUES (?1, ?2, ?3, ?4, NULL, NULL, ?5, NULL)",
        params![
            utxo_id.as_slice(),
            recipient.as_slice(),
            sqlite_i64("value", leaf_index.saturating_add(1))?,
            &[] as &[u8],
            block_idx_i64,
        ],
    )?;
    tx.commit()?;
    Ok(())
}

fn save_raft_meta(conn: &Connection, key: &str, value: Option<&[u8]>) -> Result<(), LedgerError> {
    match value {
        Some(value) => {
            conn.execute(
                "INSERT INTO raft_meta (k, v) VALUES (?1, ?2)
                 ON CONFLICT(k) DO UPDATE SET v=excluded.v",
                params![key, value],
            )?;
        }
        None => {
            conn.execute("DELETE FROM raft_meta WHERE k=?1", params![key])?;
        }
    }
    Ok(())
}

fn append_raft_logs(conn: &mut Connection, rows: &[RaftLogRow]) -> Result<(), LedgerError> {
    let tx = conn.transaction()?;
    for row in rows {
        tx.execute(
            "INSERT INTO raft_log (log_idx, term, payload) VALUES (?1, ?2, ?3)
             ON CONFLICT(log_idx) DO UPDATE SET term=excluded.term, payload=excluded.payload",
            params![
                sqlite_i64("log_idx", row.log_idx)?,
                sqlite_i64("term", row.term)?,
                row.payload.as_slice(),
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn delete_raft_logs_since(conn: &Connection, log_idx: u64) -> Result<(), LedgerError> {
    conn.execute(
        "DELETE FROM raft_log WHERE log_idx >= ?1",
        params![sqlite_i64("log_idx", log_idx)?],
    )?;
    Ok(())
}

fn purge_raft_logs_upto(
    conn: &mut Connection,
    log_idx: u64,
    last_purged: &[u8],
) -> Result<(), LedgerError> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM raft_log WHERE log_idx <= ?1",
        params![sqlite_i64("log_idx", log_idx)?],
    )?;
    tx.execute(
        "INSERT INTO raft_meta (k, v) VALUES ('last_purged_log_id', ?1)
         ON CONFLICT(k) DO UPDATE SET v=excluded.v",
        params![last_purged],
    )?;
    tx.commit()?;
    Ok(())
}

fn apply_raft_entries(
    conn: &mut Connection,
    entries: &[OmegaRaftEntry],
    last_membership: Option<&[u8]>,
) -> Result<Vec<LedgerResponse>, LedgerError> {
    let mut responses = Vec::with_capacity(entries.len());
    for entry in entries {
        let response = match &entry.payload {
            openraft::EntryPayload::Blank | openraft::EntryPayload::Membership(_) => {
                LedgerResponse::accepted()
            }
            openraft::EntryPayload::Normal(command) => match apply_claim_tx(
                conn,
                command.block_idx,
                &command.commitment,
                command.claim.clone(),
            ) {
                Ok(()) => LedgerResponse::accepted(),
                Err(error) => LedgerResponse::rejected(error.to_string()),
            },
        };
        // A rejected command still advances the state-machine index. The log
        // entry has been applied: the deterministic result is a rejection
        // response rather than a state mutation.
        let last_applied = postcard::to_allocvec(&entry.log_id)
            .map_err(|error| LedgerError::Codec(error.to_string()))?;
        save_raft_meta(conn, "last_applied_log_id", Some(last_applied.as_slice()))?;
        responses.push(response);
    }
    if let Some(last_membership) = last_membership {
        save_raft_meta(conn, "last_membership", Some(last_membership))?;
    }
    Ok(responses)
}
