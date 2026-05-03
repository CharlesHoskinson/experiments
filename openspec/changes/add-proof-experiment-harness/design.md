# Design: Omega proof-experiment harness

## Goal

Make Plonky3 proofs round-trip end-to-end on a developer laptop: builder → libp2p broadcast → openraft quorum → SQLite-backed mock ledger → Plonky3 verifier → state mutation → CLI inspection. Prove the architecture is testable before any of T2 (real consensus) / T4 (real network) / T5 (real storage) lands.

## Architecture

```
                     ┌──────────────────────────────┐
                     │     omega-experiment CLI     │
                     │  prove │ submit │ state │ bench
                     └──────────────────────────────┘
                       │                    │
              build proof          submit RPC (JSON-RPC over TCP)
                       ▼                    ▼
        ┌─────────────────────┐    ┌──────────────────────────────────────────┐
        │ omega-claim-prover  │    │              omega-mock-ledger × 3       │
        │ (Plonky3 STARK)     │    │  ┌─────────────────────────────────┐     │
        │   • p3-uni-stark    │    │  │ openraft state machine adapter  │     │
        │   • p3-blake3-air   │    │  │ apply_to_state_machine          │     │
        │   • Poseidon2 Merkle│    │  └────────────┬────────────────────┘     │
        └─────────────────────┘    │               ▼                          │
                                   │  ┌─────────────────────────────────┐     │
                                   │  │ omega-claim-verifier (Plonky3)  │     │
                                   │  │ optional: pallas-validate phase1│     │
                                   │  └────────────┬────────────────────┘     │
                                   │               ▼                          │
                                   │  ┌─────────────────────────────────┐     │
                                   │  │ rusqlite (SQLite + WAL)         │     │
                                   │  │   raft_log, raft_meta,          │     │
                                   │  │   nullifiers, starstream_utxos, │     │
                                   │  │   genesis                       │     │
                                   │  └─────────────────────────────────┘     │
                                   └────────────────┬─────────────────────────┘
                                                    │
                                            openraft RPCs
                                                    │
                                                    ▼
                                   ┌──────────────────────────────────────────┐
                                   │  omega-network (libp2p)                  │
                                   │   transport: TCP + Noise + Yamux         │
                                   │   discovery: mDNS (local) + Kademlia     │
                                   │   protocols: request_response (Raft RPCs)│
                                   │              gossipsub (claim-tx fanout) │
                                   └──────────────────────────────────────────┘
                                                    │
                                                    ▼
                                       ┌─────────────────────────┐
                                       │ optional: pallas-network│
                                       │ chain-sync to Cardano   │
                                       │ preview testnet (slot   │
                                       │ ticks for heartbeat)    │
                                       └─────────────────────────┘
```

Three nodes form a Raft quorum on one machine via three OS ports. mDNS handles local discovery; Kademlia DHT is wired in but unused at quorum-3.

## Component design

### `omega-claim-tx` — claim transaction types

Pure types + CBOR codec. No I/O. No async.

Variants:

```rust
pub enum ClaimTx {
    Utxo(ClaimUtxo),
    Collection(ClaimCollection),  // batched: prove inclusion of N leaves at once
}

pub struct ClaimUtxo {
    pub public: ClaimPublicInputs,
    pub witness: ClaimWitness,    // private; redacted before broadcast in v1
    pub proof: ProofBytes,        // Plonky3 STARK bytes
}

pub struct ClaimPublicInputs {
    pub sub_tree_id: u8,
    pub leaf_index: u64,
    pub bundle_root_blake3: [u8; 32],
    pub nullifier: [u8; 32],
    pub recipient_starstream_addr: [u8; 32],
}

pub struct ClaimCollection {
    pub public: Vec<ClaimPublicInputs>,
    pub witness: Vec<ClaimWitness>,
    pub proof: ProofBytes,        // ONE folded proof for the whole batch (v0.2)
}
```

CBOR encoding via `serde_cbor` (or `minicbor` for parity with omega-commitment-ingest). Round-trip property tests via proptest.

### `omega-claim-prover` — Plonky3 STARK prover

Wraps `p3-uni-stark` over BabyBear field with Poseidon2 Merkle config (matching the example in `var/upstream/Plonky3/examples/examples/prove_prime_field_31.rs`). The AIR is a custom `OmegaMembershipAir` that traces:
- one row per Merkle path step
- per-step Blake3 compressions delegated to `p3-blake3-air` via permutation argument

