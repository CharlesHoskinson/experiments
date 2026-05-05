---
name: rust-test-turmoil
description: Deterministic Tokio-native distributed-system simulation via Turmoil. Use when invoked by rust-test-orchestrator (Q1=yes, Tokio-shaped) or when the user asks to test the LoganNet 3-node Raft cluster, libp2p network behaviour, or partition / latency / packet-loss scenarios. Closest Rust-native match for Antithesis-style multiverse testing in scope, with Tokio-only constraint. Not for non-Tokio code (use madsim) or single-process actors (use shuttle-loom).
license: Apache-2.0
metadata:
  author: charles
  version: 0.1.0
  pack: rust-test
  invoked-by: rust-test-orchestrator
---

# rust-test-turmoil

## When this skill applies

Q1 (network/IO) on the orchestrator's matrix, when the target is Tokio-native and tests should drive multi-node interactions. Concrete shapes:
- 3-node openraft cluster (LoganNet)
- libp2p mesh with controlled partitions
- Multi-client + multi-server protocols
- Time-dependent state machines (timeouts, leases)

## Authoring loop

1. Identify the topology: how many nodes, what protocol(s) between them.
2. Define a `turmoil::Builder` setup with each node as a `host`.
3. Write a test that drives the simulation: send messages, observe state, inject faults.
4. Run: `cargo test -p <crate> --features turmoil <test_name>`.
5. Determinism check: re-run with the same seed; assert byte-identical output.
6. Hand back to orchestrator: `turmoil: wrote N tests in <path>; <pass|fail>`.

## Idioms in this codebase

**3-node LoganNet topology fixture.** See `references/loganet-patterns.md` for the canonical 3-node Raft + libp2p topology fixture used across all LoganNet tests.

**Partition mid-leader-election.** Partition node 1 from {2, 3} after node 1 sends RequestVote but before responses return. Property: a single leader still emerges.

**Snapshot-mid-leader-change.** Force node 2 to install a snapshot while a leader change is in flight. Property: applied state matches the snapshot's committed state.

**Restart durability.** Stop a node, restart it, assert it catches up via Raft AppendEntries without state corruption.

## Anti-patterns

- Single-process unit tests under Turmoil — no value, use plain Tokio.
- Non-Tokio code under Turmoil — Turmoil only intercepts Tokio's runtime; non-Tokio code runs unintercepted (and tests become non-deterministic).
- Using `std::time::Instant` instead of `tokio::time::Instant` — Turmoil controls the clock through Tokio.
- Forgetting `#[turmoil::test]` (uses `#[tokio::test]` internally with the Turmoil runtime).

## Hand-back format

Single line for the orchestrator: `turmoil: wrote N tests in <path>; <pass|fail>`.
