# Charles Hoskinson — experiments

Working space for in-progress research and prototypes. Two pieces here belong to one program: **Ouroboros Omega**, a clean-slate post-quantum redesign of Cardano with cryptographic continuity to every prior era of the chain.

| Subdirectory | What it is | Status |
|---|---|---|
| [`omega-commitment/`](./omega-commitment/) | Rust workspace producing the Ω-Commitment, a single hash that captures the entire pre-fork Cardano state across 7 sub-trees | v0.9.1 (89 commits, 248 tests) |
| [`cardano-wiki/`](./cardano-wiki/) | LLM-maintained research wiki: Cardano consensus, EUTXO, Plutus, Hydra, Mithril, Leios, Voltaire governance, plus the Omega program design and the v1.0 ingestion plans | Living document |

## What is Ouroboros Omega?

Cardano shipped in 2017 against an elliptic-curve stack: Ed25519 for ordinary signatures, Praos VRF for slot-leader election, KES for forging keys, and BLS12-381 underneath Mithril certificates. All four break against a sufficiently large quantum computer, and the timeline for that machine is no longer hand-wavy. NIST finalized its first batch of post-quantum standards in 2024. Operational target dates inside national-security agencies for finishing PQ migrations now sit between 2030 and 2035. That is one Cardano upgrade cycle.

Two honest options. Migrate Cardano in place, layering hash-based or lattice-based primitives over the existing curve-based stack and managing the compatibility tax forever. Or build the new chain you would have built in 2017 if you had known what you know now, and provide a one-way bridge so existing holders are not stranded. Omega chooses the second. The argument is not that incremental migration is impossible. It is that incremental migration produces a worse final design and never quite finishes, because every transition window between primitives demands its own coordination ritual.

The trickiest part of a clean-slate fork is the existing state. Cardano has roughly ten million UTxOs, 2.5 million stake credentials, 2,940 stake pools, a thousand DReps, and eight years of block history. None of that should disappear when the new chain starts. None of it should sit in Omega's genesis ledger either, pre-loaded and ready to be re-validated by Omega's nodes. That would force every Omega validator to carry the entire historical weight of Cardano forever, which is the burden the redesign was meant to shed.

The Ω-Commitment resolves the tension. It is one Merkle root committing to seven aspects of pre-fork Cardano state: UTxOs, block headers, transaction index, native token policies, scripts, stake state, and governance state. The root is published once in Omega's genesis block, and then Omega's ledger is empty. State migration is pull-based. A holder who wants their old UTxO back submits a `claim_utxo` transaction with a Merkle membership proof against the published root, and a plonky3 verifier inside Omega's ledger confirms the proof. The address gets credited. Nothing else moves. Dust addresses that nobody bothers to claim cost the new chain nothing.

That is the program in one paragraph. The work in this repository is the smallest of twelve tracks: the tooling that computes the Ω-Commitment from real Cardano mainnet state and produces regression-tested per-sub-tree roots and the dual-hash bundle root. The other eleven tracks live in the wiki for now. Track T6, the plonky3 verifier, is what consumes this commitment on the Omega side, but T6 cannot start until the commitment is fully specified and reproducible against a real snapshot. This repo is the gating dependency for most of what comes next.

## How to read this repo

Both subdirectories are self-contained. To run code, go to `omega-commitment/`, run `cargo test --workspace`, and read the per-crate READMEs. The workspace has five member crates (`omega-commitment-core` library, `omega-commitment-bundle` and `omega-commitment-ingest` library+binary pairs, plus standalone `omega-commitment-cli` and `omega-utxo-snapshot` binaries) and a tests tree with three layers of golden vectors that catch the regression categories that have broken in past versions. Test count is 248 as of v0.9.1, all green.

For design rationale, go to `cardano-wiki/`. The wiki is flat by design: every page lives in `wiki/pages/`, slugged by topic, indexed in `wiki/index.md`. The single most important page is [`wiki/pages/spec-ouroboros-omega.md`](./cardano-wiki/wiki/pages/spec-ouroboros-omega.md). The two most recent pages, written 2026-05-03, document the v1.0 ingestion pipeline and are the right place to start if you want to understand what the code is being wired up to do against real mainnet.

The decision log lives at [`cardano-wiki/wiki/log.md`](./cardano-wiki/wiki/log.md). It is append-only, organized by date, and records every architecture pivot, audit finding, verification result, and discovery that affects the program. Reading the last three or four entries gives you the current state of play more efficiently than any other artifact in the repo. If a question about why the code is the way it is is not answered by the code, the log entry near the relevant date usually answers it.

