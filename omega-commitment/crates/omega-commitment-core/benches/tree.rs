#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use omega_commitment_core::{hash::blake3_256, tree::MerkleTree};

fn bench_tree_build(c: &mut Criterion) {
    for n in [1_000usize, 10_000, 100_000].iter() {
        let leaves: Vec<_> = (0..*n)
            .map(|i| blake3_256(&(i as u64).to_be_bytes()))
            .collect();
        let mut group = c.benchmark_group("merkle_tree_build");
        group.throughput(Throughput::Elements(*n as u64));
        group.bench_function(format!("n={}", n), |b| {
            b.iter(|| MerkleTree::build(black_box(leaves.clone())))
        });
        group.finish();
    }
}

criterion_group!(benches, bench_tree_build);
criterion_main!(benches);
