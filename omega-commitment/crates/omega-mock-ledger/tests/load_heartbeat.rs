#![allow(missing_docs)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use omega_mock_ledger::MockLedger;

fn temp_db_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-mock-ledger-{name}-{}-{}.sqlite",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn heartbeat_ticker_keeps_moving_during_submit_storm() {
    let path = temp_db_path("heartbeat");
    let _ = std::fs::remove_file(&path);
    let ledger = MockLedger::open(&path).expect("open ledger");
    let deadline = Instant::now() + Duration::from_secs(60);
    let ticks = Arc::new(AtomicU64::new(0));
    let max_gap_ms = Arc::new(AtomicU64::new(0));

    let heartbeat_ticks = ticks.clone();
    let heartbeat_gap = max_gap_ms.clone();
    let heartbeat = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_millis(250));
        let mut last = Instant::now();
        while Instant::now() < deadline {
            ticker.tick().await;
            let gap = last.elapsed().as_millis() as u64;
            heartbeat_gap.fetch_max(gap, Ordering::Relaxed);
            heartbeat_ticks.fetch_add(1, Ordering::Relaxed);
            last = Instant::now();
        }
    });

    let writer = tokio::spawn(async move {
        let mut leaf_index = 0u64;
        while Instant::now() < deadline {
            ledger
                .insert_synthetic_claim_for_test(2, leaf_index, 11)
                .await
                .expect("synthetic actor write");
            leaf_index += 1;
        }
    });

    heartbeat.await.expect("heartbeat task");
    writer.await.expect("writer task");

    assert!(ticks.load(Ordering::Relaxed) >= 200);
    assert!(
        max_gap_ms.load(Ordering::Relaxed) < 1_000,
        "heartbeat gap reached election-timeout territory"
    );
}