Implementation plans live under [`cardano-wiki/docs/superpowers/plans/`](./cardano-wiki/docs/superpowers/plans/). They are written in a format executable by a coding agent, but humans read them fine. The two active ones are [`2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`](./cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md) (the in-progress work, plus the 2026-05-03 architecture revision at the top) and [`2026-05-01-omega-v1.1-chain-follower-plan.md`](./cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.1-chain-follower-plan.md) (next milestone, planned but not started). Older v0.x.x plans are in the same directory and are mostly historical.

Audit-handoff briefings live under [`cardano-wiki/docs/codex_briefings/`](./cardano-wiki/docs/codex_briefings/). Each is a self-contained handoff for an LLM (Codex, currently) to do an autonomous audit pass. The 2026-05-03 brief reflects the current state of the code; the 2026-05-01 brief has a "PARTIALLY SUPERSEDED" banner pointing forward. A future agent picking up this work should start with the 2026-05-03 brief.

The Claude Code skill bundle for working on this repo (about a hundred skills covering Rust, cryptography, plonky3 circuits, code review, and the cargo verification stack) lives at [`skills/`](./skills/). On a fresh machine, run `./skills/install.sh` to reproduce the setup from the manifest. See [`skills/README.md`](./skills/README.md) for the full list and the two manual `/plugin` commands that finish the install inside Claude Code.

## Status as of 2026-05-03

| Layer | State |
|---|---|
| Synthetic-fixture ingestion (5 of 7 sub-trees) | Shipped v0.9.1 |
| Headless mainnet cardano-node (Mithril-bootstrapped) | Synced epoch 628, slot 186,209,073 |
| `omega-utxo-snapshot` LSQ client (UTxO sub-tree input) | Built; smoke-test against live mainnet in flight |
| Real-mainnet ingestion (5 sub-trees) | v1.0 in progress, parser implementation next |
| Chain-follower for header + tx-index sub-trees | v1.1 planned |

The work that landed in the last 72 hours rewrote my mental model of v1.0. The original plan, written 2026-05-01, assumed a single CBOR dump of the full LedgerState produced by `cardano-cli query ledger-state --output-cbor`. That command does not exist in cardano-cli 10.16; the supported output formats are JSON, text, and YAML. The CBOR path was an assumption that did not survive contact with the tool. I should have caught it at the spec stage instead of at the implementation stage.

The first recovery attempt used `cardano-cli conway query utxo --whole-utxo --output-cbor-bin` for the UTxO sub-tree and the JSON ledger-state dump for everything else. The `--whole-utxo` invocation died after consuming about 978 MB of the response stream, with a Haskell decoder error that turned out to be an upstream bug. The cli's own help text says `--whole-utxo` is "only appropriate on small testnets." The bug is a 16-bit asymmetry in the encoder/decoder pair for pointer-address transaction indices: the encoder writes Word64-VLE, the decoder expects Word16-VLE, and mainnet's historical record contains TxIx values above 2^16. PR `IntersectMBO/cardano-cli#1350` carries the fix and has been open since March 2026 without merge.

The current architecture splits the input pipeline. Stake and governance read from the existing ledger-state JSON. UTxO, native token policies, and scripts read from the output of a small Rust binary I built called `omega-utxo-snapshot`. The binary uses pallas-network 0.30.2's local-state-query miniprotocol to issue the same `BlockQuery::GetUTxOWhole` query that cardano-cli would have issued; pallas's CBOR decoder does not share Haskell's 16-bit TxIx asymmetry. An independent agent verified the wire bytes layer-by-layer against ouroboros-consensus, and the smoke-test against a live mainnet node has been running healthy for over twenty minutes at the time of writing.

Two new wiki pages document this story. [`wiki/pages/lsq-getutxowhole-pipeline.md`](./cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md) explains why the cli path does not work and what was built instead. [`wiki/pages/ledger-state-json-layout.md`](./cardano-wiki/wiki/pages/ledger-state-json-layout.md) records the JSON path map for the stake and governance ingestion code, with verified entity counts from a real 2 GiB mainnet dump. Both pages are linked from `wiki/index.md` under a new "Mainnet Ingestion (omega-commitment v1.0)" category.

