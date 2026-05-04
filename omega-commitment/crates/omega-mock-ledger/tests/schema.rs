use std::path::PathBuf;

use omega_mock_ledger::MockLedger;

fn temp_db_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-mock-ledger-{name}-{}-{}.sqlite",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}

#[test]
fn schema_initializes_wal_pragmas_and_required_tables() {
    let path = temp_db_path("schema");
    let _ = std::fs::remove_file(&path);

    let ledger = MockLedger::open(&path).expect("open ledger");
    let pragmas = ledger.pragma_snapshot().expect("pragma snapshot");

    assert_eq!(pragmas.journal_mode, "wal");
    assert_eq!(pragmas.synchronous, 1);
    assert_eq!(pragmas.cache_size, -65_536);
    assert_eq!(pragmas.temp_store, 2);
    assert_eq!(pragmas.wal_autocheckpoint, 10_000);
    assert_eq!(pragmas.auto_vacuum, 0);

    for table in [
        "raft_log",
        "raft_meta",
        "nullifiers",
        "starstream_utxos",
        "genesis",
    ] {
        assert!(ledger.table_exists(table).expect("table query"), "{table}");
    }
}
