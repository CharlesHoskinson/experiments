# Capability matrix — the 6 questions

Each question is yes/no. Each "yes" routes to a per-framework skill. Each "no" is recorded with a one-line skip rationale so the test plan shows *what was deliberately skipped, and why*.

## Q1 — Network/IO?

Does this code touch network sockets, the filesystem, or async I/O?

- Concrete signals: imports `tokio::net`, `tokio::fs`, `std::net`, `std::fs`, `libp2p`, `pallas-network`; uses `async fn`; depends on `openraft`, `rusqlite`, `r2d2`.
- Yes → `rust-test-turmoil` (if Tokio-shaped), `rust-test-madsim` (if non-Tokio), and `rust-test-failpoints` (always — inject failures at IO sites).
- No → skip all three.

## Q2 — Shared mutable state?

Does the code share mutable state across threads (Arc<Mutex>, Arc<RwLock>, channels, atomics)?

- Concrete signals: `Arc<Mutex<...>>`, `Arc<RwLock<...>>`, `mpsc::channel`, `tokio::sync::Mutex`, `AtomicUsize`, `crossbeam`.
- Yes → `rust-test-shuttle-loom`. Pick Shuttle for fast randomised exploration; pick Loom for exhaustive small-scope models. The skill itself decides per scenario.
- No → skip.

## Q3 — Parses untrusted bytes?

Does the code parse, decode, or otherwise process bytes that could be attacker-controlled?

- Concrete signals: `pub fn` taking `&[u8]`, `Vec<u8>`, `BufRead`, `Read`; CBOR/JSON/NDJSON deserialisation; protocol message decoders.
- Yes → `rust-test-cargo-fuzz`.
- No → skip.

## Q4 — Bounded invariant?

Does the code have a small, bounded input space and a clear safety invariant that should hold for *all* inputs in that space?

- Concrete signals: pure functions over fixed-width integers, small enums, bounded structs (≤ a few hundred bytes); commutative/associative properties; collision-resistance claims; monotonicity claims.
- Yes → `rust-test-kani`. Pin the bound in the skill's invocation; if the bound exceeds Kani's tractable range, fall back to proptest.
- No → skip.

## Q5 — Random-input property?

Does the code admit a property that should hold over many random inputs (round-trips, codec inverses, monotonic state machines, idempotence)?

- Concrete signals: `serialize` + `deserialize` pair; `apply` + `revert` pair; `encode` + `decode` pair; ordering/commutativity claims.
- Yes → `rust-test-proptest`.
- No → skip.

## Q6 — Distributed protocol?

Is this an abstract protocol with N parties whose interleavings matter, where you need to check safety / liveness properties over the protocol model rather than the concrete bytes?

- Concrete signals: consensus algorithms, leader election, threshold-encryption committees, multi-party computation, replication protocols.
- Yes → `rust-test-stateright`.
- No → skip.

## Output

Phase 2 emits a matrix block exactly like this (see `decision-examples.md` for filled-in examples):

```
target: <fully-qualified-path>
ground: <one-line summary of Phase 1 grounding>
matrix:
  Q1 network/IO?           → yes/no  → <route or skip>
  Q2 shared mut state?     → yes/no  → <route or skip>
  Q3 untrusted bytes?      → yes/no  → <route or skip>
  Q4 bounded invariant?    → yes/no  → <route or skip>
  Q5 random-input prop?    → yes/no  → <route or skip>
  Q6 protocol w/ N parties? → yes/no  → <route or skip>
soundness-negative cases planned: N
  - <one-line description per case>
```