What remains, after the smoke-test lands, is roughly two dozen implementation tasks across v1.0 and v1.1. The v1.0 tasks finish the five-of-seven ingestion path against real mainnet data and produce the first real-data golden vector. The v1.1 tasks build the chain-follower that emits the remaining two sub-trees and, at the same epoch boundary as the v1.0 anchor, produces the complete seven-of-seven mainnet bundle root tuple. The task list is in the next section.

## Zero-knowledge architecture

The Ω-Commitment is one half of a two-part construction. This repository builds the commitment from real Cardano state. The other half, which lives in the planned T6 track, is a plonky3 STARK verifier that consumes claim transactions on Omega and confirms that a holder's claimed pre-fork state was present in the commitment. The diagram below traces the full lifecycle from snapshot to claim, with the cryptographic objects and trust boundaries marked at each stage.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                  PRE-FORK CONSTRUCTION  (off-chain, one-time)                │
└──────────────────────────────────────────────────────────────────────────────┘

  Cardano mainnet
  (snapshot at epoch N, ≥ k=2160 blocks deep)
        │
        ├──► Mithril certificate ◄── trust boundary #1
        │       (stake-based threshold sig over snapshot bytes)
        │
        ▼
  ┌─────────────────────┐         ┌──────────────────────────┐
  │ omega-utxo-snapshot │         │ cardano-cli              │
  │ (pallas-network LSQ)│         │ conway query ledger-state│
  │  BlockQuery::       │         │  --output-json           │
  │  GetUTxOWhole       │         │                          │
  └──────────┬──────────┘         └────────────┬─────────────┘
             │                                 │
             ▼                                 ▼
       utxo_*.cbor                    ledger_state_*.json
       (~ multi-GB)                   (~ 2 GiB)
             │                                 │
             └────────────────┬────────────────┘
                              ▼
                  ┌───────────────────────┐
                  │  omega-ingest         │
                  │  (per-sub-tree        │
                  │   parsers + leaf      │
                  │   canonicalisation)   │
                  └──────────┬────────────┘
                             ▼
       ┌───────┬────────┬────────┬────────┬────────┬────────┬────────┐
       │ UTXO  │ Header │ Tx-idx │ Token  │ Script │ Stake  │ Gov    │
       │ leaves│ leaves │ leaves │ leaves │ leaves │ leaves │ leaves │
       │ blake2b each, sorted, zero-padded to next power of 2        │
       └───┬───┴───┬────┴───┬────┴───┬────┴───┬────┴───┬────┴───┬────┘
           ▼       ▼        ▼        ▼        ▼        ▼        ▼
        utxo    header   tx-idx   token   script   stake     gov
        root    root      root    root    root     root      root
        (32 B per root, blake2b throughout the inner tree)
           │       │        │        │        │        │        │
           └───────┴────────┴────────┴───┬────┴────────┴────────┘
                                         ▼
                          ┌──────────────────────────────┐
                          │  Bundle layer (dual-hash)    │
                          │                              │
                          │  blake2b_root = blake2b(     │
                          │     concat 7 sub-tree roots) │
                          │                              │
                          │  sha3_root    = sha3(        │
                          │     concat 7 sub-tree roots) │
                          └──────────────┬───────────────┘
                                         ▼
                          Ω-Commitment = (blake2b_root, sha3_root)
                                         64 bytes total
                                         │
                          ┌──────────────┴───────────────┐
                          │   Cross-impl reproducibility │
                          │   ceremony (m-of-n co-sign,  │
                          │   transcript published       │
                          │   on Cardano pre-fork)       │
                          └──────────────┬───────────────┘
                                         ▼

┌──────────────────────────────────────────────────────────────────────────────┐
│                              GENESIS PUBLICATION                             │
└──────────────────────────────────────────────────────────────────────────────┘

  Omega genesis block
  ┌────────────────────────────────────────────┐
  │ ω-commit:    (blake2b_root, sha3_root)     │  ◄── trust boundary #2
  │ snap-block:  <pinned Cardano block hash>   │      (genesis ceremony
  │ snap-cert:   <Mithril cert hash>           │       attestor signatures)
  │ params:      plonky3 (FRI rate, queries),  │
  │              hash domain tags, era bytes   │
  └────────────────────────────────────────────┘
                                         │
                                         ▼

