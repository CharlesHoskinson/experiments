use std::path::PathBuf;

use omega_mock_ledger::{MockLedger, MockLedgerStorage, OmegaRaftTypeConfig};
use openraft::{CommittedLeaderId, Entry, EntryPayload, LogId, RaftLogReader, RaftStorage, Vote};

fn temp_db_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-mock-ledger-{name}-{}-{}.sqlite",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raft_storage_persists_vote_and_log_entries() {
    let path = temp_db_path("storage");
    let _ = std::fs::remove_file(&path);

    let ledger = MockLedger::open(&path).expect("open ledger");
    let mut storage = MockLedgerStorage::new(ledger);
    let vote = Vote::new(3, 2);
    storage.save_vote(&vote).await.expect("save vote");

    let entry = Entry::<OmegaRaftTypeConfig> {
        log_id: LogId::new(CommittedLeaderId::new(7, 1), 4),
        payload: EntryPayload::Blank,
    };
    storage
        .append_to_log([entry.clone()])
        .await
        .expect("append log");

    assert_eq!(storage.read_vote().await.expect("read vote"), Some(vote));
    assert_eq!(
        storage
            .get_log_state()
            .await
            .expect("log state")
            .last_log_id,
        Some(entry.log_id)
    );
    assert_eq!(
        storage
            .try_get_log_entries(4..5)
            .await
            .expect("read log entries"),
        vec![entry.clone()]
    );

    drop(storage);
    let reopened = MockLedger::open(&path).expect("reopen ledger");
    let mut storage = MockLedgerStorage::new(reopened);

    assert_eq!(storage.read_vote().await.expect("read vote"), Some(vote));
    assert_eq!(
        storage
            .try_get_log_entries(4..5)
            .await
            .expect("read log entries after restart"),
        vec![entry]
    );
}
