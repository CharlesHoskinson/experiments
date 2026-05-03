## Why

We have proven (2026-05-03 spike) that Plonky3 produces a real STARK over Blake3 permutations in 3.4s and that Starstream's interleaving-proof crate produces a folded ZK proof in 92s. Both work straight from upstream HEAD without forking. We have not yet proven that one of those proofs round-trips through an Omega-side state machine: builder → wire-format → consensus → ledger → verifier-accepts. Without that round-trip we cannot demonstrate the design end-to-end, and we cannot benchmark realistic claim throughput on a developer laptop.

This change scopes a local proof-experiment harness: a small constellation of Rust crates that lets a developer construct a `claim_utxo` over a synthetic Ω-Commitment, broadcast it through libp2p, accept it through an openraft-managed quorum of three mock ledger nodes, persist the resulting state to SQLite, and inspect the result via CLI. It is the missing testable surface between the v0.10 commitment work (which lands per-leaf hashes and bundle roots) and the v1.x work (real-mainnet ingest, real consensus). It is intentionally not production: openraft + libp2p + rusqlite is a developer-laptop quorum, not a chain.

## What Changes

- Add 7 new crates to the `omega-commitment/` workspace:
  - `omega-claim-tx` — claim-transaction types (`ClaimUtxo`, `ClaimCollection`), CBOR codec, public-input layout
  - `omega-claim-prover` — wraps Plonky3 (`p3-uni-stark` + `p3-blake3-air`) to prove Merkle inclusion of a leaf collection against an Ω-Commitment bundle root
  - `omega-claim-verifier` — Plonky3 verifier; pure function over `(commitment, public_inputs, proof) -> Result<(), VerifyError>`
  - `omega-mock-ledger` — in-memory state machine + rusqlite persistence; nullifier set, Starstream UTxO set, genesis params; openraft state-machine adapter
  - `omega-toy-consensus` — openraft node runner; quorum 3, single-leader, fixed cluster membership; not a real consensus
  - `omega-network` — libp2p transport bundle (TCP + Noise + Yamux + mDNS + Kademlia + request_response + gossipsub) wired to openraft's `RaftNetwork` trait
  - `omega-experiment` — CLI: `prove`, `submit`, `state`, `bench`
- Add a new "Run a proof experiment" section to `README.md` with copy-paste commands for spinning up a 3-node local quorum, generating a proof, submitting it, and observing state.
- Optional Cardano-testnet integration: `omega-toy-consensus --tap-cardano preview` ticks the Raft heartbeat off the Cardano preview chain's slot stream via pallas-network. Off by default; on for "feels like a chain" experiments.
- Optional Cardano-tx validation: `omega-mock-ledger` gains a `cardano-tx-validation` Cargo feature that pulls in `pallas-validate` 1.0.0-alpha.6 (`phase1` only, per-era: Byron / Shelley_MA / Alonzo / Babbage / Conway). When on, the mock ledger accepts two tx types: native Omega `claim_tx` (verified by `omega-claim-verifier` against the Ω-Commitment) and Cardano `MultiEraTx` (validated by `pallas-validate::phase1::validate_tx`). Default off. `amaru-ledger` is considered for the future "second implementation" parity work but is not pulled in here — its nightly-Rust requirement and heavier dep tree (amaru-kernel / amaru-plutus / amaru-uplc / amaru-stores) are out of scope for the harness.
- **Pin every dep to a concrete version or git rev**. Captured once in `[workspace.dependencies]` so every harness crate references via `workspace = true` (single source of truth, addresses the P3-rev / Cargo.lock split-pin hazard). Specifically: `plonky3` git `rev = <40-char-hash>` (record at first build), `openraft = "0.9"` (current 0.9.x series — the 1.0 release is on roadmap, not yet shipped on crates.io), `libp2p = "0.55"` (current 0.5x series, pin minor), `rusqlite = "0.32"` with `bundled` feature, `pallas-network = "0.30"`, `pallas-validate = "=1.0.0-alpha.6"` behind feature gate. Bump revs deliberately, never via blind `cargo update`. No forks. No vendored copies.

## Capabilities

### New Capabilities

- `proof-harness`: end-to-end harness for constructing, broadcasting, accepting, persisting, and verifying Plonky3 proofs of Merkle inclusion against an Ω-Commitment, across a 3-node libp2p+openraft quorum with rusqlite-backed state.

### Modified Capabilities

(None. The existing seven sub-tree commitment, the bundle root, the leaf encodings, and the v0.10 Blake3 migration all carry over unchanged.)

## Impact

- Workspace gains 7 crates and a top-level CLI binary `omega-experiment`.
- New runtime dependencies: `plonky3` (git rev pinned at first build), `openraft = "0.9"`, `libp2p = "0.55"` (default-features off; opt in `tcp`, `noise`, `yamux`, `mdns`, `kad`, `request-response`), `rusqlite = "0.32"` with `bundled` feature, `r2d2` + `r2d2_sqlite` for the reader pool, `pallas-validate = "=1.0.0-alpha.6"` behind `cardano-tx-validation` feature (default off), `tokio` (already used). `pallas-network` is already a workspace dep.
- New developer-facing tooling: a 3-node local quorum runs on one machine via three OS ports. No external services required.
- README adds one section (~80 lines).
- New OpenSpec specs directory under `openspec/specs/proof-harness/`.
- No breaking changes to the v0.10 commitment crates. `omega-commitment-core::Tree::build_v1` and the per-sub-tree leaf encoders are consumed unchanged.
- Cardano-testnet tap is opt-in; default offline mode lets CI exercise the full harness without network.
