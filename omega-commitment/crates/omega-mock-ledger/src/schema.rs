//! SQLite schema and PRAGMA setup for `omega-mock-ledger`.
//!
//! The schema module is public because integration tests and future harness
//! crates need to inspect the exact PRAGMA state. State mutation still flows
//! through [`crate::MockLedger`]'s writer actor.

use std::path::Path;

use rusqlite::Connection;

/// Snapshot of the SQLite PRAGMAs that matter to the mock ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaSnapshot {
    /// Current journal mode, expected to be `wal`.
    pub journal_mode: String,
    /// Current synchronous mode, expected to be `NORMAL` (`1`).
    pub synchronous: i64,
    /// Current page-cache size, expected to be `-65536`.
    pub cache_size: i64,
    /// Current mmap size in bytes, or `0` on Windows where mmap is disabled.
    pub mmap_size: i64,
    /// Current temporary-storage mode, expected to be `MEMORY` (`2`).
    pub temp_store: i64,
    /// Current WAL autocheckpoint page interval.
    pub wal_autocheckpoint: i64,
    /// Current auto-vacuum mode, expected to be `NONE` (`0`).
    pub auto_vacuum: i64,
}

/// Opens a SQLite connection, applies PRAGMAs, and creates the ledger tables.
///
/// # Errors
///
/// Returns the underlying [`rusqlite::Error`] when SQLite cannot open the
/// database, apply the PRAGMAs, or create the required tables.
///
/// # Soundness
///
/// This function establishes the storage layout used by the apply pipeline.
/// The `nullifiers` table uses `(sub_tree_id, leaf_index)` as a composite
/// primary key, and every table is created `WITHOUT ROWID` so SQLite uses the
/// declared key as the B-tree key. If callers mutate the schema after this
/// function returns, replay protection and raft persistence are outside this
/// crate's guarantees.
pub fn open_initialized(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    apply_pragmas(&conn)?;
    create_tables(&conn)?;
    Ok(conn)
}

/// Applies the PRAGMAs required by the proof-harness design.
///
/// # Errors
///
/// Returns [`rusqlite::Error`] when SQLite rejects one of the PRAGMA
/// statements.
///
/// # Soundness
///
/// The WAL and synchronous settings are part of the actor-pattern contract:
/// readers use their own connections while a single writer owns the write
/// connection. `mmap_size` is intentionally skipped on Windows because the
/// restore path needs predictable file-lock behaviour there.
pub fn apply_pragmas(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA cache_size = -65536;
        PRAGMA temp_store = MEMORY;
        PRAGMA wal_autocheckpoint = 10000;
        PRAGMA auto_vacuum = NONE;
        ",
    )?;

    #[cfg(not(windows))]
    conn.execute_batch("PRAGMA mmap_size = 268435456;")?;

    Ok(())
}

fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS raft_log (
          log_idx     INTEGER PRIMARY KEY,
          term        INTEGER NOT NULL,
          payload     BLOB NOT NULL
        ) WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS raft_meta (
          k           TEXT PRIMARY KEY,
          v           BLOB NOT NULL
        ) WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS nullifiers (
          sub_tree_id INTEGER NOT NULL,
          leaf_index  INTEGER NOT NULL,
          block_idx   INTEGER NOT NULL,
          PRIMARY KEY (sub_tree_id, leaf_index)
        ) WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS starstream_utxos (
          utxo_id     BLOB PRIMARY KEY,
          recipient   BLOB NOT NULL,
          value       INTEGER NOT NULL,
          asset_blob  BLOB NOT NULL,
          datum       BLOB,
          script_ref  BLOB,
          block_idx   INTEGER NOT NULL,
          spent_in    INTEGER
        ) WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS genesis (
          k           TEXT PRIMARY KEY,
          v           BLOB NOT NULL
        ) WITHOUT ROWID;
        ",
    )
}

/// Reads back the PRAGMAs that the ledger cares about.
///
/// # Errors
///
/// Returns [`rusqlite::Error`] when SQLite rejects a PRAGMA read, except that
/// `mmap_size` failures are represented as `0` for Windows compatibility.
pub fn pragma_snapshot(conn: &Connection) -> rusqlite::Result<PragmaSnapshot> {
    Ok(PragmaSnapshot {
        journal_mode: pragma_string(conn, "journal_mode")?,
        synchronous: pragma_i64(conn, "synchronous")?,
        cache_size: pragma_i64(conn, "cache_size")?,
        mmap_size: pragma_i64(conn, "mmap_size").unwrap_or(0),
        temp_store: pragma_i64(conn, "temp_store")?,
        wal_autocheckpoint: pragma_i64(conn, "wal_autocheckpoint")?,
        auto_vacuum: pragma_i64(conn, "auto_vacuum")?,
    })
}

fn pragma_i64(conn: &Connection, name: &str) -> rusqlite::Result<i64> {
    conn.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
}

fn pragma_string(conn: &Connection, name: &str) -> rusqlite::Result<String> {
    conn.query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
}