**Soundness boundary — explicit**. `p3-blake3-air` constrains the Blake3 *compression function*, not the full Blake3 hash including chunk/tree-mode chaining, finalization flag handling, and the length-prefix block. For leaf preimages that fit in one compression block (≤ 64 bytes after the domain tag and length prefixes), the compression-only AIR is sound — the prover's claim that a particular compression input represents the public preimage can be checked trivially in the verifier. For leaf preimages that span multiple compression blocks (variable-length UTxO leaves with native-asset bundles, ≥ 81 bytes minimum, can grow into multi-block territory), an off-circuit gluing step must assert that the compression block inputs were correctly derived from the public preimage including the `omega:v2:leaf` domain tag and the `payload_len_be` length prefix — and that gluing step is exactly where soundness bugs hide.

**v0.1 scope decision**: restrict the prototype to leaf preimages ≤ 64 bytes (one Blake3 compression block). This bounds the synthetic UTxO fixtures to no-asset, no-datum, no-script-ref UTxOs — sufficient for the round-trip demo. spec.md captures this as an explicit limitation. v0.2 lands a `LeafPreimageAir` that constrains the chunk/finalization plumbing for variable-length leaves, with its own audit. Real mainnet UTxO ingestion requires the v0.2 AIR; the harness is not a production prover, by design.

For v0.1 the prover discharges only C1-C5 from the README's verifier table (the membership half), under the ≤ 64-byte leaf restriction. C6 (PQ signature) is mocked by an Ed25519 sigcheck. C7-C8 (PLUME nullifier + uniqueness) are out-of-circuit at v0.1. Node hashes (`tag || left || right` = 13 + 32 + 32 = 77 bytes) span two compression blocks but follow a fixed layout, so the off-circuit gluing for the node hash is trivially correct (the verifier re-derives the inputs deterministically).

API surface:

```rust
pub fn prove_collection(
    commitment: &OmegaCommitment,
    witnesses: &[InclusionWitness],
    config: &ProverConfig,
) -> Result<ProofBytes, ProverError>;
```

### `omega-claim-verifier` — Plonky3 STARK verifier

Pure verification function. Symmetric with the prover. Uses the same `OmegaMembershipAir` and config.

```rust
pub fn verify(
    commitment: &OmegaCommitment,
    public_inputs: &[ClaimPublicInputs],
    proof: &ProofBytes,
) -> Result<(), VerifyError>;
```

No tokio, no async, no I/O.

### `omega-mock-ledger` — state machine + persistence

Implements openraft's `RaftStateMachine` trait. State lives entirely in SQLite via rusqlite; the in-memory cache is bounded and purely a hot path.

Apply pipeline (per Raft-committed entry):

1. Parse the entry's payload as `ClaimTx`.
2. If `ClaimTx::Utxo` or `ClaimTx::Collection`: invoke `omega-claim-verifier::verify(...)`. Reject on error.
3. For each public input: assert `(sub_tree_id, leaf_index) ∉ nullifiers` table. Reject on collision.
4. (Optional, behind `cardano-tx-validation` feature) If the entry is a wrapped Cardano `MultiEraTx`, invoke `pallas_validate::phase1::validate_tx`. Reject on error.
5. Begin SQLite transaction. Insert nullifier row(s). Insert Starstream UTxO row(s) computed from witness payload. Commit.
6. Return success to openraft so the entry is marked applied.

SQLite schema:

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA cache_size = -65536;
PRAGMA mmap_size = 268435456;
PRAGMA temp_store = MEMORY;
PRAGMA wal_autocheckpoint = 10000;
PRAGMA auto_vacuum = NONE;

CREATE TABLE raft_log (
  log_idx     INTEGER PRIMARY KEY,
  term        INTEGER NOT NULL,
  payload     BLOB NOT NULL              -- serialized openraft Entry
) WITHOUT ROWID;

CREATE TABLE raft_meta (
  k           TEXT PRIMARY KEY,
  v           BLOB NOT NULL              -- vote, last_purged_log_id, current_term
) WITHOUT ROWID;

CREATE TABLE nullifiers (
  sub_tree_id INTEGER NOT NULL,
  leaf_index  INTEGER NOT NULL,
  block_idx   INTEGER NOT NULL,
  PRIMARY KEY (sub_tree_id, leaf_index)
) WITHOUT ROWID;

