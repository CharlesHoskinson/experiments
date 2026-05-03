---
slug: omega-testnet-e2e-plan
title: Omega testnet end-to-end plan (preview → claim_utxo on prototype Omega node)
tags: [planning, testnet, t1, t6, t7, omega-commitment, plonky3, claim-circuit]
sources:
  - cardano-developer-portal-testnets
  - mithril-client-doc
  - pallas-network-crate
  - plonky3-repo
  - slh-dsa-crate
  - starstream-impl-plan
  - cardano-cli-pr-1350
  - openspec-getting-started
provenance:
  - cardano-developer-portal-testnets → preview/preprod network endpoints, supported networks list
  - mithril-client-doc → mithril-client CLI usage, aggregator endpoint format
  - pallas-network-crate → LSQ miniprotocol, GetUTxOWhole, BlockQuery types
  - plonky3-repo → STARK toolkit; what we build C1-C8 against
  - slh-dsa-crate → SLH-DSA Rust impl; PQ signature for claim witness
  - cardano-cli-pr-1350 → upstream Cabal SRP fix for Word16-VLE TxIx bug (already worked-around in omega-utxo-snapshot)
  - starstream-impl-plan → upstream Starstream module status (compiler/interp/wasm shipping; type-checker/IVC/MCC/lookups TODO)
  - openspec-getting-started → spec-driven workflow for tracking change proposals as we build
confidence: high
created: 2026-05-03
updated: 2026-05-03
aliases: [testnet-flow, e2e-claim-flow]
cssclass: wiki-page
---

# Omega testnet end-to-end plan

The smallest end-to-end demo we can ship from here: a single Cardano-preview UTxO, snapshotted, ingested into an Ω-Commitment, then claimed against a Rust prototype Omega node that runs a Plonky3 verifier circuit. This is what proves the architecture works before any of T2 (consensus), T4 (network), T9 (formal spec) need to land.

## Goal of the demo

1. Pick one UTxO on Cardano **preview testnet** (small, fast, cheap, already-shielded test data).
2. Run the existing `omega-utxo-snapshot` to acquire the whole-UTxO set at a chosen block.
3. Run `omega-ingest` to produce the seven sub-trees and the Ω-Commitment tuple.
4. Pin the Ω-Commitment as the "genesis" of a one-process Rust Omega node.
5. Pick a leaf, fetch its Merkle path.
6. Construct a `claim_utxo` transaction with PQ signature + Plonky3 STARK proof.
7. Submit to the prototype node; verify the C1-C8 constraints pass; assert a Starstream UTxO is emitted and the nullifier `(sub_tree_id=1, leaf_index)` is now in the spent-set.

This becomes a CI integration test: small enough to run on a developer laptop, large enough to exercise every layer.

## Why preview, not preprod or mainnet

- **Preview** is the developer-facing network with frequent resets and small ledger; Mithril snapshot is gigabytes not hundreds-of-gigabytes.
- **Preprod** is closer to mainnet behaviour but heavier; reserve for v1.0 hardening once preview round-trip is green.
- **Mainnet** is the genesis-quality target; not for prototype circuits or mocked PQ keys.

The supported Mithril networks are `preview`, `preprod`, `mainnet` (Mithril client doc). Aggregator URL pattern: `https://aggregator.<network>.api.mithril.network/aggregator` (preview is `release-preview`, preprod is `release-preprod`).

## What we already have

The omega-commitment Rust workspace (`omega-commitment/`) at v0.9.1 ships:

- `omega-commitment-core` — leaf encodings, Blake3/SHA3 hashing, v2 domain-separated binary Merkle trees, inclusion witnesses for all seven sub-trees.
- `omega-commitment-ingest` — synthetic-fixture parsers for five sub-trees (UTxO, token-policy, script, stake, governance).
- `omega-commitment-bundle` — assemble + verify dual-hash bundle root.
- `omega-commitment-cli` — `commit` subcommand.
- `omega-utxo-snapshot` — pallas-network LSQ client, calls `BlockQuery::GetUTxOWhole`, buffers raw CBOR to disk. **Decoder TODO at `crates/omega-utxo-snapshot/src/main.rs:202`.**

282 unit + integration + golden tests, all green.

