#![allow(missing_docs)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use omega_mock_ledger::MockLedger;

fn temp_db_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-mock-ledger-{name}-{}-{}.sqlite",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}

fn p99_micros(samples: &mut [u128]) -> u128 {
    samples.sort_unstable();
    let idx = samples.len().saturating_mul(99) / 100;
    samples[idx.min(samples.len().saturating_sub(1))]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_readers_do_not_block_actor_writes() {
    let path = temp_db_path("concurrent-readers");
    let _ = std::fs::remove_file(&path);
    let ledger = MockLedger::open(&path).expect("open ledger");
    let deadline = Instant::now() + Duration::from_secs(5);

    let mut readers = Vec::new();
    for _ in 0..16 {
        let ledger = ledger.clone();
        readers.push(tokio::spawn(async move {
            while Instant::now() < deadline {
                ledger.nullifier_count().await.expect("reader count");
            }
        }));
    }

    let mut write_latencies = Vec::new();
    let mut leaf_index = 0u64;
    while Instant::now() < deadline {
        let started = Instant::now();
        ledger
            .insert_synthetic_claim_for_test(1, leaf_index, 7)
            .await
            .expect("synthetic actor write");
        write_latencies.push(started.elapsed().as_micros());
        leaf_index += 1;
    }

    for reader in readers {
        reader.await.expect("reader task");
    }

    assert!(!write_latencies.is_empty());
    assert!(
        p99_micros(&mut write_latencies) < 50_000,
        "writer p99 exceeded 50 ms"
    );
}