CREATE TABLE starstream_utxos (
  utxo_id     BLOB PRIMARY KEY,           -- 32-byte hash of (recipient || value || ...)
  recipient   BLOB NOT NULL,
  value       INTEGER NOT NULL,
  asset_blob  BLOB NOT NULL,
  datum       BLOB,
  script_ref  BLOB,
  block_idx   INTEGER NOT NULL,
  spent_in    INTEGER                     -- NULL until spent on Omega
) WITHOUT ROWID;

CREATE TABLE genesis (
  k           TEXT PRIMARY KEY,
  v           BLOB NOT NULL               -- ω-commit, snap-block, snap-cert, ceremony
) WITHOUT ROWID;
```

Concurrency model — **actor pattern, not per-call spawn_blocking**:

- One dedicated OS thread owns the rusqlite write `Connection`. It loops on an `mpsc::UnboundedReceiver<WriteCmd>`; each `WriteCmd` carries a `oneshot::Sender<Result<…, _>>` for the reply. The openraft state machine sends a `WriteCmd` and `await`s the oneshot. This pattern is required (not just preferred) because openraft's `RaftStateMachine::apply` returns a future that must be cancel-safe and make progress under back-pressure; per-call `spawn_blocking` does not pipeline writes and produces a thundering herd against the WAL writer-serialisation under load. (Per-call `spawn_blocking` was the design's first instinct; the QA review correctly flagged it as necessary-but-not-sufficient.)
- Readers borrow short-lived `Connection`s from an `r2d2_sqlite` pool sized to `num_cpus::get()`. Reader queries run in `tokio::task::spawn_blocking` (acceptable for reads — they don't block the writer thread, and SQLite WAL reads are concurrent with writes).
- Prepared-statement cache (rusqlite's built-in) keeps the hot statements (`INSERT OR ABORT INTO nullifiers`, `SELECT 1 FROM nullifiers WHERE ...`) permanently compiled on the writer connection.
- Heartbeat-continuity test under load: `tests/load_heartbeat.rs` runs a 60-second submit storm and asserts the cluster's leader does not change. If it does, the actor pattern has a contention bug.

Snapshotting:

- openraft's `RaftSnapshotBuilder` sends a `WriteCmd::Snapshot { snapshot_id, reply }` through the same channel as writes. The writer thread picks up the command, runs `VACUUM INTO 'snapshot-<idx>.sqlite'` synchronously (blocking the writer thread for the duration), then resumes ordinary writes. This is the **snapshot pause window** — explicitly serialised against writes via the channel ordering, addressing the snapshot-skew race (no separate mutex needed because the channel is the mutex). Document the pause cost: VACUUM scales with DB size, not with log size; a multi-GiB DB pauses the writer for tens of seconds.
- Periodic explicit `PRAGMA wal_checkpoint(TRUNCATE)` runs every 30 seconds from a low-priority task, capping WAL growth between automatic checkpoints under sustained load.
- openraft's snapshot policy is overridden in `RaftConfig` to compact every 100,000 log entries (10× the default) so VACUUM does not fire on every minor log compaction.

### `omega-toy-consensus` — openraft node runner

Wraps `openraft::Raft` with our `omega-mock-ledger` storage and `omega-network` transport.

```rust
pub struct ToyConsensusNode {
    raft: openraft::Raft<TypeConfig>,
    storage: Arc<MockLedgerStorage>,
    network: Arc<LibP2pNetwork>,
}

