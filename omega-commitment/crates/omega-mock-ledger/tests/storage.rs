#![allow(missing_docs)]

use std::path::PathBuf;

use omega_claim_prover::OmegaCommitment;
use omega_claim_tx::{ClaimPublicInputs, ClaimTx, ClaimUtxo, ClaimWitness, ProofBytes};
use omega_commitment_core::hash::Hash;
use omega_mock_ledger::{LedgerCommand, MockLedger, MockLedgerStorage, OmegaRaftTypeConfig};
use openraft::{CommittedLeaderId, Entry, EntryPayload, LogId, RaftLogReader, RaftStorage, Vote};

fn temp_db_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-mock-ledger-{name}-{}-{}.sqlite",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}

fn hash(byte: u8) -> Hash {
    [byte; 32]
}

fn sample_command() -> LedgerCommand {
    let commitment = OmegaCommitment {
        bundle_root_blake3: hash(0xA0),
        sub_tree_roots_blake3: [hash(0xB0); 7],
        item_counts: [1, 0, 0, 0, 0, 0, 0],
        leaf_counts: [1, 0, 0, 0, 0, 0, 0],
        tree_depths: [0, 0, 0, 0, 0, 0, 0],
    };
    let claim = ClaimTx::Utxo(ClaimUtxo {
        public: ClaimPublicInputs {
            sub_tree_id: 1,
            leaf_index: 0,
            tree_depth: 0,
            per_sub_tree_root: hash(0xB0),
            bundle_root_blake3: commitment.bundle_root_blake3,
            nullifier: hash(0xC0),
            recipient_starstream_addr: hash(0xD0),
        },
        witness: ClaimWitness {
            leaf_payload: vec![],
            merkle_path: vec![],
            signing_key_proof: vec![],
        },
        proof: ProofBytes(vec![0xE0]),
    });
    LedgerCommand {
        block_idx: 0,
        commitment,
        claim,
    }
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raft_storage_persists_normal_log_entries() {
    let path = temp_db_path("storage-normal");
    let _ = std::fs::remove_file(&path);

    let ledger = MockLedger::open(&path).expect("open ledger");
    let mut storage = MockLedgerStorage::new(ledger);
    let entry = Entry::<OmegaRaftTypeConfig> {
        log_id: LogId::new(CommittedLeaderId::new(7, 1), 5),
        payload: EntryPayload::Normal(sample_command()),
    };

    storage
        .append_to_log([entry.clone()])
        .await
        .expect("append normal log");

    assert_eq!(
        storage
            .try_get_log_entries(5..6)
            .await
            .expect("read normal log entry"),
        vec![entry]
    );
}