## What we still need to develop

Five new crates inside the existing workspace:

| Crate | Role | Status |
|---|---|---|
| `omega-claim-tx` | Public + witness types for `claim_<kind>` transactions; CBOR codec | TODO |
| `omega-claim-circuit` | Plonky3 implementation of C1-C8 (the README's verifier-proves table); Merkle membership + PQ-sig + nullifier checks | TODO |
| `omega-claim-prover` | Wallet-side: build witness, run prover, emit proof + tx | TODO |
| `omega-node-proto` | One-process "Omega node": holds genesis params + nullifier set + Starstream UTxO set; accepts claim tx; runs the verifier; mutates state | TODO |
| `omega-mithril-verify` | Wraps `mithril-client` to verify the input snapshot's certificate before ingestion | TODO |

Plus changes to the existing crates:

- `omega-utxo-snapshot`: typed `GetUTxOWhole` decoder (audit `A2/F001`, the v1.0 unblock).
- `omega-commitment-ingest`: real-mainnet (and preview-flavoured) parsers, splitting `synthetic.rs` from `mainnet.rs` per sub-tree (Task 3 in the v1.0 plan).

## What to download

Everything goes under a `var/` directory at the repo root (already conventional for `omega-commitment/scripts/download_snapshot.sh`).

| Artifact | Source | Approximate size | Use |
|---|---|---|---|
| `cardano-node` 10.x binary (preview) | IntersectMBO release tarball | ~120 MB | Headless node syncing preview |
| Preview Mithril snapshot | `mithril-client cardano-db download latest` against `aggregator.release-preview.api.mithril.network` | a few GB compressed | Bootstrap the preview node fast |
| Preview Byron + Shelley + Conway genesis files | `https://book.play.dev.cardano.org/environments/preview/{byron-genesis,shelley-genesis,alonzo-genesis,conway-genesis}.json` | < 1 MB total | `cardano-node` config inputs |
| Preview topology config | same source as genesis | < 1 KB | network bootstrap |
| Mithril `genesis-verification-key` for preview | published in `mithril/networks/release-preview/` | < 1 KB | snapshot certificate verification |
| `mithril-client` 2617.x binary | mithril release tarball | ~30 MB | snapshot download + verification |

Per-flow runtime artifacts (produced, not downloaded):

| Artifact | Source command |
|---|---|
| `utxo_*.cbor` | `omega-utxo-snapshot --network preview --era 6 --out var/utxo_<TS>.cbor` |
| `ledger_state_*.json` | `cardano-cli conway query ledger-state --testnet-magic 2 --out-file var/ledger_state_<TS>.json` |
| Per-sub-tree commitment.json + witnesses | `omega-commitment commit --sub-tree utxo --input ... --out var/commit/utxo/` |
| Bundle root tuple | `omega-bundle assemble --input-dir var/commit --out var/bundle.json` |

## Repos to fork (vs depend-on as crate or binary)

The principle: fork only when we expect to patch upstream. Default to crate-dep.

### Fork (we will patch)

| Repo | Why | Patch surface |
|---|---|---|
| [`LFDT-Nightstream/Starstream`](https://github.com/LFDT-Nightstream/Starstream) | Omega's execution model. Type-checker / IVC / MCC / lookups still TODO upstream. We will need to vendor and patch as we develop the `claim_utxo`-emits-Starstream-UTxO output type. | The Starstream UTxO emission API; possibly the folding interface for multi-claim folding (T6 ergonomics) |

### Depend (no fork unless a bug forces it)

| Crate / repo | Source | Notes |
|---|---|---|
| [`pallas-network`](https://github.com/txpipe/pallas) | crates.io 0.30.x | Already a workspace dep; LSQ + chain-sync. Fork only if we need a wire-format patch. |
| [`Plonky3/Plonky3`](https://github.com/Plonky3/Plonky3) | crates.io / git tag | STARK toolkit. Use the Goldilocks + Poseidon2 trio. No expected fork; track the tag for reproducibility. |
| [`slh-dsa`](https://crates.io/crates/slh-dsa) | crates.io | Pure-Rust SLH-DSA per FIPS 205. Use for the holder PQ key in the witness. The lattice-vs-hash decision (RESEARCH-QUESTIONS.md Q2) is unsettled but SLH-DSA is the conservative-default for a prototype. |
| [`mithril-client`](https://github.com/input-output-hk/mithril) | crates.io `mithril-client` | Wrapper crate verifies certificates before ingestion. |
| [`blake3`, `sha3`, `hex`, `serde`, `clap`](https://crates.io) | crates.io | Workspace deps; `blake3` replaces `blake2` per the 2026-05-03 migration. |

### Cardano upstream — observe-only

| Repo | Why we don't fork | Workaround if we hit a bug |
|---|---|---|
| [`IntersectMBO/cardano-cli`](https://github.com/IntersectMBO/cardano-cli) | We bypass it via `omega-utxo-snapshot` for UTxO; we use it as-is for the JSON ledger-state dump (stake + governance). | PR `IntersectMBO/cardano-cli#1350` (Cabal SRP against `cardano-ledger`) is the upstream fix for the Word16-VLE TxIx bug. Track its merge; until then `omega-utxo-snapshot` is canonical. |
| [`IntersectMBO/cardano-ledger`](https://github.com/IntersectMBO/cardano-ledger) | Reference impl; we read CBOR shapes from it but never patch. | If the ledger-state JSON layout changes, update the path map in `wiki/pages/ledger-state-json-layout.md` and re-pin the parser. |
| [`IntersectMBO/cardano-node`](https://github.com/IntersectMBO/cardano-node) | We run the binary; we never touch its source. | None expected. |

## End-to-end flow as it will run on preview

```
┌────────────────────────────────────────────────────────────────────────┐
│ 1. SETUP (one-time per machine)                                        │
└────────────────────────────────────────────────────────────────────────┘
  - cardano-node 10.x (preview) installed, configured, synced via Mithril
  - var/ledger_state_<TS>.json produced (small on preview: tens of MB)
  - var/utxo_<TS>.cbor produced via omega-utxo-snapshot

┌────────────────────────────────────────────────────────────────────────┐
│ 2. INGEST → Ω-COMMITMENT                                               │
└────────────────────────────────────────────────────────────────────────┘
  omega-ingest utxo  --format mainnet --in var/utxo_*.cbor    --out var/commit/utxo/
  omega-ingest stake --format mainnet --in var/ledger_state_*.json --out var/commit/stake/
  omega-ingest governance --format mainnet --in ... --out var/commit/governance/
  omega-ingest token-policy ...
  omega-ingest script ...
  (header + tx-index sub-trees: v1.1, mocked to all-zero for the prototype)

  omega-bundle assemble --input-dir var/commit --out var/bundle.json
  → produces (bundle_blake3, bundle_sha3) — the Ω-Commitment

┌────────────────────────────────────────────────────────────────────────┐
│ 3. PROTOTYPE OMEGA NODE STARTS                                         │
└────────────────────────────────────────────────────────────────────────┘
  omega-node-proto --genesis var/bundle.json
    - reads (bundle_blake3, bundle_sha3) into in-memory genesis params
    - holds: nullifier_set: HashSet<(u8, u64)>, starstream_utxos: Vec<...>
    - listens on localhost (TCP or unix socket) for claim_<kind> transactions

┌────────────────────────────────────────────────────────────────────────┐
│ 4. WALLET SIDE: BUILD A claim_utxo                                     │
└────────────────────────────────────────────────────────────────────────┘
  Pick UTxO at canonical_index = K from var/commit/utxo/
  omega-claim-prover \
    --sub-tree-id 1 \
    --leaf-index K \
    --commit-dir var/commit \
    --bundle var/bundle.json \
    --signing-key var/keys/holder_slh_dsa.key \
    --recipient <starstream-addr> \
    --out var/claim_utxo_K.tx

  Internally:
    - assemble witness payload (tx_id, output_index, address, value, assets, datum, script)
    - assemble Merkle path (omega-commitment-core::InclusionWitness)
    - sign (sub_tree_id || leaf_index || recipient || nullifier) under SLH-DSA
    - run plonky3 prover over the C1-C8 circuit
    - emit (PUBLIC_INPUTS, PROOF) bundled as claim_utxo_K.tx

┌────────────────────────────────────────────────────────────────────────┐
│ 5. NODE SIDE: VERIFY                                                   │
└────────────────────────────────────────────────────────────────────────┘
  echo claim_utxo_K.tx | omega-node-proto submit
    - parse public inputs
    - assert bundle_root matches genesis bundle_blake3
    - run plonky3 verifier over the proof
    - assert nullifier ∉ nullifier_set
    - on success:
        nullifier_set.insert((sub_tree_id, leaf_index))
        starstream_utxos.push(StarstreamUtxo { recipient, value, ... })
        burn_fee()
    - on failure: drop, log, return error

┌────────────────────────────────────────────────────────────────────────┐
│ 6. ASSERTION (the integration test)                                    │
└────────────────────────────────────────────────────────────────────────┘
  - HTTP/RPC GET /state ⇒ nullifier (1, K) present, exactly one Starstream UTxO
    with the expected (recipient, value, asset_bundle, datum, script).
  - Replay the same claim_utxo_K.tx ⇒ rejected as nullifier-collision.
  - Tamper the proof or witness ⇒ rejected at C1-C8.
```

## Constraint mapping (what each Plonky3 gadget proves)

The README's C1-C8 table is the spec. The first prototype implements all eight in pure Rust (no recursion, no folding) — folding lives at v0.2 of the circuit when multi-claim aggregation matters.

| Constraint | Plonky3 gadget shape | Cost dominant in |
|---|---|---|
| C1: leaf hash binds `(sub_tree_id, leaf_index, len, payload)` | Variable-length Blake3 AIR | Hash compression |
| C2: leaf_index < item_count[sub_tree_id] | Range check | One comparison |
| C3: walk Merkle path; each node is `H_blake3("omega:v2:node" \|\| left \|\| right)` | Fixed-arity binary tree gadget | Hash compression × tree depth |
| C4: terminal root == per-sub-tree root in genesis | Equality | One comparison |
| C5: bundle_blake3 == H(root_1 \|\| ... \|\| root_7) | Hash | One compression call |
| C6: SLH-DSA signature verifies against address-derived public key | SLH-DSA verify gadget | Dominant — SLH-DSA verification is large |
| C7: nullifier == PLUME-style derivation from K_pq + (sub_tree_id, leaf_index) | Hash + key-binding | Two compressions |
| C8: nullifier ∉ ledger.nullifier_set | OFF-CIRCUIT (state read at submit time) | Hash-set lookup |

The prototype circuit is one big AIR. v0.2 splits into per-constraint gadgets composed via Plonky3's permutation argument. v0.3 introduces folding via Starstream's IVC interface so a multi-sub-tree claim is one recursive proof.

## OpenSpec workflow

The repo now has `openspec/` initialised at the root with `--profile core --tools claude`. The workflow for any of the new crates is:

```
/opsx:propose "add omega-claim-circuit crate"
  → openspec/changes/<id>/proposal.md, specs/, design.md, tasks.md
/opsx:apply
  → implements the tasks.md checklist
/opsx:archive
  → moves the change to openspec/changes/archive/<date>-<id>/, merges specs into openspec/specs/
```

The current `omega-commitment/` workspace was developed before OpenSpec was introduced; future changes (the five new crates above) should land via OpenSpec proposals so the spec evolves alongside the code.

## Sequencing

1. **Unblock the existing pipeline** — finish `omega-utxo-snapshot` typed decoder (v1.0 Task 4). Without this, no real-data ingest works.
2. **Real-data ingest for stake + governance** — Tasks 7 + 8 of the v1.0 plan. Then UTxO + token-policy + script (Tasks 4-6).
3. **Add `omega-claim-tx`** — types only; CBOR codec; round-trip tests. No circuit yet.
4. **Add `omega-claim-circuit`** — Plonky3 AIR for C1-C5 only first (membership half). Use a mock signature for C6 (Ed25519 stand-in until SLH-DSA gadget lands). Skip C7-C8.
5. **Add `omega-node-proto`** — accept claim_tx, verify the C1-C5 circuit, mutate state.
6. **Round-trip test on preview** — single UTxO end-to-end; this is the demo.
7. **Add SLH-DSA gadget for C6** and PLUME nullifier for C7. Re-run.
8. **Add `omega-mithril-verify`** — refuse to ingest a snapshot whose Mithril cert does not verify against the published genesis verification key.

## Success criteria

- One green integration test in CI that exercises steps 2-6 against a hand-crafted `preview-fixture.cbor` (so CI does not depend on a synced node).
- One manual runbook that exercises the same flow against a real preview node (gated, multi-GB, not in CI).
- One openspec change archived for each of the five new crates.

## Spike: real ZK proofs from upstream, measured 2026-05-03

Both upstream proving stacks build and run their canonical examples without modification on a stock Rust 1.90 toolchain. The point of the spike was to confirm we can wire either one into the Omega claim circuit without a long cold-start engineering project on the proving primitives.

### Plonky3 (BabyBear, 1024 Blake-3 permutations, Poseidon2 Merkle)

Cloned `Plonky3/Plonky3` HEAD into `var/upstream/Plonky3/`. Built `prove_prime_field_31` example in release mode (1m47s, all workspace crates). Ran:

```bash
./target/release/examples/prove_prime_field_31.exe \
  --field babybear --objective blake3-permutations --log-trace-length 10 \
  --discrete-fourier-transform recursive-dft --merkle-hash poseidon2
```

Result:

```
Proving 2^10 Blake-3 permutations
prove        3.43s
  commit-to-trace      2.71s (79%)
  open / FRI            295ms
verify        204ms
Proof size:   4,908,932 bytes (4.9 MiB)
Proof Verified Successfully
```

This is the exact primitive set §1 of the design spec mandates: BabyBear or Goldilocks field, Poseidon2 in-circuit hashing, no curve operations. The 4.9 MiB proof for 2^10 Blake-3 invocations is a useful baseline for what the Ω-Commitment Merkle-membership half of the C1-C5 verifier will cost.

### Starstream interleaving-proof (Nightstream-fold, BN254 + Goldilocks)

Cloned `LFDT-Nightstream/Starstream` HEAD into `var/upstream/Starstream/`. The `interleaving/starstream-interleaving-proof` crate pulls `neo-fold`, `neo-ccs`, `neo-ajtai`, `neo-params`, `neo-vm-trace`, `neo-memory` from the sibling `LFDT-Nightstream/Nightstream` repo at pinned rev `8b32cc8f`. Build of dev-profile tests took 2m07s; the only patch needed is the workspace's existing `[patch.crates-io] ark-relations = git`.

Ran one of 11 unit tests in `circuit_test.rs`:

```bash
cargo test --package starstream-interleaving-proof test_circuit_small -- --nocapture
```

Result (excerpt):

```
making proof, steps 40
mem tracing instr NewRef { size: 1, ret: 0 }
mem tracing instr RefPush { vals: [0, 0, 0, 0] }
mem tracing instr NewUtxo { program_hash: [0, 0, 0, 0], val: 0, target: 0 }
mem tracing instr Resume { target: 0, f_id: 0, val: 0, ret: 0, caller: OptionalF(0) }
mem tracing instr Enter { f_id: 0 }
mem tracing instr Yield { val: 0 }
num constraints 1174
num instance variables 1
num variables 1248
preprocess_shared_bus_r1cs took 312ms
[neo-fold] Cache miss: synthesizing circuit preprocessing (SparseCache + matrix digest).
proof generated in 92410 ms
test circuit_test::test_circuit_small ... ok
```

This is a real folded ZK proof of a 6-step coroutine trace (1174 R1CS constraints, 1248 variables) using the Ajtai-lattice-based Nightstream-fold scheme. The 92-second cost is debug-profile; release-profile would be substantially faster. The 6 instructions exercised — `NewRef`, `RefPush`, `NewUtxo`, `Resume`, `Enter`, `Yield` — are exactly the coroutine primitives Omega's `claim_utxo` will use to emit a Starstream UTxO.

### What this means for the build path

Both crate trees compile without forking. We can vendor either as a workspace dep behind a thin Cargo path-or-git declaration:

```toml
# In omega-commitment/Cargo.toml or a new omega-claim-circuit/Cargo.toml:
p3-uni-stark      = { git = "https://github.com/Plonky3/Plonky3", rev = "<pinned>" }
p3-blake3-air     = { git = "https://github.com/Plonky3/Plonky3", rev = "<pinned>" }
p3-poseidon2-air  = { git = "https://github.com/Plonky3/Plonky3", rev = "<pinned>" }
p3-baby-bear      = { git = "https://github.com/Plonky3/Plonky3", rev = "<pinned>" }

# Or, for the Starstream-folded route:
starstream-interleaving-proof = { git = "https://github.com/LFDT-Nightstream/Starstream", rev = "<pinned>" }
```

For the prototype, **start with Plonky3 alone** for C1-C5 (Merkle membership). Add the Starstream `claim_utxo`-emits-Starstream-UTxO output type later by vendoring `starstream-interleaving-spec` and feeding `WitLedgerEffect` records into the existing interleaving circuit. The 22 WASM host functions in `starstream-runtime/src/lib.rs` are the Omega-side coroutine ABI we will eventually call.

No fork of Starstream is required for a first end-to-end demo. Forking becomes useful only when we need to extend the `Instruction` / `WitLedgerEffect` enums with a `ClaimSubtreeRoot { root, sub_tree_id, leaf_index }` variant that emits the resurrected UTxO with a domain-separated provenance tag. That's a v0.2 of the prototype, not v0.1.

### Next concrete code task

Three files in a new `omega-commitment/crates/omega-claim-circuit/` — Plonky3-only first, no Starstream yet:

1. `src/air.rs` — `OmegaMembershipAir`, an AIR with one trace per Merkle path step. Uses `p3-blake3-air` directly from upstream Plonky3 — no custom hash AIR required after the 2026-05-03 Blake3 migration.
2. `src/prover.rs` — wraps `p3-uni-stark::prove` with the exact field + Poseidon2 Merkle config from `prove_prime_field_31.rs`. Public input is `(bundle_blake3, sub_tree_id, leaf_index, item_count)`. Witness is `(payload, merkle_path)`.
3. `tests/membership_e2e.rs` — generates a 256-leaf synthetic UTxO sub-tree using the existing `omega-commitment-core::Tree` builder, picks a random leaf, runs the prover, runs the verifier, asserts pass. Plus a tampered-witness negative test.

### Blake3 inside the verifier circuit

As of the 2026-05-03 design upgrade, the Ω-Commitment hashes with Blake3 (replacing Blake3). Plonky3 ships `p3-blake3-air` out of the box, so the C1 (leaf hash) and C3 (Merkle node hash) constraints compile against the upstream gadget without a custom AIR. The migration spec is at `docs/superpowers/specs/2026-05-03-blake3-migration-design.md`. Domain tags bump from `omega:v1:*` to `omega:v2:*` so post-migration preimages cannot collide with v0.9.x outputs.

## What this demo does not prove

- Anything about T2 consensus (Crypsinous / Chronos / Minotaur). The prototype node is single-process, no validator set, no VRF.
- Anything about T3 Starstream beyond emitting a typed UTxO record. No coroutine execution, no folding.
- Anything about T5 mirror partnerchain. The prototype node holds the snapshot in-process.
- Mass-MPC genesis ceremony. The "genesis" is just a JSON file pinned at startup.
- Multi-claim folding. Each `claim_<kind>` produces one independent proof.

These are downstream of T1 + T6 prototype landing. The point of the demo is to flush out the C1-C8 verifier interface end-to-end and prove the seven-sub-tree commitment round-trips against a live testnet.

## References

- Cardano Developer Portal — Testnets: https://developers.cardano.org/docs/get-started/networks/testnets/
- Mithril client manual: https://mithril.network/doc/manual/develop/nodes/mithril-client/
- pallas-network on crates.io: https://crates.io/crates/pallas-network
- Plonky3 toolkit: https://github.com/Plonky3/Plonky3
- slh-dsa Rust crate: https://crates.io/crates/slh-dsa
- Starstream impl-plan: https://github.com/LFDT-Nightstream/Starstream/blob/main/impl-plan.md
- IntersectMBO/cardano-cli PR #1350 (TxIx fix): https://github.com/IntersectMBO/cardano-cli/pull/1350
- OpenSpec getting started: https://github.com/Fission-AI/OpenSpec/blob/main/docs/getting-started.md