impl ToyConsensusNode {
    pub async fn submit(&self, claim: ClaimTx) -> Result<(), SubmitError> {
        self.raft.client_write(ClaimEntry::from(claim)).await?;
        Ok(())
    }
}
```

Cluster membership: fixed 3 nodes at `127.0.0.1:{4001,4002,4003}` (libp2p TCP listen ports). Bootstrap via `openraft::raft::Raft::initialize` with a hard-coded membership set. No dynamic membership in v1.

Heartbeat: set explicitly in `RaftConfig`, not inherited from openraft defaults (defaults shift across versions). Concrete values: `heartbeat_interval = Duration::from_millis(250)`, `election_timeout_min = Duration::from_secs(1)`, `election_timeout_max = Duration::from_secs(2)`. With `--tap-cardano preview`, the harness subscribes to the named Cardano network's chain-sync miniprotocol via `pallas-network` and uses slot ticks as a wall-clock reference — but **only as a heartbeat hint, not as the only timing source**. If the slot stream goes silent (preview reset, partition, `MsgRollback`), the harness falls back to wall-clock heartbeats after 5 s of silence so the cluster does not start an election storm whenever preview testnet wobbles. Rollback events from the chain-sync miniprotocol are handled by resetting the slot reference, not by re-electing.

### `omega-network` — libp2p transport

A thin shim that implements openraft's `RaftNetworkFactory` + `RaftNetwork` traits over rust-libp2p.

Stack:

| Layer | Choice | Why |
|---|---|---|
| Transport | TCP + Noise + Yamux | Default, well-tested, all three nodes on one box |
| Identity | Ed25519 (libp2p `Keypair::generate_ed25519`) | Prototype only — swap to SLH-DSA in v0.2 if we want PQ identity |
| Discovery | mDNS (local) + Kademlia (latent) | mDNS auto-discovers on a developer laptop; Kademlia is wired but inactive at quorum-3 |
| Raft RPCs | `request_response` codec with CBOR payloads | Append-Entries, Request-Vote round-trip with bounded payloads. Same path also carries client `submit` calls (no need for a separate JSON-RPC channel; client points at any node, the node forwards to leader). |
| Snapshot transfer | chunked `request_response` (see protocol below) | InstallSnapshot can be GiB-scale; chunk into 1 MiB request_response payloads with index headers |

Gossipsub is **dropped from v0.1**. On a 3-node fixed-membership cluster, Raft's `AppendEntries` already broadcasts every entry to every follower with authoritative ordering. A second broadcast layer is redundant and was a net negative (extra protocol surface, possible double-apply if dedup-by-content-hash fails). Reintroduce only if a future use case (e.g., "find-the-leader-via-broadcast" for a stateless client) actually requires it.

Snapshot wire protocol — explicit because the QA review correctly flagged that `request_response` is unordered and not flow-controlled by default:

1. Leader sends `SnapshotInit { snapshot_id, total_chunks, total_bytes, sha3_of_full }` as a single `request_response` call. Follower acks.
2. Leader sends chunks **serially in chunk-index order**, **single in-flight request at a time**: each chunk is a `SnapshotChunk { snapshot_id, chunk_idx, payload }` ≤ 1 MiB.
3. Follower writes each chunk to disk synchronously before acknowledging. Disk-write rate is the natural backpressure.
4. After the last chunk, leader sends `SnapshotFinalize { snapshot_id, sha3_of_full }`. Follower verifies the SHA3 hash, swaps the file into place, and reports `Ok` to openraft.
5. Any leader-change event during a snapshot session aborts the session; the new leader restarts from chunk 0 (matches openraft's snapshot model — no resume-mid-transfer in v0.1).

This is mandatory v0.1, not nice-to-have. A scenario in spec.md exercises a leader-change-mid-snapshot by killing the leader between chunks and asserting the new leader restarts the snapshot cleanly.

### `omega-experiment` — CLI orchestrator

Single binary, four subcommands:

```bash
omega-experiment prove   --commit var/bundle.json --leaves var/leaves.json --out var/proof.bin
omega-experiment submit  --node 127.0.0.1:4001 --proof var/proof.bin
omega-experiment state   --node 127.0.0.1:4001
omega-experiment bench   --leaves 100 --commit var/bundle.json
```

`bench` runs N rounds of `prove + submit` against a hot quorum and reports p50 / p95 / p99 prove latency and submit-to-applied latency.

## Data flow: a full claim

```
1. CLI: omega-experiment prove --leaves leaves.json
   ──► omega-claim-prover::prove_collection(commitment, witnesses)
       (Plonky3 STARK over BabyBear, ~3-30s depending on N)
   ──► writes var/proof.bin

2. CLI: omega-experiment submit --node 127.0.0.1:4001 --proof var/proof.bin
   ──► JSON-RPC submit to node 1's ledger endpoint
   ──► node 1's openraft client_write(ClaimEntry)
   ──► openraft replicates via omega-network → libp2p request_response
       to nodes 2 and 3
   ──► nodes 2, 3 ack; quorum reached
   ──► all three nodes' state machines apply the entry:
       a. Verifier runs (Plonky3 verify, ~200ms)
       b. Nullifier check (SQLite SELECT, ~10us)
       c. Insert (SQLite WAL append, ~50us)
       d. Insert Starstream UTxO (SQLite WAL append, ~50us)
   ──► openraft marks entry applied; client_write returns Ok

3. CLI: omega-experiment state --node 127.0.0.1:4001
   ──► JSON-RPC state query
   ──► reads SQLite via reader pool: nullifier set size, Starstream UTxO list
   ──► prints summary
