# omega-commitment Workspace Audit

_Source: parallel agent pass, 2026-05-03. Files audited: `omega-commitment/Cargo.toml`, all five crate manifests, `lib.rs`/`main.rs`, test files, `omega-commitment/README.md`._

## Crate topology

Five crates at v0.9.1, Apache-2.0, Rust 1.79, all in a single workspace:

| Crate | Kind | Purpose |
|---|---|---|
| `omega-commitment-core` | lib (13 modules) | Canonical leaf encodings, Blake2b/SHA3 hashing, v1 domain-separated binary Merkle trees, inclusion witnesses for all seven Ω-Commitment sub-trees |
| `omega-commitment-cli` | bin `omega-commitment` | Subcommand dispatcher; `commit --sub-tree <type>` produces `commitment.json` + per-leaf witness files |
| `omega-commitment-bundle` | lib + bin `omega-bundle` | `assemble`/`verify` seven sub-tree roots → `(blake2b_bundle_root, sha3_bundle_root)` |
| `omega-commitment-ingest` | lib + bin `omega-ingest` | Transforms CBOR LedgerState snapshots → per-sub-tree JSON; full impls for utxo/token-policy/script/stake/governance against synthetic fixtures |
| `omega-utxo-snapshot` | bin only | Acquires whole-UTXO via Node-to-Client LSQ (`BlockQuery::GetUTxOWhole`); workaround for cardano-cli `--whole-utxo` mainnet bug |

Dependency graph: cli → core, bundle → core, ingest → core, utxo-snapshot → core. Workspace deps for blake2, sha3, serde, hex, clap, anyhow, thiserror. Ingest and utxo-snapshot pin `pallas-codec`/`pallas-network`/`pallas-primitives` to **0.30.2** (Cargo.lock holds, no manifest pin).

## Per-crate purpose

- **core** — exports `leaf_hash_v1(sub_tree_id, canonical_index, payload)` and `node_hash_v1(left, right)` with ASCII tags `omega:v1:leaf` / `omega:v1:node` bound into preimages. Sort-then-pad-to-power-of-two layout. Each sub-tree exports `commit_to_subtree()` returning canonical bytes; builder rejects duplicate raw payloads (`tree.rs:106-113`). 248 unit tests + criterion benches (5.14 Melem/s at 100k UTXO).
- **cli** — `commit` reads JSON, builds `MerkleTree` via core, writes `commitment.json` (root, leaf_count, tree_depth, item_count, input_digest) and witnesses to `witnesses/<leaf_hash>.json`. `safe_child()` canonicalization prevents directory escape (`cli/src/main.rs:128-147`). 2 GiB default input-size cap (`cli/src/main.rs:41`).
- **bundle** — `assemble()` reads seven JSON files, recomputes via core leaf encoders, produces both bundle roots. `verify()` re-runs assembly and asserts both roots + per-sub-tree counts (`bundle.rs:99-122`, closes A1/F003). SHA3 root aggregates over Blake2b leaf hashes — drift detection, not break hedge (`bundle.rs:7-11`).
- **ingest** — minicbor-based parser (`ingest/src/cbor.rs`); `read_32_bytes`, `read_28_bytes`, `read_array_len`, `read_map_len`, `expect_end`. All five parse paths reject trailing bytes (`ingest/src/lib.rs:76-84`). Real Mithril LedgerState parsing deferred to v1.0.
- **utxo-snapshot** — pallas-network `NodeClient` LSQ; `BlockQuery::GetUTxOWhole`; multi-GB response buffered before single atomic write. Reproducible via `--manifest` (pins block_hash, slot, epoch, stability_depth) or experimental `--snapshot-tip`. `TODO(v1.0 Task 4)` at `main.rs:202` for typed deserialization.

## Cryptographic primitives

- **Hash family**: Blake2b-256 (primary, all leaf + internal nodes) + SHA3-256 (shadow, only at bundle aggregation). Test vectors confirmed for empty input (`hash.rs:34-50`).
- **Merkle structure**: binary tree, sorted by raw leaf-payload bytes, padded to next power of two. v1 domain separation (Batch 1 of audit response, `core/src/tree.rs:4-20`):
  - Leaf preimage: `DOMAIN_LEAF || sub_tree_id (u8) || canonical_index (u64 BE) || payload_len (u64 BE) || payload`.
  - Node preimage: `DOMAIN_NODE || left || right`.
  - Padding leaves: `leaf_hash_v1(sub_tree_id, EMPTY_INDEX_SENTINEL=u64::MAX, &[])` so verifiers reject padding-leaf inclusion proofs (`tree.rs:122-124`, closes A1/F003).
  - Duplicate payloads rejected (`tree.rs:106-113`, closes A1/F001/F002).
- **Leaf widths**: UTXO variable; Header 80 B; Tx-index 76 B; Token-policy 52 B (28-byte Cardano Blake2b-224 policy ID); Script 41 B (language: 0=native, 1/2/3=Plutus V1/V2/V3); Stake 93 B; Governance 57 B (kind: 0=treasury, 1=CC seat, 2=ratified, 3=in-flight).

