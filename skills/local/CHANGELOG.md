# Skills changelog (skills/local/)

## rust-test pack — v0.1.0 — 2026-05-04

Initial release. 9 skills installed at `c:/experiments/skills/local/`:

- `rust-test-orchestrator` — workflow + classification + report (5-phase, G2-gated)
- `rust-test-proptest` — property tests (Q5)
- `rust-test-cargo-fuzz` — libFuzzer fuzz testing (Q3, S3 with scripts)
- `rust-test-kani` — bounded model checking (Q4, S3 with script)
- `rust-test-shuttle-loom` — concurrency-schedule exploration (Q2)
- `rust-test-failpoints` — failure injection at IO sites (Q1, augmenting)
- `rust-test-turmoil` — Tokio-native distributed simulation (Q1, Tokio-shaped)
- `rust-test-madsim` — non-Tokio distributed simulation (Q1, non-Tokio)
- `rust-test-stateright` — abstract protocol model checking (Q6)

Test gates passed before ship:
- T1: 25/25 positive triggers + 10/10 negative
- T2-F1: leaf_hash_v2 fixture matches expected matrix + invocations + STATUS=GREEN
- T3: F4 Adversary self-test produces STATUS=P0_REGRESSION
- T5: iteration-log.md initialized

Deferred to LoganNet milestone:
- T2-F2 (omega-mock-ledger::WriterActor fixture)
- T2-F3 (omega-toy-consensus::raft_node fixture)

T4 (performance comparison): baseline captured; comparison run scheduled for v0.2.0.
