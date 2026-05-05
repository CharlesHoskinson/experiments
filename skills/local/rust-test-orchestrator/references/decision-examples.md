# Decision examples — three worked classifications

Three worked examples from `c:\experiments\omega-commitment\`. Use these as templates when emitting a Phase 2 matrix.

## Example 1 — pure cryptographic primitive

```
target: omega-commitment-core::leaf_hash_v2
ground: blake3=yes, no async, no Arc<Mutex>, takes &[u8] payload, pub fn
matrix:
  Q1 network/IO?           → no    → skip turmoil, madsim, failpoints
  Q2 shared mut state?     → no    → skip shuttle-loom
  Q3 untrusted bytes?      → yes   → invoke rust-test-cargo-fuzz
  Q4 bounded invariant?    → yes   → invoke rust-test-kani  (bound: payload_len ≤ 4KB)
  Q5 random-input prop?    → yes   → invoke rust-test-proptest
  Q6 protocol w/ N parties? → no    → skip stateright
soundness-negative cases planned: 3
  - leaf-as-internal-node second-preimage swap (must reject)
  - EMPTY_INDEX_SENTINEL forgery at idx >= item_count (must reject)
  - leaf with idx >= u32::MAX (must reject as out-of-bounds)
```

## Example 2 — single-process actor

```
target: omega-mock-ledger::WriterActor
ground: tokio=yes, rusqlite=yes, mpsc::channel inside actor, no network
matrix:
  Q1 network/IO?           → yes (fs only) → invoke rust-test-failpoints (skip turmoil/madsim — no network)
  Q2 shared mut state?     → yes  → invoke rust-test-shuttle-loom (Shuttle for actor contention)
  Q3 untrusted bytes?      → no   → skip cargo-fuzz
  Q4 bounded invariant?    → no   → skip kani (state machine too large)
  Q5 random-input prop?    → yes  → invoke rust-test-proptest (apply/revert idempotence)
  Q6 protocol w/ N parties? → no   → skip stateright
soundness-negative cases planned: 1
  - apply same tx twice → second must error (no double-spend in single actor)
```

## Example 3 — distributed cluster

```
target: omega-toy-consensus::raft_node (planned, not yet implemented)
ground: tokio=yes, libp2p=yes, openraft=yes, multi-process via TCP
matrix:
  Q1 network/IO?           → yes  → invoke rust-test-turmoil (Tokio-native), rust-test-failpoints (network drops)
  Q2 shared mut state?     → yes  → invoke rust-test-shuttle-loom (mpsc-actor inside each node)
  Q3 untrusted bytes?      → yes  → invoke rust-test-cargo-fuzz (libp2p protocol decoder)
  Q4 bounded invariant?    → no   → skip kani (state space too large)
  Q5 random-input prop?    → yes  → invoke rust-test-proptest (state-machine commands)
  Q6 protocol w/ N parties? → yes → invoke rust-test-stateright (abstract Raft model)
soundness-negative cases planned: 4
  - two leaders elected in same term (must never happen — safety)
  - committed entry forgotten after partition heal (must never happen — durability)
  - apply called out of order on a follower (must never happen — order preservation)
  - snapshot install during leader change (must succeed — robustness)
```

## How to choose between turmoil and madsim

Both are deterministic distributed simulators. The decision rule:

- If the target uses `tokio::*` directly → turmoil.
- If the target uses `madsim::*` shims → madsim.
- If the target is new code where the choice is open → turmoil. (Lighter, more idiomatic, smaller blast radius.)

## How to choose between Shuttle and Loom

Both explore thread schedules. The decision rule, embedded inside `rust-test-shuttle-loom`:

- Lock-free data structure (atomics, hand-rolled queues) → Loom (exhaustive within a small bound).
- Lock-shaped concurrency (mpsc actors, Mutex/RwLock graphs) → Shuttle (randomised; faster on larger models).
- If unsure → start with Shuttle.
