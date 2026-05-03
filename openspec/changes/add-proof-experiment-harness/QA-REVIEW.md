# QA Review: add-proof-experiment-harness

Critical review of `proposal.md`, `design.md`, `specs/proof-harness/spec.md`, and `tasks.md` as of 2026-05-03. Read-only — no artifacts modified.

Severity legend: **P0** (blocker / soundness / "this won't compile or run"), **P1** (correctness or scope risk that will bite during implementation), **P2** (clarity, documentation, or polish).

---

## Strengths

These are genuinely correct, load-bearing items the design got right. Not just "nice things"; they are decisions that, if wrong, would have eaten weeks.

- **Pin scope to membership-only at v0.1** (design.md:301-307, spec.md:21). Restricting the prover to C1-C5 (Merkle membership only) and explicitly deferring C6 (PQ signature) and C7 (PLUME nullifier) is the single most important design choice in the doc. SLH-DSA verification inside an AIR is the dominant cost in the full claim circuit per ARCHITECTURE.md and the e2e plan; trying to land it in the same change as the openraft+libp2p+rusqlite plumbing would have been a multi-month yak shave. The deferral is named, not glossed.
- **Acknowledging the rusqlite-sync vs openraft-async impedance mismatch** (design.md:280, tasks.md:36-37). This is the kind of integration hazard that takes a week to debug from a Raft-leader-flapping symptom; calling it out explicitly with a `tokio::task::spawn_blocking` mandate and a heartbeat-continuity test is the right defence.
- **Snapshot chunking for libp2p `request_response`** (design.md:232-234, spec.md:73, tasks.md:48). Correctly identifies that the 10 MiB default cap on `request_response` payloads collides with Raft's `InstallSnapshot` RPC. Most prototype designs miss this and discover it at the first follower-reattach. The 1 MiB chunk size with `(snapshot_id, chunk_idx, total_chunks)` indexing is the conventional fix.
- **Reframing dual-hash as drift-detection** stays consistent (proposal.md inherits ARCHITECTURE.md:9, blake3-migration-design.md:22). The harness does not reintroduce the "break-hedge" framing the audit closed in F004.
- **`v0` topic name** for gossipsub (`omega-claims-v0`, design.md:231). Versioning the topic from day one is the right hygiene; topic-rename migrations on libp2p networks are painful.
- **Pinning `pallas-validate` to `1.0.0-alpha.6` behind a default-off feature flag** (proposal.md:19, spec.md:88-94). Alpha-version dep behind a feature gate is the right way to take an alpha dep into a workspace that has to keep building.
- **WITHOUT ROWID + composite PK on `nullifiers`** (design.md:165-169). Correct schema choice for a (sub_tree_id, leaf_index) lookup-heavy table; saves a level of indirection vs the implicit rowid index.

---

## Challenges

### P0 — openraft 0.9 storage API is *async*, but rusqlite is *sync*; the design's "spawn_blocking" mitigation is necessary but not sufficient

**Where**: design.md:280, design.md:188-192, tasks.md:34-37, spec.md:53.

**Claim under review**: "rusqlite is sync; openraft is async. Every state-machine call is wrapped in `tokio::task::spawn_blocking` to avoid stalling the runtime."

**Problem**: openraft's `RaftStorage` and `RaftStateMachine` traits (since 0.9 the split-storage refactor) require returning futures. `spawn_blocking` returns a `JoinHandle<T>` that yields its `T` only after the blocking work completes, but openraft expects the *future itself* to be cancel-safe and to make progress under back-pressure. The naive pattern `spawn_blocking(|| conn.execute(...)).await?` works for individual calls but does **not** automatically pipeline writes; if the openraft worker awaits a long blocking insert, log-append latency rises directly with SQLite write latency. Worse, `spawn_blocking` runs on the blocking pool (default 512 threads) — if every state-machine call grabs one, contention with rusqlite's single-`Connection` write serialisation produces a thundering-herd against the WAL.

The design says "single dedicated writer task" (design.md:189) but does not show how that writer task interacts with the openraft `apply` future contract. The correct pattern is an `mpsc` channel from the openraft worker to a dedicated thread that owns the `Connection`, with the apply future awaiting a `oneshot` reply — *not* `spawn_blocking` per call. The current design conflates these.

