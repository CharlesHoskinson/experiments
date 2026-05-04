use std::path::Path;

use rusqlite::Connection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PragmaSnapshot {
    pub journal_mode: String,
    pub synchronous: i64,
    pub cache_size: i64,
    pub mmap_size: i64,
    pub temp_store: i64,
    pub wal_autocheckpoint: i64,
    pub auto_vacuum: i64,
}

pub fn open_initialized(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    apply_pragmas(&conn)?;
    create_tables(&conn)?;
    Ok(conn)
}

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