┌──────────────────────────────────────────────────────────────────────────────┐
│                  POST-FORK CLAIM  (per holder, lazy)                         │
└──────────────────────────────────────────────────────────────────────────────┘

  Holder  (PQ key + Cardano-side credential)
        │
        ▼
  Snapshot service                       (multi-aggregator, Mithril-certified)
  ─ get_path(credential, sub_tree) ──►   ┌─────────────────────┐
                                         │ Returns:            │
                                         │  • leaf preimage    │
                                         │  • Merkle path      │
                                         │    (~24 × 32 B)     │
                                         │  • sub-tree root    │
                                         └──────────┬──────────┘
                                                    ▼
  Holder's wallet
  ┌────────────────────────────────────────────────────────────┐
  │ Build claim_<kind> tx:                                     │
  │                                                            │
  │ public input  = (sub_tree_id, leaf_index,                  │
  │                  bundle_root, omega_recipient,             │
  │                  chain_id="omega-mainnet")                 │
  │                                                            │
  │ witness       = (leaf_preimage, merkle_path,               │
  │                  pq_signature over public input)           │
  │                                                            │
  │ proof         = plonky3_prove(circuit, public, witness)    │
  │                                                            │
  │ → submit (public_input, proof) to Omega                    │
  └────────────────────────────────────────────────────────────┘
        │
        ▼
  Omega ledger
  ┌────────────────────────────────────────────────────────────┐
  │ STARK verifier (T6)                                        │
  │   1. plonky3_verify(circuit, public_input, proof)          │
  │      ▸ recomputes leaf hash, walks 24 levels of merkle     │
  │      ▸ recomputes sub-tree root, recomputes bundle root    │
  │      ▸ asserts bundle root matches genesis ω-commit        │
  │      ▸ verifies pq_signature inside circuit                │
  │      ▸ binds chain_id + recipient (replay protection)      │
  │                                                            │
  │   2. nullifier check                                       │
  │      ▸ (sub_tree_id, leaf_index) not in consumed-set       │
  │                                                            │
  │   3. apply state transition                                │
  │      ▸ insert nullifier (one-shot per (sub-tree, leaf))    │
  │      ▸ credit recipient with the resurrected state         │
  └────────────────────────────────────────────────────────────┘