## Serialization (CBOR/CDDL)

- **Input format**: JSON for `omega-commitment commit`. Each `--sub-tree` has a distinct shape (cli/src/main.rs:59-92).
- **Output**: `commitment.json`, `bundle.json`, witness JSON — all hex-encoded hashes.
- **CBOR ingest**: pallas-codec minicbor with custom helpers; canonicality enforced (sorted keys, no duplicates) — `IngestError::NonCanonical` (`ingest/src/lib.rs:86-92`). Multi-asset map ordering validated (`utxo.rs:85-89`). Trailing-byte rejection added in v0.9.1 (closes A2/F002 B2).
- **CDDL**: no formal CDDL schemas in-tree. Leaf encodings documented in module docstrings as byte layouts (e.g., `utxo_leaf.rs:1-41`). Hand-crafted fixture format documented in `tests/fixtures/ledger_state_minimal.cbor.md` and inline at `utxo.rs:28-33`.
- **u128 fields** (token supply, governance value): string-encoded JSON to survive serde_json buffering (`serde_helpers.rs:39-46`).

## Test architecture

**248 tests** total (per `omega-commitment/README.md:747`):

- **Golden vectors**:
  - `core/tests/golden_vectors.rs` — per-sub-tree roots for all seven synthetic fixtures.
  - `core/tests/golden_per_leaf.rs` — canonical bytes + v1 leaf hashes for one example per sub-tree, plus three edge cases (empty set, single-leaf, AlwaysAbstain DRep).
  - `bundle/tests/golden_bundle.rs` — bundle root tuple.
  - `ingest/tests/golden_ingest.rs` — ingestion-layer roots + hybrid bundle (5 from CBOR + 2 from JSON).
- **Per-sub-tree integration**: `core/tests/{utxo,header,tx_index,token_policy,script,stake,governance}_integration.rs` — leaf encoding, tree building, witness round-trip.
- **Ingestion integration**: `ingest/tests/qa_pipeline.rs`, `*_ingest_integration.rs` (5 files), `cli.rs`.
- **Bundle integration**: `bundle/tests/end_to_end_integration.rs`.
- **Property testing**: proptest in core (`leaf.rs::tests::encoding_is_pure`).
- **Benchmarks**: `core/benches/tree.rs` — 1k / 10k / 100k leaves.
- **Edge-case backlog (A4/F003, v1.1)**: Byron addresses, pointer addresses, large/deep trees, non-UTF-8 asset names, multiple DRep kinds, inline datums, reference scripts.

## Risk and unfinished work

**Unfinished:**
1. **Real Mithril snapshot ingestion** — synthetic CBOR fixtures only in v0.9.x. v1.0 plan documented (two-stream: cardano-cli JSON for stake/gov + omega-utxo-snapshot CBOR for utxo/token/script). Critical blocker for production.
2. **GetUTxOWhole CBOR decoder** — `omega-utxo-snapshot/main.rs:202` TODO; binary buffers raw CBOR but does not deserialize. (A2/F001 deferred.)
3. **Header + tx-index ingestion** — needs chain-follower (block-by-block walker); v1.1.
4. **Edge-case fixture corpus** (A4/F003) — multi-day fixture-expansion task; only 3 cases pinned today.

**Risky / load-bearing:**
1. **SHA3 bundle root is drift-detection only** — both roots aggregate over the same Blake2b leaf hashes; if Blake2b breaks, SHA3 root does not independently protect (`bundle.rs:7-11`). True independent SHA3 tree deferred to v2.0.
2. **Path-traversal defense not adversarially tested** — `safe_child()` is defense-in-depth; today witness filenames are hex-only. Future sub-trees with richer filenames could stress it.
3. **CBOR canonicality coverage is incomplete** — multi-asset maps validated; full LedgerState parse will need full canonicality audit of all map-keyed structures.
4. **CLI dispatcher scales** — match in `commit()` will become unwieldy at sub-tree 8; trait-registry refactor flagged but not done.

**STARK/Plonky3 relevance:**
- Workspace targets Plonky3 claim circuits (T2; `README.md:504, 602`). Per-sub-tree leaf encodings + roots locked at v0.7.0 bundle-assembly (`README.md:595-603`).
- 2026-05-01 dual-hash decision unblocks circuit authors to lock to v0.4.0+ Blake2b-only root format. Per-leaf hashing is Blake2b only; SHA3 root over Blake2b leaves at the bundle layer.
- `InclusionWitness` (leaf hash, leaf_index, hex-encoded sibling list) designed for Merkle-path verification in circuits. No circuit code in-tree; interface stable.
- No Plonky3 / recursive STARK proving code in this workspace. Commitment artifacts (roots, witnesses, bundle tuple) are the **inputs** to track T2 and the bundle-attesting layer (Mithril-PQ + recursive STARK, `README.md:88`); prover side is separate work.