**Impact**: heartbeat misses under load (the failure mode the design itself flagged); leader churn; the e2e test in design.md:289-298 will pass but `bench --leaves 100` will degrade non-linearly.

**Recommendation**: Make the actor pattern explicit in design.md and tasks.md. Replace "All `INSERT` / `UPDATE` calls go through `tokio::task::spawn_blocking`" with "All writes go through a single `mpsc` channel to a dedicated OS thread that owns the `Connection`; readers borrow short-lived `Connection`s from a `r2d2_sqlite` pool." Validate against openraft's actual `RaftStateMachine::apply` signature in the version pinned.

---

### P0 — "Pin to upstream HEAD" for openraft + libp2p + Plonky3 is a reproducibility hazard

**Where**: proposal.md:20.

**Claim under review**: "Pin to upstream HEAD for Plonky3, openraft, libp2p."

**Problem**: HEAD is not a pin. A `cargo update` six weeks from now will pull a different commit and the goldens will drift. The e2e plan (omega-testnet-e2e-plan.md:319-327) and the prover task (tasks.md:16) actually say "git tag pin" / "rev = `<pinned>`", which is correct. The proposal contradicts that.

**Impact**: Tagged release in three months that no longer reproduces; CI passes on Tuesday and fails on Wednesday because Plonky3 master rebased; "works on my machine" against a developer who happened to clone earlier.

**Recommendation**: Reword proposal.md:20 to read "Pin to a recorded `rev = <hash>` for Plonky3, openraft, and libp2p, captured in `Cargo.toml`. Bump the rev deliberately, never on `cargo update`." Make this an explicit task gate.

---

### P0 — libp2p version "1.x" is not a thing; the actual current major is 0.5x

**Where**: proposal.md:35, tasks.md:46.

**Claim under review**: "libp2p (1.x; full default protocols)" / "libp2p 1.x".

**Problem**: rust-libp2p as of mid-2026 is around the 0.5x — 0.7x major series; a 1.0 release has been on the roadmap for years and is not yet shipped on crates.io as of the cutoff. Specifying "1.x" produces an empty version-resolver result. This will fail `cargo build` on the very first CI run.

