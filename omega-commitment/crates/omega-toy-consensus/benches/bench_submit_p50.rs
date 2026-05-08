//! Single-claim submit latency on a single-node localhost cluster.

#![allow(missing_docs)]

use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use jsonrpsee::core::client::ClientT;

fn bench_submit_single_claim(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut cfg = omega_toy_consensus::NodeConfig::single_node_localhost(1).unwrap();
    cfg.data_dir = tempfile::tempdir().unwrap().keep();
    cfg.rpc.bind = "127.0.0.1:18001".parse().unwrap();
    cfg.rpc.max_request_bytes = 16 * 1024 * 1024;
    cfg.apply_deadline = Duration::from_secs(3_600);

    let _handle = runtime.block_on(omega_toy_consensus::start(cfg)).unwrap();
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build("http://127.0.0.1:18001")
        .unwrap();
    runtime.block_on(tokio::time::sleep(Duration::from_secs(3)));

    let mut leaf = 0u64;
    let mut group = c.benchmark_group("submit_claim");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(20));
    group.bench_function("single_claim_localhost", |b| {
        b.iter(|| {
            let claim =
                omega_claim_prover::test_fixtures::build_synthetic_accepted_claim(leaf % 256);
            leaf = leaf.wrapping_add(1);
            let mut params = jsonrpsee::core::params::ObjectParams::new();
            params.insert("claim", claim).unwrap();
            let outcome: omega_toy_consensus::SubmitOutcome = runtime
                .block_on(client.request("omega_submitClaim", params))
                .unwrap();
            criterion::black_box(outcome);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_submit_single_claim);
criterion_main!(benches);
