---
name: rust-test-shuttle-loom
description: Concurrency-schedule exploration for Rust via Shuttle (randomised) or Loom (exhaustive). Use when invoked by rust-test-orchestrator (Q2=yes) or when the user asks to test thread-interleaving bugs, mpsc actor contention, or lock-free data structures. The skill picks Shuttle vs Loom per scenario. Not for distributed protocols (use stateright) or single-threaded property tests (use proptest).
license: Apache-2.0
metadata:
  author: charles
  version: 0.1.0
  pack: rust-test
  invoked-by: rust-test-orchestrator
---

# rust-test-shuttle-loom

## When this skill applies

Q2 (shared mutable state across threads) on the orchestrator's matrix. Concrete shapes:
- mpsc actors receiving from multiple producers
- Lock graphs with `Arc<Mutex>` or `Arc<RwLock>`
- Lock-free data structures using atomics or Crossbeam
- Test cases where the bug only surfaces under a specific schedule

## Picking Shuttle vs Loom

See `references/shuttle-vs-loom.md` for the full decision rule. Quick version:
- **Lock-free data structure (atomics, hand-rolled queues) → Loom.** Exhaustive within a small bound.
- **Lock-shaped concurrency (mpsc actors, Mutex/RwLock graphs) → Shuttle.** Randomised; faster on larger models.
- **Unsure → start with Shuttle.** Switch to Loom if Shuttle finds nothing and the model is small.

## Authoring loop (Shuttle)

1. Identify the property: "for any thread schedule, the actor's invariant holds" (typically: no deadlock, no message loss, idempotence under reorder).
2. Write a `#[test]` that builds the actor + its producers, then run under `shuttle::check_random`.
3. Run: `cargo test -p <crate> --features shuttle <test_name>`.
4. If Shuttle finds a counterexample, it prints the offending schedule. Patch the actor; re-run.
5. Hand back to orchestrator: `shuttle: wrote N tests in <path>; <pass|fail>`.

## Authoring loop (Loom)

1. Identify the property: "for any interleaving of these N atomic operations, the invariant holds".
2. Wrap the data structure under `loom::sync::*` types (loom replaces `std::sync::*`).
3. Write `loom::model(|| { ... })` test.
4. Run: `LOOM_MAX_PREEMPTIONS=3 cargo test -p <crate> --release <test_name>`. (Release mode + bounded preemptions to keep runtime tractable.)
5. Hand back to orchestrator: `loom: wrote N tests in <path>; <pass|fail>`.

## Idioms in this codebase

**Shuttle for the rusqlite WriterActor.** The mpsc-actor that serialises rusqlite writes against openraft's apply path is a Shuttle target. Property: any sequence of (apply, write) interleavings produces a state machine that openraft's snapshot path can reconstruct. See `references/actor-models.md`.

**Loom for any lock-free primitive added to omega-commitment.** Currently none, but if a lock-free queue or RCU-shaped structure lands, Loom is the right tool.

**Don't model the full Raft cluster here.** That's `rust-test-turmoil`'s job. Shuttle/Loom test single-process concurrency; Turmoil tests multi-node behaviour.

## Anti-patterns

- Modelling a multi-node cluster → defer to `rust-test-turmoil` or `rust-test-stateright`.
- Running Loom in debug mode → 10-100x slower; always use `--release`.
- Setting `LOOM_MAX_PREEMPTIONS` higher than 3 without strong reason — exponential blowup.
- Leaving real `std::sync::*` types in a Loom test (Loom only sees its own types).

## Hand-back format

Single line for the orchestrator: `shuttle: wrote N tests in <path>; <pass|fail>` or `loom: wrote N tests in <path>; <pass|fail>`.
