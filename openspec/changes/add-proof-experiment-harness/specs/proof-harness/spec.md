## ADDED Requirements

### Requirement: Claim transaction format

The harness SHALL define a `ClaimTx` enum with two variants — `Utxo(ClaimUtxo)` for single-leaf claims and `Collection(ClaimCollection)` for batched-leaf claims — encoded canonically via CBOR. Each variant SHALL carry public inputs (sub-tree id, leaf index, bundle root, nullifier, recipient address), private witness data (leaf payload, Merkle path, signing key proof), and a Plonky3 STARK proof byte string. The encoding MUST round-trip via `ClaimTx::to_cbor` / `ClaimTx::from_cbor` for any valid input.

#### Scenario: Single-leaf claim round-trips through CBOR
- **WHEN** a `ClaimUtxo` is constructed with valid public inputs, witness, and a Plonky3 proof
- **THEN** `ClaimTx::Utxo(c).to_cbor()` followed by `ClaimTx::from_cbor` returns a `ClaimTx::Utxo` with byte-identical fields

#### Scenario: Batched-leaf claim round-trips through CBOR
- **WHEN** a `ClaimCollection` is constructed with N ≥ 2 sets of public inputs and one folded proof
- **THEN** `ClaimTx::Collection(c).to_cbor()` followed by `ClaimTx::from_cbor` returns a `ClaimTx::Collection` with byte-identical fields

#### Scenario: Tampered CBOR is rejected
- **WHEN** any byte of an encoded `ClaimTx` payload is flipped before decode
- **THEN** `ClaimTx::from_cbor` returns `Err` with a deterministic error variant (no panic)

### Requirement: Plonky3 prover for Merkle membership (v0.1, ≤ 64-byte leaves)

The `omega-claim-prover` crate SHALL produce a Plonky3 STARK proof attesting that a collection of leaves is included in a published Ω-Commitment bundle root. The prover MUST use the BabyBear field with Poseidon2 Merkle commits and delegate Blake3 *compression* to `p3-blake3-air`. **For the v0.1 prototype the prover is restricted to leaf preimages whose total length (after the `omega:v2:leaf` domain tag and length prefix) fits in a single Blake3 compression block — ≤ 64 bytes of payload.** This bounds synthetic UTxO fixtures to no-asset, no-datum, no-script-ref UTxOs. Variable-length leaves require a `LeafPreimageAir` that constrains Blake3's chunk/finalization plumbing; that AIR is v0.2 and out of scope here. The prover SHALL discharge constraints C1-C5 from the README's verifier table under this restriction. C6 (PQ signature), C7 (PLUME nullifier), and C8 (uniqueness) are explicitly out of scope for v0.1.

Performance budgets in the scenarios below are **first-pass targets**, not SLAs; they are derived from the 2026-05-03 Plonky3 spike (1024 Blake3 perms in 3.43 s on a developer laptop) and may need adjustment after `bench` measurements.

#### Scenario: Single-leaf proof is generated under the v0.1 leaf-size restriction
- **WHEN** `prove_collection(commitment, &[witness], config)` is called with a valid inclusion witness for a ≤ 64-byte leaf in the bundle
- **THEN** the call returns `Ok(proof_bytes)` within 30 seconds on a developer laptop (target, not SLA)

#### Scenario: Batched proof for 256 leaves
- **WHEN** `prove_collection(commitment, witnesses, config)` is called with 256 valid witnesses, each a ≤ 64-byte leaf
- **THEN** the call returns `Ok(proof_bytes)` and the resulting proof bytes are < 16 MiB (target)

#### Scenario: Prover rejects a witness with a wrong path
- **WHEN** an inclusion witness has a Merkle path that does NOT walk to the published per-sub-tree root
- **THEN** `prove_collection` returns `Err(ProverError::PathMismatch)` and does NOT emit a proof

#### Scenario: Prover rejects a leaf payload exceeding the v0.1 size restriction
- **WHEN** an inclusion witness has a leaf payload that would push the preimage past 64 bytes
- **THEN** `prove_collection` returns `Err(ProverError::LeafTooLargeForV01)` with the actual size and the bound

### Requirement: Plonky3 verifier for Merkle membership

The `omega-claim-verifier` crate SHALL provide a pure verification function `verify(commitment, public_inputs, proof) -> Result<(), VerifyError>` that returns `Ok(())` exactly when the proof attests to inclusion of the listed leaves under the listed bundle root, and returns a typed `VerifyError` otherwise. The verifier MUST be deterministic and side-effect free (no I/O, no async, no global state).

