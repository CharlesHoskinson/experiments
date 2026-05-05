---
name: rust-test-failpoints
description: Failure injection at storage and network sites via fail-rs. Use when invoked by rust-test-orchestrator (Q1=yes) or when the user asks to test error paths that are hard to trigger naturally — disk-full, network partition, fsync failure, partial writes. Augments integration tests; not a replacement for them.
license: Apache-2.0
metadata:
  author: charles
  version: 0.1.0
  pack: rust-test
  invoked-by: rust-test-orchestrator
---

# rust-test-failpoints

## When this skill applies

Q1 (network/IO) on the orchestrator's matrix, when the user wants to exercise error paths. Concrete shapes:
- WAL fsync failures
- libp2p send failures (peer down, connection reset)
- rusqlite SQLITE_BUSY contention
- snapshot read failures (file truncated, checksum mismatch)
- Mempool decrypt failures (committee threshold not met)

## Authoring loop

1. Identify the failure path you want to exercise (e.g., "what happens if WAL fsync returns ENOSPC mid-apply?").
2. Plant a `fail::fail_point!` at the relevant site in production code (under `cfg(feature = "failpoints")`).
3. Write a test that activates the failpoint with a probability or counter, then asserts the recovery behaviour.
4. Run: `cargo test -p <crate> --features failpoints <test_name>`.
5. Hand back to orchestrator: `failpoints: wrote N tests in <path>; <pass|fail>`.

## Idioms in this codebase

**Failpoint placement.** Plant points at:
- WAL fsync sites (rusqlite WAL truncate cron)
- libp2p `send` sites (per-peer)
- Snapshot read paths (Mithril verification, NDJSON streaming reader)
- Mempool decrypt path (threshold-encryption committee)

See `references/injection-sites.md` for the canonical site list.

**Probabilistic vs deterministic activation.** Prefer deterministic for unit tests (`cfg.activate("wal_fsync", "return(ENOSPC)")`). Use probabilistic only for stress-style runs.

**Soundness-negative tests under failpoints.** When a failpoint causes a partial write, the recovery path must NEVER mark the partial state as committed. Adversary class: "partial write recovered as committed" → P0_REGRESSION.

## Anti-patterns

- Using failpoints in production code paths without `cfg(feature = "failpoints")` — slows down release builds.
- Replacing actual integration tests with failpoint tests — failpoints inject *additional* coverage, not replacement.
- Forgetting to deactivate failpoints between tests — state leaks.
- Activating with `panic!()` when the production code does not have a panic recovery path — the test crashes instead of testing.

## Hand-back format

Single line for the orchestrator: `failpoints: wrote N tests in <path>; <pass|fail>`.