```

Total wall: prove (3-30s) + submit (50-100ms) + apply (~200ms × 3 nodes parallel) + state (~5ms).

## Failure modes (tracked, not closed)

- **rusqlite is sync; openraft is async.** Every state-machine call is wrapped in `tokio::task::spawn_blocking` to avoid stalling the runtime. If we forget this somewhere, the Raft heartbeat misses and the node gets demoted. Mitigation: lint rule + integration test that asserts heartbeat continuity under load.
- **WAL grows unbounded.** Without periodic checkpoints the WAL file dominates disk. `wal_autocheckpoint = 10000` pages caps it; openraft's snapshot policy (compact every 10k log entries) gives the SQLite checkpoint a natural tick. Both must fire.
- **libp2p `request_response` size limit vs Raft InstallSnapshot.** Default 10 MiB; Raft snapshots are GiB-scale. Solved by chunking, but the chunking code is a real implementation task — not a one-liner.
- **mDNS in CI.** GitHub Actions runners often disable mDNS multicast. The harness must accept hard-coded peer addresses via `--peers` for CI, falling back to mDNS only on developer machines.
- **Plonky3 prover memory & perf budgets are unbacked targets, not SLAs.** The single-leaf 30 s prove budget and 1024-leaf cap in the spec are first-pass targets derived from the 2026-05-03 spike (1024 Blake3 perms in 3.43 s, 4.9 MiB proof, BabyBear+Poseidon2). A 1024-leaf collection exercises ~40,000 compressions and the budgets may not hold; v0.1 acceptance lowers the bench to 256 leaves and treats anything beyond that as best-effort. Real numbers come from `bench --json --leaves N` measurements once the prover lands; the spec gets updated with measured percentiles before the v0.1 tag.
- **`pallas-validate` is alpha.** Pinning to `=1.0.0-alpha.6` accepts API churn. Mitigation: the Cardano-tx-validation feature is off by default; harness ships and runs without it. A task in `tasks.md` explicitly verifies the `phase1::validate_tx` signature against the pinned source before writing test code.
- **mDNS LAN flooding.** mDNS multicast packets reach every machine on the LAN; running the harness on a developer laptop broadcasts the `omega-claims-v0` topic to the whole network. Mitigation: scope discovery to `localhost`-only and use a non-default service name with a per-installation salt.
- **WAL `mmap_size` × external-tool interaction.** If a developer opens the live SQLite file with `sqlite3` while it's mmap'd at 256 MiB, behaviour differs across platforms. Document the implication; consider disabling `mmap_size` on Windows.
- **WAL truncate cadence.** `wal_autocheckpoint = 10000` is *pages* (40 MiB at 4 KiB/page); under sustained submit load the WAL crosses that threshold every few seconds and a periodic explicit `PRAGMA wal_checkpoint(TRUNCATE)` is required to keep the file from growing into mmap territory.

## Test strategy

- Unit tests per crate (proptest where possible).
- Integration test in `omega-experiment/tests/e2e.rs` that:
  1. Spins up 3 in-process `ToyConsensusNode`s (mDNS disabled, hard-coded peers via in-memory libp2p `MemoryTransport`).
  2. Generates a 256-leaf synthetic UTxO sub-tree using `omega-commitment-core::Tree::build_v1`.
  3. Picks a random leaf, builds witness, runs prover, submits.
  4. Asserts all 3 nodes' nullifier tables contain `(1, leaf_index)` and exactly one Starstream UTxO row.
  5. Replays same submission → expects nullifier-collision rejection.
  6. Tampered proof / witness → expects verifier rejection.
- Soak test (`bench --soak 1h`) manual only.
- No CI requirement for the `--tap-cardano preview` path — it requires network and a synced node.

## What this design defers

- C6 (PQ signature SLH-DSA) — mocked Ed25519 in v0.1.
- C7 (PLUME nullifier) — derived deterministically from `(K_pq, sub_tree_id, leaf_index)` via Blake3 in v0.1; the actual ERC-7524 PLUME construction lands in v0.2.
- Snapshot of the immutable Cardano state into the genesis params — for v0.1 the genesis is a synthetic fixture pinned at startup.
- Real openraft membership changes / dynamic peers — v0.1 has fixed 3-node membership.
- Production Plonky3 config — v0.1 uses the demo config from `prove_prime_field_31.rs`. Production picks fri-rate, queries, etc. based on a security analysis.