#### Scenario: Verifier accepts a valid proof
- **WHEN** `verify(commitment, public_inputs, proof)` is called with a proof produced by `prove_collection` against the same commitment and inputs
- **THEN** the call returns `Ok(())` within 1 second

#### Scenario: Verifier rejects a tampered proof
- **WHEN** any byte of a valid proof byte string is flipped before verify is called
- **THEN** `verify` returns `Err(VerifyError::InvalidProof)`

#### Scenario: Verifier rejects a proof against a wrong commitment
- **WHEN** `verify` is called with a proof and a `bundle_root_blake3` that differs from the one the proof was generated against
- **THEN** the call returns `Err(VerifyError::CommitmentMismatch)`

### Requirement: SQLite-backed mock ledger

The `omega-mock-ledger` crate SHALL persist all state in a single SQLite database via rusqlite. The schema MUST contain at least the tables `raft_log`, `raft_meta`, `nullifiers`, `starstream_utxos`, and `genesis`, all declared `WITHOUT ROWID` with composite primary keys where applicable. The database MUST be opened with `journal_mode=WAL`, `synchronous=NORMAL`, `cache_size=-65536`, `mmap_size=268435456`, `temp_store=MEMORY`, `wal_autocheckpoint=10000`, and `auto_vacuum=NONE`. All state-mutating calls SHALL run via `tokio::task::spawn_blocking` so the openraft event loop is never blocked.

#### Scenario: Database initialises with the optimised pragmas
- **WHEN** `MockLedger::open(path)` is called for a fresh path
- **THEN** the resulting `*.sqlite` file has `PRAGMA journal_mode` returning `wal` and `PRAGMA synchronous` returning `1` (NORMAL)

#### Scenario: Nullifier insert and lookup
- **WHEN** an applied claim inserts `(sub_tree_id=1, leaf_index=42)` into `nullifiers`
- **THEN** a subsequent `SELECT 1 FROM nullifiers WHERE sub_tree_id=1 AND leaf_index=42` returns one row

#### Scenario: Duplicate nullifier is rejected at insert
- **WHEN** a second applied claim attempts to insert the same `(sub_tree_id, leaf_index)` pair
- **THEN** the apply pipeline returns `ApplyError::NullifierExists` and the SQLite transaction rolls back leaving the table unchanged

#### Scenario: Concurrent reads do not block writes
- **WHEN** N concurrent reader tasks run `SELECT * FROM nullifiers` while one writer task inserts new rows
- **THEN** all reader tasks complete without `SQLITE_BUSY` errors and the writer's insert latency p99 is < 50 ms on a developer laptop

### Requirement: Raft consensus over libp2p

The `omega-toy-consensus` crate SHALL run an `openraft::Raft` node whose network transport is a libp2p stack (TCP + Noise + Yamux + mDNS + Kademlia + request_response). Gossipsub is NOT used in v0.1 — Raft's `AppendEntries` is the authoritative broadcast layer. A 3-node cluster MUST elect a leader within 5 seconds of the third node joining, and the leader MUST be able to commit a `ClaimTx` entry to a quorum within 1 second on loopback. Snapshot transfer over libp2p MUST chunk the snapshot file into ≤ 1 MiB request_response payloads, transmitted serially in chunk-index order with at most one in-flight request, framed by `SnapshotInit` / `SnapshotChunk` / `SnapshotFinalize` messages. Heartbeat and election timeouts MUST be set explicitly in `RaftConfig`, never inherited from openraft defaults (defaults shift across versions).

#### Scenario: Three-node cluster reaches quorum
- **WHEN** three `ToyConsensusNode` instances bootstrap with hard-coded membership on `127.0.0.1:{4001,4002,4003}`
- **THEN** within 5 seconds exactly one node reports `RaftServerState::Leader` and the other two report `Follower`

#### Scenario: Leader commits a claim to quorum
- **WHEN** a client calls `submit(claim_tx)` on the leader
- **THEN** within 1 second `submit` returns `Ok(_)` and all three nodes' SQLite `nullifiers` table contains the claim's `(sub_tree_id, leaf_index)` row