```

The construction has three trust boundaries the design treats explicitly. The first is the Mithril certificate over the snapshot bytes, which the cross-implementation reproducibility ceremony tightens by re-deriving the same seven sub-tree roots from the certified immutable database under multiple independent codebases. The second is the genesis ceremony itself, which is m-of-n co-signed by attestors who also witness both the published commitment and the Cardano block hash that anchors the snapshot epoch. The third is the plonky3 verifier circuit, which the program treats as needing its own audit at the exact FRI rate and query count baked into the genesis parameters.

A few cryptographic choices in the diagram deserve flags. The dual-hash at the bundle layer is a binding-agility hedge, not a collision-resistance hedge: a Blake2b break against a leaf or an internal node still produces a tree the SHA3 root commits to, because SHA3 only sees the bundle of seven Blake2b roots. The end-state design either runs two parallel trees (Blake2b throughout, SHA3 throughout) or treats the SHA3 root as a coordination signal for a hard re-commitment rather than a drop-in replacement. The current commitment-tooling code computes only the Blake2b inner tree; the SHA3 path is wired at the bundle layer for protocol forward-compatibility while the dual-tree question is being resolved.

Domain separation matters. Every leaf is bound to `(sub_tree_id, leaf_index, payload_hash)` before it enters the tree, and every internal node carries a separator byte distinct from leaves. Without this, the classic Merkle second-preimage swap of an internal node's preimage as a "leaf" would let an attacker mint claims for state that never existed. The leaf preimage canonicalisation is part of the spec the second implementation must reproduce byte-for-byte.

The verifier's nullifier set is keyed by `(sub_tree_id, leaf_index)` rather than by raw credential. Each leaf can only be consumed once across all sub-trees, but a single Cardano credential that holds positions in multiple sub-trees (a stake credential that also controls a UTxO that also names a DRep role, for example) submits one claim per leaf. This composes cleanly because each claim's plonky3 proof commits to a fresh `(sub_tree_id, leaf_index)` pair and the ledger refuses any second claim for the same pair.

Several design questions remain open and are flagged in the program goal map. The post-quantum signature primitive used inside the verifier circuit is a Pareto choice between SLH-DSA (FIPS 205, hash-only, 7–50 KB signatures) and ML-DSA / FN-DSA (FIPS 204 / 206, lattice-based, 0.7–4 KB signatures). The hash-based VRF that the consensus track needs is a research-frontier construction without a standardised reference. Plutus-script-locked UTxO resurrection requires either a verified Plutus Core interpreter on Omega or a coordinated re-deploy program for dApps before the snapshot. These items live on the T6, T2, and T7 track sheets respectively; the commitment construction in this repository does not pre-commit any of them.

## To do

Each item below has a one-paragraph description. Where a wiki page or plan document covers it, the link is in the heading.

### v1.0 — finish real-mainnet ingestion for the 5 LedgerState-derivable sub-trees

[Plan: `cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`](./cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md)

#### Smoke-test completion + CBOR validation

The `omega-utxo-snapshot` binary is running against a live mainnet node, accumulating the `BlockQuery::GetUTxOWhole` response in memory before writing it to disk. When the file lands, the next step is a quick parse of the first hundred or so UTxO entries via `pallas_codec::minicbor::Decoder` to confirm the output is well-formed CBOR with the expected shape (a CBOR array of 2-element arrays, each `(TransactionInput, TransactionOutput)`). If that parses, the binary is production-ready and the unblock condition for Tasks 3 through 14 is satisfied. See [`wiki/pages/lsq-getutxowhole-pipeline.md`](./cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md).

#### Task 3 — restructure ingest crate (split `synthetic.rs` + `mainnet.rs` per sub-tree)

The `omega-commitment-ingest` crate currently has one file per sub-tree, each implementing only the synthetic-fixture path. Task 3 converts each file into a module with two siblings, one for the v0.9.x synthetic format and one for v1.0's mainnet format, and wires the routing through a `--format synthetic|mainnet` CLI flag. Mechanical refactoring; touches every sub-tree but keeps the public API unchanged. The risk is the cross-cutting nature of the change. Better one commit per sub-tree than a single 2,000-line patch.

#### Task 4 — UTXO mainnet parser (consume the `omega-utxo-snapshot` CBOR)

The load-bearing implementation task of v1.0. The parser reads the multi-gigabyte CBOR file produced by `omega-utxo-snapshot` and walks the roughly ten million UTxO entries, emitting one `Utxo` struct per entry with all the fields the v0.9.1 leaf encoding requires (tx_id, output_index, address, value, multi-asset bundle, optional datum hash). Streaming matters because the file does not fit in RAM in any decoded form. Reference: `crates/omega-commitment-ingest/src/utxo.rs` (the synthetic implementation has the leaf format pinned).

#### Task 5 — Token-policy mainnet parser

The native token policy sub-tree is derived from the same UTxO walk that drives Task 4. Each UTxO carries an optional `multi_assets` field; aggregating these by `policy_id` across the full UTxO set yields the per-policy total mint amount. The first-issuance-slot field is not present in the UTxO snapshot at all, so we either join against chain history (deferred to v1.1's chain-follower) or pin it to zero with a documented limitation. Pinning to zero is the v1.0 choice. The real value gets backfilled when the chain-follower lands.

#### Task 6 — Script mainnet parser

The script registry sub-tree is also derived from the UTxO walk. Each UTxO has an optional reference-script hash; deduplicated, sorted, and combined with the script-language discriminant, this produces the registry. Same caveat as Task 5: deployment slot is pinned to zero pending chain-follower data.

#### Task 7 — Stake mainnet parser ([`wiki/pages/ledger-state-json-layout.md`](./cardano-wiki/wiki/pages/ledger-state-json-layout.md))

The stake parser reads the ledger-state JSON dump and navigates a handful of documented paths to extract stake credentials, delegations, controlled stake amounts, and rewards balances. Path map and entity counts are pinned and verified. The parser uses `serde_json::from_reader` over a `BufReader<File>`, measured at 6.47 seconds wall and a 3.24x RAM-to-file ratio against a 2 GiB dump. Output rows feed the v0.9.1 leaf encoding for the stake sub-tree without modification.

#### Task 8 — Governance mainnet parser

The governance parser reads the same JSON file as Task 7 but navigates a different set of paths. The interesting payloads are proposal records (each with full vote tallies attached), the committee state, the constitution, treasury and reserves balances, and the DRep set. The leaf encoding is `(kind, key, value, slot)` for each fact, where the kind discriminates treasury / CC seat / ratified gov action / in-flight gov action, and key/value carry the type-dependent payload. The challenge is canonicalization: two semantically-equivalent JSON serializations of the same fact must produce the same payload hash, so we either canonicalize the JSON or roundtrip through a structured Rust type before hashing.

#### Task 9 — Format auto-detection + CLI `--format` flag

Once each sub-tree has both a synthetic and a mainnet parser, the CLI needs a way to pick between them. Auto-detection is straightforward (CBOR vs JSON probe on the first few bytes), but the explicit `--format` flag is cleaner for scripting and CI. Task 9 wires both paths into the `omega-ingest` binary's existing subcommand structure.

#### Task 10 — Format-detect integration test

A fixture-driven test that confirms the auto-detection logic correctly routes synthetic and mainnet inputs through the right parser and produces matching outputs at the leaf level. Catches regressions where the format probe returns the wrong answer.

#### Task 11 — End-to-end mainnet pipeline integration test

Gated, manual, multi-gigabyte. Reads both the LedgerState JSON and the UTxO CBOR from real mainnet inputs (paths supplied via `OMEGA_LEDGER_STATE_JSON_PATH` and `OMEGA_UTXO_SNAPSHOT_PATH`), runs all five sub-tree parsers, computes the leaf-level Merkle roots, and asserts that each root is non-zero and distinct. Not part of CI: requires a synced mainnet node and 30+ minutes of runtime.

#### Task 12 — Real-data golden vector capture

After Task 11 passes, run it once at a chosen mainnet epoch boundary and pin the resulting per-sub-tree roots and bundle root tuple as the v1.0 golden vector. Document the snapshot height, the input file hashes, and the timing in `docs/golden_vectors/mainnet_v1.0_epoch_<N>.md`. This is the "5 real + 2 placeholder" intermediate result; v1.1 replaces the placeholders.

#### Task 13 — Reframe (or drop) the pallas-vs-Koios decision matrix doc

The original v1.0 plan included a task to write a doc comparing pallas-traverse against Koios REST as the mainnet ingestion strategy. The 2026-05-03 architecture revision made the question moot: pallas is the in-tree producer for one stream, the JSON cli is the producer for the other, and Koios is no longer in the picture. Task 13 either updates this task to describe the actual decision tree (when pallas wins, when JSON wins, when chain-replay wins) or deletes it as obsolete.

#### Task 14 — Bump workspace to v1.0.0 + extend README

After the real-data golden vector is pinned, bump every `Cargo.toml` to v1.0.0, extend the workspace README with v1.0 release notes (what shipped, what the goldens look like, what limitations remain), update the wiki status table, and tag the release in git. The formal "T1 v1.0 done" marker.

### v1.1 — chain-follower for the remaining 2 sub-trees

[Plan: `cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.1-chain-follower-plan.md`](./cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.1-chain-follower-plan.md)

#### v1.1 chain-follower (twelve sub-tasks)

The header chain and transaction index sub-trees cannot be derived from a snapshot alone. They require walking every block from genesis (or from a Mithril-restored recovery point) to the chosen tip. The v1.1 plan breaks this into twelve tasks: pallas-network N2C chain-sync client, per-block header decoder, per-block tx-index decoder, NDJSON streaming writer with rotation, checkpoint manager for resumability, postprocessors that fold the NDJSON into the per-sub-tree input format, two new omega-ingest subcommands, a hand-crafted block-history fixture for CI, the manual end-to-end run against real mainnet at the v1.0 epoch boundary, and the v1.1.0 version bump. The capstone is a complete seven-of-seven mainnet bundle root tuple that replaces v1.0's "5 real + 2 placeholder" intermediate.

### Tracks T2 through T12 — the rest of the program

Each of the remaining eleven tracks has a section in [`wiki/pages/spec-ouroboros-omega.md`](./cardano-wiki/wiki/pages/spec-ouroboros-omega.md) and shows up in [`GOALS.md`](./GOALS.md). The summaries below are what is currently scoped or assumed; details in the wiki page.

#### T2 — Consensus (PQ Praos)

A redesign of Praos with post-quantum primitives in place of the elliptic-curve VRF used for slot-leader election and the KES used for forging keys. Most likely candidates: a hash-based VRF (verifiable via STARK circuit) and a hash-based forging-key scheme that rotates more aggressively than KES does. The design needs to preserve Praos's stake-based selection probabilities and chain-quality theorems exactly. Spec in early drafting; no reference implementation yet.

#### T3 — Smart-contract VM

A Plutus-equivalent execution model designed to run efficiently inside a STARK circuit. Instruction set, fee model, data-type system all need revisiting. Plutus's CEK machine is not plonky3-friendly. One option is a custom RISC-style VM (Jolt-flavored) compiled to STARK constraints, with a high-level language on top. Spec in early drafting; compiler and VM not started.

#### T4 — Network stack

The Ouroboros networking miniprotocols (chain-sync, block-fetch, tx-submission, local-state-query, local-tx-monitor) ported to Omega's primitives, with PQ-handshake variants of the noise-style transport encryption. The open design question is backwards compatibility: do Omega nodes speak any Cardano-flavored protocol versions for migration tooling, or is wire incompatibility total? Not started.

#### T5 — Storage and state management

On-disk layout for Omega's UTxO state, block storage, and ledger snapshots, redesigned around the plonky3-friendly tree structure used by the Ω-Commitment. Cardano's V2InMemory and LSM backends are reasonable starting points, but the snapshot format handed to a STARK prover differs fundamentally from the format efficient for query workloads. Not started.

#### T6 — ZK verifier (plonky3 integration)

The on-chain verifier that consumes claim transactions. A claim transaction includes a Merkle membership proof against the published Ω-Commitment plus a witness that the holder controls the credential associated with the claimed UTxO (or stake position, or DRep role). The verifier checks both. The plonky3 circuit for the Merkle membership part is straightforward; the witness-control part requires PQ signature verification inside the circuit, which is where most of the cost will be. T6 cannot start until T1 ships a stable Ω-Commitment format.

#### T7 — Bridge protocol

End-to-end claim-transaction format, replay-attack resistance (each claim must be one-shot per credential), fee model (who pays for the verifier work and how much), and the policy questions around partial claims, delegation transfers, and timelocked migration windows. Spec in early drafting; depends on T6 (verifier) and T2 (consensus settling). [`wiki/pages/spec-ouroboros-omega.md`](./cardano-wiki/wiki/pages/spec-ouroboros-omega.md) has the rough decision matrix.

#### T8 — Tooling and CLI

Wallet primitives that construct claim transactions, devtools for inspecting the Ω-Commitment, debugging Merkle proofs, and simulating claims against testnet snapshots. None started; most of it cannot start usefully until T6 and T7 are firm.

#### T9 — Documentation and spec

A whitepaper and a formal protocol specification, both at the level of detail of the original Ouroboros and Plutus papers. The whitepaper makes the case for the redesign and explains the bridge mechanics. The formal spec is what auditors and second-implementation teams read. T1 has a design spec at [`wiki/pages/spec-ouroboros-omega.md`](./cardano-wiki/wiki/pages/spec-ouroboros-omega.md); the rest of T9 has not started.

#### T10 — Audits and formal verification

Third-party audits of the cryptographic primitives, the bridge protocol, and the consensus protocol. Machine-checked proofs (probably in Lean or Coq) of the critical invariants: that the verifier rejects invalid claims, that the lazy resurrection state machine is monotonic, that the dual-hash bundle root is collision-resistant under the chosen hash assumptions. Not started; needs T1, T2, T6, T7 specified.

#### T11 — Test-network operations

Devnet, internal testnet, public testnet. The full lifecycle of standing up a chain that runs Omega's protocol with rotating committees, fault-injection scenarios, and bridge-claim load tests. Not started; needs T2, T4, T5 implementations to exist.

#### T12 — Mainnet operations

Genesis ceremony, key rollout, validator onboarding, claim-window rollout schedule. Most of T12 is operational rather than technical. Not started; the launch date depends on every prior track shipping.

### Cross-cutting

#### Reproducibility-grade second implementation

A second independent implementation of the Ω-Commitment construction (probably Haskell, possibly Lean) that consumes the same mainnet snapshot and produces byte-identical roots. Required as an audit precondition for using the commitment in Omega's genesis. Not started; needs v1.1 done first so the spec is complete.

#### Mithril verification of input snapshots

Right now we trust that the Mithril snapshot we restored is correct. For genesis-quality work, the input snapshot needs its Mithril certificate independently verified and the certificate itself recorded as part of the v1.0 / v1.1 golden vector documentation. Tooling exists in `mithril-client`; a wrapper in the omega-commitment workspace is the right place to land it.

## License

Apache-2.0. See `LICENSE` at repo root.