**Recommendation**: Pin the actual current libp2p version (likely `0.55` or `0.56` depending on what's current at implementation time) and record the version explicitly in the workspace `Cargo.toml`. Same hazard for "openraft 1.x" (proposal.md:35) — openraft is `0.9.x` series; a `1.0` is not yet released. Update both proposal.md and tasks.md to use real version numbers.

---

### P1 — Plonky3 `p3-blake3-air` proves Blake3 *permutations*, not Blake3 *hashes*; the C1/C3 mapping is hand-waved

**Where**: spec.md:21 ("Blake3 hashing inside the AIR (matching `p3-blake3-air`)"), tasks.md:18 ("delegates to `p3-blake3-air` via permutation argument"), omega-testnet-e2e-plan.md:344, blake3-migration-design.md:23 and :132.

**Claim under review**: C1 (leaf hash) and C3 (Merkle node hash) "compile directly without a custom AIR" against `p3-blake3-air`.

**Problem**: `p3-blake3-air` constrains the Blake3 *compression function* (or permutation), not the full Blake3 hash including the chunk/tree mode, finalization flags, key flag, derive-key flag, and length-prefix block. A real Blake3 hash of the leaf preimage `"omega:v2:leaf" || sub_tree_id || canonical_index_be || payload_len_be || payload` runs through:

1. Splitting input into 1024-byte chunks.
2. Per-chunk Merkle-style chaining (Blake3's tree mode).
3. Finalization with the ROOT flag set on the last compression.
4. Length encoding into the final block.

If the AIR only constrains the inner compression and the prover does the chunk/finalization plumbing in Rust, then the **soundness boundary** is at "the prover claims this compression input was derived correctly from the public preimage." That's an off-circuit gluing step. For a leaf payload of ≤ 64 bytes (the small fixed-arity sub-trees) the gluing is trivial, but for variable-length UTxO payloads (≥ 81 bytes, can grow into multi-block territory with native-asset bundles) the gluing is *not* trivial and is exactly where soundness bugs live.

The same concern applies to node hashes if the design ever needs a node-hash preimage that exceeds one compression block (it doesn't — `tag || left || right` is 13 + 32 + 32 = 77 bytes which fits in two compressions, but the design should say so explicitly).

**Impact**: A reviewer asking "what does the verifier actually check" gets back "p3-blake3-air did the compression"; the question of whether the *leaf preimage* and the *node preimage* were correctly fed into that compression is unaddressed. For v0.1 prototype against synthetic ≤ 64-byte fixtures this is fine, but the spec writes as though it's also true for real UTxO leaves, and it isn't.

**Recommendation**: Add a constraint to the spec: "C1 SHALL be discharged by (a) constraining `p3-blake3-air` over each compression block, AND (b) a separate AIR or constraint set that asserts the compression block inputs were correctly derived from the public leaf preimage including the `omega:v2:leaf` domain tag and the `payload_len_be` length prefix." Either implement both, or restrict the v0.1 prototype to leaf payloads ≤ 64 bytes and say so in spec.md as an explicit limitation. Cross-reference the `plonky3-friendly-rust` skill's "gap between works in Rust and constrains cheaply in a STARK" point.

---

### P1 — `pallas-validate::phase1::validate_tx` API claim is unverified

**Where**: proposal.md:19, design.md:138, spec.md:88-101, tasks.md:42.

**Claim under review**: "`pallas_validate::phase1::validate_tx` against a configured `Environment`, `UTxOs`, and `CertState`".

**Problem**: `pallas-validate` 1.0.0-alpha.6 is alpha-versioned and the `phase1::validate_tx` function signature, the `Environment` / `UTxOs` / `CertState` types, and per-era enum variants are subject to alpha churn. The change does not show what version of these types is being targeted, does not pin a specific commit, and the task list (tasks.md:42) describes the validation behaviour as if the API is stable. The proposal claims "phase1 only, per-era: Byron / Shelley_MA / Alonzo / Babbage / Conway" — this enumeration matches `MultiEraTx` variants in `pallas-traverse`, but whether `phase1::validate_tx` actually takes all five eras through one function or requires per-era dispatch is not verified in the proposal.

**Impact**: The `cardano-tx-validation` feature compiles today and breaks at the next `cargo update`; the test in tasks.md:43 (cardano_phase1.rs) is brittle.

**Recommendation**: Either (a) before merging, run `cargo doc --open -p pallas-validate` or read the `pallas-validate` source at the pinned version and quote the actual function signature into design.md, or (b) drop the Cardano-validation feature from this change entirely and put it in a follow-up change. The harness goal (proof round-trip on a developer laptop) does not require it.

---

### P1 — openraft "fixed 3-node membership" plus mDNS plus libp2p `MemoryTransport` for tests creates two distinct network paths to maintain

**Where**: design.md:215, tasks.md:50, design.md:289-298.

**Claim under review**: The integration test uses `libp2p::core::transport::MemoryTransport`; the production path uses TCP + Noise + Yamux + mDNS.

**Problem**: openraft + libp2p has subtle ordering issues that surface only when actual TCP sockets are involved (TCP handshake delay, Noise XX handshake completing after Raft has already started election timer, mDNS multicast jitter on Linux vs Windows). A test that runs against `MemoryTransport` does not exercise these; the design.md:283 "mDNS in CI" failure mode acknowledges one part but the test plan only covers the in-memory path.

**Impact**: e2e.rs passes; the first time someone runs three-node-quorum on a real machine it fails to elect a leader because Noise handshakes take 200 ms and the election timeout is 1 s on a busy laptop.

**Recommendation**: Add a second integration test using actual TCP loopback (`127.0.0.1:{4001,4002,4003}`) gated behind `RUN_TCP_HARNESS=1`. The CI pipeline should run it on at least one Linux runner.

---

### P1 — Snapshot chunking via `request_response` lacks a flow-control story

**Where**: design.md:232-234, spec.md:73, tasks.md:48.

**Claim under review**: "We split the snapshot file into 1 MiB chunks, transfer chunks as separate `request_response` calls indexed by `(snapshot_id, chunk_idx, total_chunks)`, and reassemble on the receiver."

**Problem**: `request_response` is, by default, *unordered* and *not flow-controlled*. If the leader fires all chunks at once, the follower's libp2p stream multiplexer (Yamux) buffers them; with a GiB-scale snapshot at 1 MiB chunks that's 1024+ in-flight requests, easily exceeding Yamux's default per-connection window. The design does not specify:

- Whether chunks are sent serially or concurrently.
- What the receiver does if chunks arrive out of order (it must, given `request_response` parallelism).
- Whether there is a `(snapshot_id, chunk_count)` advertisement before chunk transfer or whether the receiver discovers `total_chunks` from chunk 0.
- Backpressure: what happens if the receiver disk-write rate is slower than the network rate.
- Resumption: if the leader steps down mid-snapshot, does the new leader restart chunk 0 or pick up where the old one left off? (openraft's snapshot model says restart, but the chunking layer must respect that.)

**Impact**: First production-shaped follower lag-out either OOMs the receiver (buffered chunks) or stalls indefinitely (lost chunk, no retry). Either is a debug nightmare.

**Recommendation**: Specify the wire protocol explicitly: chunks transferred serially in chunk-index order, single in-flight request at a time, receiver writes to disk synchronously before requesting next chunk, snapshot session aborts on any leader-change event. Add a scenario to spec.md exercising a snapshot-resume after a simulated mid-transfer leader change.

---

### P1 — gossipsub for claim broadcast collides with Raft's authoritative ordering

**Where**: design.md:231, tasks.md:49.

**Claim under review**: "gossipsub topic `omega-claims-v0` for client-side claim broadcast. Mock ledger deduplicates by content hash."

**Problem**: This is two paths into the ledger: (a) JSON-RPC `submit` to the leader (design.md:241), which goes through Raft, and (b) gossipsub broadcast to all three nodes. Followers that receive a gossipsub claim cannot apply it directly — only the leader can. So the gossipsub path is, at best, a "pre-broadcast" optimization that lets followers cache the proof bytes before the leader's `AppendEntries` arrives. At worst it's dead code, because Raft's `AppendEntries` already carries the entry payload to every follower.

If the design intent is "gossipsub lets a non-leader-knowing client broadcast and have *some* node forward to the leader," that's reasonable but unstated. If the intent is "gossipsub is the broadcast layer and Raft is the consensus layer," that's a two-level broadcast with no semantic gain on a 3-node cluster.

**Impact**: Wasted complexity; possible double-apply bugs if dedup-by-content-hash fails; one more libp2p protocol to debug.

**Recommendation**: Either drop gossipsub from v0.1 (`request_response` is sufficient for client→leader and leader→followers) or add a one-paragraph justification in design.md naming exactly the use case it enables that `request_response` does not.

---

### P1 — `EnumName` mismatch between `omega-experiment bench` performance gates and Plonky3 example baseline

**Where**: spec.md:25-29, tasks.md:106 ("prove p50 < 30 s and submit p50 < 500 ms"), omega-testnet-e2e-plan.md:262-280 (Plonky3 example, 1024 Blake3 perms in 3.43 s).

**Claim under review**: "the call returns `Ok(proof_bytes)` within 30 seconds on a developer laptop" for a single-leaf prove.

**Problem**: The 30-second budget is set against a single-leaf prove. The Plonky3 spike (omega-testnet-e2e-plan.md:262-280) measured 1024 Blake3 *permutations* in 3.43 s using the upstream example config. A single Merkle path through a 2^20-leaf tree at depth 20 with two Blake3 compressions per node ≈ 40 compressions = ~80 permutations. Naively that's ~0.3 s of prover time *for the compressions alone*, which leaves a 30-second budget for the rest of the AIR. That sounds generous, but the harness's prover also has to:

- Run the leaf-preimage gluing AIR (P1 challenge above).
- Commit to the full trace (the 79% commit-to-trace cost in the spike).
- Run FRI.

If the AIR adds many columns for the gluing logic, the "commit-to-trace" phase scales with `trace_rows × trace_cols`. A 30-second target for a single leaf is plausible but not free; it depends on AIR size which is not specified.

The 1024-leaf budget (spec.md:28-29) is also under-specified: 1024 leaves × 20-deep Merkle path × ~2 compressions per step = ~40,960 compressions, which from the spike's 1024 perms in 3.43 s extrapolates to ~140 s of compression-only time, before commit/FRI overhead. The "32 MiB proof bytes" cap (spec.md:29) is tight: the spike showed 4.9 MiB for 1024 *single* compressions; for 40,960 compressions plus the membership AIR plus the leaf-preimage AIR, 32 MiB may be the wrong number.

**Impact**: Acceptance test 12.5 may fail on a developer laptop and the team will spend a week tuning FRI rate / queries / blowup factor instead of shipping the harness.

**Recommendation**: Either (a) lower the 1024-leaf scenario to 256 leaves (matching tasks.md:21 prover_smoke), or (b) measure on the actual hardware before committing the SLA into the spec, or (c) say "best-effort, no SLA below 1024 leaves."

---

### P1 — `--tap-cardano preview` design does not say what happens if the chain stalls

**Where**: proposal.md:18, design.md:217, tasks.md:58.

**Claim under review**: "ticks the Raft heartbeat off the Cardano preview chain's slot stream via pallas-network."

**Problem**: Cardano preview testnet is a real network with reset events, partition events, and operator-driven downtime. If the slot stream stops ticking (preview rolls back, or pallas-network's chain-sync miniprotocol hits an exception), the harness's Raft heartbeat stops. With heartbeat 250 ms wall-clock and election timeout 1 s, the cluster will start an election within seconds of preview going quiet — even though the cluster is itself perfectly healthy. The design says this is "purely cosmetic" but that's only true if the slot stream is monotone and continuous, which preview isn't.

**Impact**: `--tap-cardano preview` users see leader-thrash whenever preview testnet wobbles. Bad demo when the demo is "feels like a chain."

**Recommendation**: Either fall back to wall-clock heartbeat after N seconds of slot-stream silence, or document `--tap-cardano` as "preview-stable-only, expect election storms during preview reset events." Also: verify `pallas-network`'s chain-sync error semantics on `MsgRollback` events — the harness needs to handle rollback, not just forward progress.

---

### P1 — "Plonky3 prover memory ≈ 1 GiB at 2^20 trace" cap is unsound under unfolded multi-claim

**Where**: design.md:284 ("A 2^20 trace at BabyBear with Poseidon2 Merkle commits to ~1 GiB peak. The prototype caps `--leaves` at 1024 with a clear error if exceeded.").

**Problem**: 1024 leaves is asserted to fit; 2^20 is the cap. But each leaf's Merkle path is depth ~20, each path step contributes multiple trace rows (one per compression), so 1024 leaves × 20 steps × ~2 compressions × N rows-per-compression = potentially well past 2^20 rows. The arithmetic is hand-waved, and `OmegaMembershipAir` is "one row per Merkle path step" (design.md:99) — but the per-row width depends on whether the Blake3 compression is unrolled into the same trace or in a separate trace via permutation argument. The latter is cheaper but adds a permutation column.

**Impact**: The first time someone runs `bench --leaves 1024` on a 16 GiB laptop, OOM.

**Recommendation**: Pin a concrete arithmetic budget: `trace_height = N_leaves × tree_depth × rows_per_step`, `peak_memory ≈ trace_height × trace_cols × field_size × constant_factor`. Either lower `--leaves` cap to 256 (matching the smoke test) or actually measure before committing the SLA. The 1024 number sounds reasonable but is unbacked.

---

### P2 — "openraft default 250 ms heartbeat; election 1 s" not verified

**Where**: design.md:217.

**Problem**: openraft's defaults shift across versions. The numbers may be right, but verifying against the pinned `openraft = "0.9"` is one `cargo doc` away and worth doing.

**Recommendation**: Read `openraft::Config::default()` at the pinned version and quote the actual values into design.md. Or, better, set them explicitly in `RaftConfig` so the harness is not at the mercy of openraft default churn.

---

### P2 — JSON-RPC mention is informal; no schema

**Where**: design.md:241, proposal.md:13 ("JSON-RPC submit endpoint").

**Problem**: "JSON-RPC over TCP" appears once with no method name, params shape, or error code map. The CLI tasks (tasks.md:65-69) call into it but the spec does not constrain what it returns.

**Recommendation**: Either name the method (`omega.submit`, `omega.state`, etc.) and pin a JSON schema in spec.md, or drop the "JSON-RPC" framing and call it "a thin TCP RPC" with explicit message types.

---

### P2 — `proof-harness` capability is named, but the spec.md path is unusual

**Where**: proposal.md:25-26 ("New Capabilities: `proof-harness`"), spec.md location.

**Problem**: OpenSpec convention typically uses lowercase-hyphenated capability slugs that match the spec directory. `specs/proof-harness/spec.md` matches; good. But the README.md at line 1 in the change directory should ideally cross-link to the spec.md so readers can navigate. (Did not read README.md to confirm.)

**Recommendation**: Verify the change-directory README cross-links to `specs/proof-harness/spec.md`.

---

## Missed failure modes

These are scenarios the design does not list but should. Each one has bitten a similar prototype before.

### M1 — SQLite WAL is per-process; openraft's snapshot-via-VACUUM-INTO is not transactional with the live writer

**Where**: design.md:194, tasks.md:39.

When openraft requests a snapshot, the design does `VACUUM INTO 'snapshot-<idx>.sqlite'`. `VACUUM INTO` opens a read transaction at the start and writes a new file. If the writer task commits new entries during the VACUUM, those entries are *not* in the snapshot — which is correct (snapshot is at log index `idx`) — but the harness must ensure the writer is *paused* at log index `idx` when VACUUM runs, otherwise the snapshot's last-applied-log-id and the snapshot file content disagree. This is a classic "snapshot-skew" race.

**Recommendation**: Add a `Mutex<()>` or `RwLock` around the writer-thread input channel that the snapshot path takes for the duration of the VACUUM. Document the expected pause window in the spec.

### M2 — WAL grows unbounded *between* checkpoints under load

The design notes `wal_autocheckpoint = 10000` (design.md:151), but this is *pages*, not log entries. Under 1 MB-per-claim load (a 1024-leaf collection's CBOR is large), the WAL grows by megabytes per claim and 10000 pages = 40 MB at 4 KiB/page. The autocheckpoint fires when a writer crosses the threshold during commit, which means readers see the WAL grow to the threshold before any cleanup. Combined with `mmap_size = 256 MiB`, a sustained 100 claim/s ingest rate will hit the mmap ceiling within seconds.

**Recommendation**: Add a periodic explicit `PRAGMA wal_checkpoint(TRUNCATE)` from a low-priority task, every N seconds or M committed entries.

### M3 — openraft snapshot policy and SQLite VACUUM cost interact badly

openraft's default snapshot policy compacts every 10000 log entries (or thereabouts; verify against pinned version). VACUUM INTO on a multi-gigabyte SQLite file is *not* fast — typically minutes for tens of GB, dominated by full-table copy cost. During VACUUM the writer is paused (per M1). On a busy node, snapshot frequency should be much lower than the natural openraft default.

**Recommendation**: Override openraft's snapshot policy explicitly. Document that VACUUM INTO scales with DB size, not with log size.

### M4 — `PRAGMA mmap_size` interacts with multi-process file locking; if any external tool (a CLI debugger, a backup script) opens the DB while it's mmap'd, behaviour is platform-dependent

If a developer runs `sqlite3 ./node1.db` while the harness has it mmap'd at 256 MiB, behaviour on Windows differs from Linux. Not a soundness issue but a debugging gotcha.

**Recommendation**: Document the implication. Possibly disable mmap on Windows.

### M5 — libp2p's mDNS on macOS will flood the developer's local network

mDNS multicast packets reach every machine on the LAN. A developer running the harness on a laptop with three nodes will broadcast `omega-claims-v0` topic discovery to every other macOS device sharing the LAN. Not a security or correctness issue; an embarrassment factor.

**Recommendation**: Use a non-default mDNS service name that includes a per-developer salt, or scope to `localhost`-only discovery.

### M6 — Plonky3 git rev pinning vs `Cargo.lock` discipline

If only one of the prover/verifier crates pins the Plonky3 git rev and the other inherits via workspace, a workspace-level `cargo update` could pull two different revs. This is a rare but real failure when prover and verifier disagree on AIR layout.

**Recommendation**: Pin the rev *once* in the workspace `[workspace.dependencies]` block; every harness crate references via `workspace = true`. Single source of truth.

### M7 — `MultiEraTx` decoding is nominal; the harness does not say which era's CBOR codec it uses

If a developer submits a Conway-era tx and the feature code defaults to Babbage decoding, validation succeeds for the wrong reasons.

**Recommendation**: Spec the decode path: which `pallas` decoder, which era, what error if the tx CBOR is from an unsupported era.

### M8 — `omega-experiment bench` percentile reporting from 100 samples is statistically meaningless for p99

100 samples gives a p99 estimator with ±15% noise. The bench command's `prove_p99_ms` (spec.md:117) is reported as if it's a number but reading it as a literal SLA is statistically unsound.

**Recommendation**: Either default `bench` to ≥ 1000 samples for p99 or rename to `prove_p95_ms` only. (Bench p99 with 100 samples is fine for "is it the same order of magnitude as last week" but should not be a CI gate.)

### M9 — `--features cardano-tx-validation` is tested in CI (tasks.md:99) but not exercised in any e2e flow

The integration test (design.md:289-298, tasks.md:73-79) only exercises native Omega ClaimTx; the Cardano-tx feature has unit tests (tasks.md:43) but no e2e through Raft + libp2p. Two of the design's listed scenarios in spec.md:95-101 are unit-test scenarios disguised as integration scenarios.

**Recommendation**: Either add an integration test that submits a Cardano `MultiEraTx` through the JSON-RPC endpoint with the feature enabled, or scope spec.md scenarios down to "unit-test" level explicitly.

### M10 — No test for "node restart preserves state"

The whole point of SQLite persistence is durability. The test plan (design.md:289-298) exercises a single in-process run. Missing: stop a node, restart it, assert it rejoins quorum without losing committed state.

**Recommendation**: Add a `tests/restart_durability.rs` covering "submit claim → quorum applies → kill all 3 nodes → restart → assert nullifier table contents persisted on all 3."

---

## Recommendations

In implementation order:

1. **Fix the version pins (proposal.md:20, :35).** Replace "1.x" / "HEAD" with concrete versions/revs. This is a 10-minute edit and unblocks every downstream task.
2. **Replace `spawn_blocking` per-call with an actor pattern (design.md:188-192, tasks.md:36).** Document the channel + dedicated thread pattern explicitly. Add a heartbeat-continuity test under load.
3. **Tighten the Plonky3 leaf-preimage soundness story (spec.md:21, tasks.md:18).** Either commit to a leaf-preimage AIR or restrict v0.1 to ≤ 64-byte leaves with a documented limitation.
4. **Verify pallas-validate API at the pinned version (spec.md:88-101).** Quote actual function signatures into design.md, or punt the feature to a follow-up change.
5. **Specify the snapshot-chunking wire protocol (design.md:232-234).** Serial transfer, in-order, single-in-flight, abort-on-leader-change. Add a resume-after-leader-change scenario.
6. **Drop or justify gossipsub (design.md:231).** On a 3-node cluster Raft's `AppendEntries` already broadcasts; gossipsub is redundant unless the use case is "find-the-leader-via-broadcast."
7. **Add the missing failure-mode tests (M1, M2, M10 above).** Snapshot-during-write race; WAL growth under load; restart durability. These three cover the operational gotchas the design does not.
8. **Realistic perf budgets (spec.md:25-29, tasks.md:106).** Either measure on actual hardware or remove the SLA-style numbers; "30 s prove" and "32 MiB proof" should be evidence-backed, not aspirational.
9. **Verify openraft heartbeat/election defaults at pinned version (design.md:217).** Either trust the docs after reading them, or set explicitly in `RaftConfig`.
10. **Add real-TCP integration test (design.md:289-298).** `MemoryTransport`-only testing hides the most common deployment-shaped bugs.

The proposal is well-scoped overall — the deferred constraints (C6/C7), the feature-flagged Cardano validation, and the in-memory test path are correctly bounded. The challenges above are concentrated in the integration seams (openraft↔rusqlite, libp2p↔Raft, Plonky3↔real-leaf-preimage), which is where prototype harnesses always live or die. None of the P0 items are conceptual blockers; all three are "fix the wording / pin the version / commit to the actor pattern" rather than redesigns.

The harness is worth building. The design needs ~2 hours of cleanup before tasks.md is safe to start.