#### Scenario: Snapshot transfer chunks correctly
- **WHEN** a follower lags behind the leader by more than the log retention threshold and an `InstallSnapshot` is required
- **THEN** the snapshot is transferred as `SnapshotInit` + N × `SnapshotChunk` + `SnapshotFinalize` request_response calls, each chunk ≤ 1 MiB, transmitted serially in chunk-index order with at most one in-flight request, and the follower verifies the SHA3 hash from `SnapshotFinalize` before applying the file

#### Scenario: Snapshot transfer aborts on leader change
- **WHEN** a leader-change event occurs mid-snapshot-transfer
- **THEN** the in-progress snapshot session aborts on the receiver, partial chunks are discarded, and the new leader (when one is elected) restarts the snapshot from chunk 0

#### Scenario: Restart durability
- **WHEN** all 3 nodes are killed after a successful claim apply, then restarted from disk with the same SQLite files
- **THEN** all 3 nodes' `nullifiers` and `starstream_utxos` tables contain the same rows as before the kill, and the cluster re-elects a leader within 5 seconds of restart

### Requirement: Optional Cardano transaction validation

When the `cardano-tx-validation` Cargo feature is enabled on `omega-mock-ledger`, the apply pipeline SHALL accept Cardano-shaped `MultiEraTx` payloads alongside native Omega `ClaimTx` payloads. Cardano payloads MUST be validated by `pallas_validate::phase1::validate_tx` against a configured `Environment`, `UTxOs`, and `CertState`. When the feature is disabled the harness MUST compile and run without `pallas-validate` in its dependency tree.

#### Scenario: Default build excludes pallas-validate
- **WHEN** `cargo build --workspace` is run without `--features cardano-tx-validation`
- **THEN** `pallas-validate` does NOT appear in `cargo tree -p omega-mock-ledger`

#### Scenario: Feature-on Cardano tx is validated
- **WHEN** the feature is on and a syntactically valid Cardano Conway-era `MultiEraTx` is submitted with sufficient `UTxOs` to satisfy phase-1
- **THEN** the apply pipeline calls `pallas_validate::phase1::validate_tx` and returns `Ok(())`; the tx is accepted into the consensus log

#### Scenario: Feature-on invalid Cardano tx is rejected
- **WHEN** the feature is on and a Cardano tx with insufficient input balance is submitted
- **THEN** `pallas_validate::phase1::validate_tx` returns an error and the apply pipeline returns `ApplyError::CardanoPhase1(error)`; the tx is NOT inserted into the log

### Requirement: CLI orchestrator

The `omega-experiment` binary SHALL provide four subcommands — `prove`, `submit`, `state`, `bench` — that together exercise the full prove → submit → apply → query loop on a local 3-node quorum. Each subcommand MUST produce machine-readable output via a `--json` flag and human-readable output by default.

#### Scenario: prove subcommand writes a proof file
- **WHEN** `omega-experiment prove --commit var/bundle.json --leaves var/leaves.json --out var/proof.bin` is run
- **THEN** the command exits with code 0 and `var/proof.bin` exists with non-zero size

#### Scenario: submit subcommand round-trips a proof to a node
- **WHEN** `omega-experiment submit --node 127.0.0.1:4001 --proof var/proof.bin` is run against a healthy 3-node cluster
- **THEN** the command exits with code 0 within 5 seconds and node 1's `state` query reports the new nullifier and Starstream UTxO

#### Scenario: bench reports latency percentiles
- **WHEN** `omega-experiment bench --leaves 100 --commit var/bundle.json --json` is run
- **THEN** the command emits a JSON object with at least `prove_p50_ms`, `prove_p95_ms`, `prove_p99_ms`, `submit_p50_ms`, `submit_p95_ms`, `submit_p99_ms` fields

### Requirement: README how-to section

The repo `README.md` SHALL contain a top-level section titled "Run a proof experiment" that walks the reader from a fresh checkout to a proof round-trip in ≤ 8 copy-paste commands. The commands MUST work on a developer laptop with Rust 1.79+ and no external services beyond the Cargo dependency tree.

#### Scenario: Reader follows the section to completion
- **WHEN** a reader on a fresh checkout runs the commands in the "Run a proof experiment" section in order
- **THEN** the final command prints a state dump containing exactly one nullifier and one Starstream UTxO

#### Scenario: Section explains optional Cardano-tx-validation feature
- **WHEN** a reader reads the section
- **THEN** the section documents how to enable the `cardano-tx-validation` Cargo feature and what additional behaviour it unlocks
