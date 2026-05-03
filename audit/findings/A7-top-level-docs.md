---
agent: A7
lane: docs
title: top-level-docs
files-reviewed: [README.md, ARCHITECTURE.md, GOALS.md, cardano-wiki/README.md, omega-commitment/README.md, cardano-wiki/wiki/pages/ledger-state-json-layout.md, cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md, cardano-wiki/wiki/pages/spec-ouroboros-omega.md, cardano-wiki/docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md, cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.1-chain-follower-plan.md, omega-commitment/Cargo.toml, omega-commitment/crates/omega-commitment-core/src/tree.rs, omega-commitment/crates/omega-commitment-core/src/hash.rs, omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs, omega-commitment/crates/omega-commitment-bundle/src/bundle.rs, omega-commitment/crates/omega-commitment-bundle/src/recompute.rs, omega-commitment/crates/omega-commitment-bundle/src/sub_tree_id.rs, omega-commitment/crates/omega-commitment-ingest/src/main.rs, omega-commitment/crates/omega-utxo-snapshot/src/main.rs, omega-commitment/scripts/dump_ledger_state.sh, omega-commitment/scripts/setup_headless_node.md]
findings-count: { p0: 0, p1: 2, p2: 3, p3: 1 }
---

# Summary

The scoped Markdown links resolve, and the main leaf-size, ledger-count, and v0.9.1 test-count claims are backed by code or pinned wiki pages. The publication blockers are accuracy mismatches in the top-level crypto/data-flow prose: README.md asserts domain-separated Merkle hashing that the current code does not implement, and README.md / ARCHITECTURE.md describe a SHA3 bundle-root construction that differs from the bundle crate. The remaining findings are stale task/command prose that would send a follow-on worker down the wrong path.

# Findings

## F001 — README claims Merkle domain separation that is absent from code

- **Severity:** P1
- **Confidence:** high
- **Location:** `README.md:194`
- **Issue:** The README states that every leaf is bound to `(sub_tree_id, leaf_index, payload_hash)` and that internal nodes carry a distinct separator byte. The implemented tree hashes raw 32-byte leaf hashes and raw `left || right` internal nodes without those domain tags, so the top-level security invariant is false as written.
- **Evidence:**
```text
README.md:194: Domain separation matters. Every leaf is bound to `(sub_tree_id, leaf_index, payload_hash)` before it enters the tree, and every internal node carries a separator byte distinct from leaves.
omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs:80-82: pub fn leaf_hash(&self) -> Result<Hash, LeafError> { Ok(blake2b_256(&self.encode()?)) }
omega-commitment/crates/omega-commitment-core/src/tree.rs:40-43: buf[..32].copy_from_slice(&chunk[0]); ... next.push(blake2b_256(&buf));
```
- **Suggested fix:** Either implement the documented invariant and re-pin vectors, or change README.md to say v0.9.1 currently uses `leaf_hash = Blake2b-256(canonical_leaf_encoding)` and `node_hash = Blake2b-256(left || right)`. If implementing, use explicit tags such as `0x00 || sub_tree_id || leaf_index || payload_hash` for leaves and `0x01 || left || right` for internal nodes.
- **Verification:** Run `rg -n "Domain separation matters|leaf_hash\\(&self\\)|next.push\\(blake2b_256" README.md omega-commitment/crates/omega-commitment-core/src`.

## F002 — Top-level docs define the SHA3 bundle root differently from the bundle crate

- **Severity:** P1
- **Confidence:** high
- **Location:** `ARCHITECTURE.md:41-66`, `README.md:103-112`
- **Issue:** The top-level docs say `bundle_root_sha3` is `sha3` over the same seven Blake2b sub-tree roots used by the Blake2b bundle root. The code computes each sub-tree's `sha3_root` first and then aggregates those seven SHA3 roots, so a second implementation following ARCHITECTURE.md would derive a different 64-byte commitment tuple.
- **Evidence:**
```text
ARCHITECTURE.md:51:   bundle_root_sha3 = sha3(same concatenation)
ARCHITECTURE.md:62: Then both Blake2b and SHA3 hash the seven-root concatenation, yielding two bundle roots published as a tuple.
omega-commitment/crates/omega-commitment-bundle/src/recompute.rs:76: let sha3_root = sha3_root_of(leaves);
omega-commitment/crates/omega-commitment-bundle/src/bundle.rs:114-119: fn aggregate_sha3(...) { ... buf.extend_from_slice(&r.sha3_root); sha3_256(&buf) }
omega-commitment/README.md:578-581: sha3_bundle_root = SHA3-256(utxo_sha3_root || ... || governance_sha3_root)
```
- **Suggested fix:** Make README.md and ARCHITECTURE.md match the code and omega-commitment README: define `sha3_root` per sub-tree, specify how it is computed, and define `sha3_bundle_root = SHA3-256(concat seven sha3_root values)`. If the top-level prose is intended to be canonical instead, change `omega-commitment-bundle` and re-pin bundle goldens.
- **Verification:** Run `rg -n "same concatenation|Then both Blake2b and SHA3|sha3_root_of|aggregate_sha3|sha3_bundle_root = SHA3-256" ARCHITECTURE.md README.md omega-commitment`.

## F003 — README architecture diagram omits the chain-follower input for header and tx-index

- **Severity:** P2
- **Confidence:** high
- **Location:** `README.md:70-97`
- **Issue:** The ASCII diagram routes only two inputs, `omega-utxo-snapshot` CBOR and ledger-state JSON, into `omega-ingest`, then shows all seven leaf sets including Header and Tx-idx. The v1.0/v1.1 plans and current `omega-ingest` CLI make header and tx-index a separate v1.1 chain-follower path, not outputs of those two v1.0 streams.
- **Evidence:**
```text
README.md:90-97: │ UTXO │ Header │ Tx-idx │ Token │ Script │ Stake │ Gov │ ... utxo root header root tx-idx root ...
README.md:260-266: v1.1 — chain-follower for the remaining 2 sub-trees ... header chain and transaction index ... require walking every block
GOALS.md:68-72: v1.1 — chain-follower for the remaining 2 sub-trees ... require walking every block from genesis ...
omega-commitment/crates/omega-commitment-ingest/src/main.rs:25-62: enum Cmd { Utxo, TokenPolicy, Script, Stake, Governance }
```
- **Suggested fix:** Add a third diagram lane for the v1.1 chain-follower / block NDJSON source feeding Header and Tx-idx leaves, or label Header and Tx-idx as v1.1 placeholders outside the v1.0 two-stream pipeline.
- **Verification:** Run `rg -n "Header \\| Tx-idx|chain-follower for the remaining 2|enum Cmd|Utxo|TokenPolicy" README.md GOALS.md omega-commitment/crates/omega-commitment-ingest/src/main.rs`.

## F004 — README Task 13 does not match the v1.0 plan text

- **Severity:** P2
- **Confidence:** high
- **Location:** `README.md:252-258`
- **Issue:** README.md says the 2026-05-03 revision made the pallas-vs-Koios decision matrix moot and Task 13 should be reframed or deleted. The active v1.0 plan still contains `Task 13: Document pallas-vs-Koios decision matrix` and earlier plan text still describes Koios as a fallback, so the top-level To-Do list is not actually synchronized with the plan it links.
- **Evidence:**
```text
README.md:254: The 2026-05-03 architecture revision made the question moot ... Task 13 either updates this task ... or deletes it as obsolete.
cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md:15: Pallas-traverse is the primary parser; Koios REST is the per-sub-tree fallback ...
cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md:1605: ## Task 13: Document pallas-vs-Koios decision matrix
```
- **Suggested fix:** Update the v1.0 plan to supersede Task 13 explicitly, or change README.md to match the plan's current Task 13 until the plan is revised. The two files should use the same task title and disposition.
- **Verification:** Run `rg -n "Task 13|Koios no longer|pallas-vs-Koios|Pallas-traverse is the primary parser" README.md cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`.

## F005 — omega-commitment README still has stale runnable-command and test-count prose

- **Severity:** P2
- **Confidence:** high
- **Location:** `omega-commitment/README.md:64-67`, `omega-commitment/README.md:642-650`
- **Issue:** The build/test command block still says `cargo test --workspace # 26 tests`, while the same README later says v0.9.1 corrected the count to 248. The `omega-ingest` code fence also still marks four subcommands as `SCAFFOLD - unimplemented` and omits their required `--output` flags, even though the current CLI requires output paths and calls implemented runners.
- **Evidence:**
```text
omega-commitment/README.md:64-67: cargo test --workspace          # 26 tests
omega-commitment/README.md:646-649: omega-ingest token-policy --input ...   # SCAFFOLD - unimplemented
omega-commitment/README.md:748-749: actual count was 228 in v0.9.0. Corrected to 248 in v0.9.1.
omega-commitment/crates/omega-commitment-ingest/src/main.rs:35-62: TokenPolicy/Script/Stake/Governance each declare input and output PathBuf args.
omega-commitment/crates/omega-commitment-ingest/src/main.rs:89-135: run_token_policy/run_script/run_stake/run_governance write JSON output.
```
- **Suggested fix:** Update the test comment to 248 or remove the inline count. Replace the `omega-ingest` block with current runnable forms, for example `omega-ingest token-policy --input path/to/snapshot.cbor --output path/to/token_policies.json`, and remove the scaffold comments.
- **Verification:** Run `rg -n "26 tests|SCAFFOLD - unimplemented|Corrected to 248|run_token_policy|output: PathBuf" omega-commitment/README.md omega-commitment/crates/omega-commitment-ingest/src/main.rs`.

## F006 — Signature-size ranges are not pinned in repo-local evidence

- **Severity:** P3
- **Confidence:** medium
- **Location:** `README.md:198`
- **Issue:** README.md gives exact post-quantum signature-size ranges for SLH-DSA and ML-DSA / FN-DSA, but no repo-local wiki page or spec table pins those ranges. The wiki/spec files mention FIPS 204/205/206 labels, but the cited 7-50 KB and 0.7-4 KB ranges are unsupported inside this repository.
- **Evidence:**
```text
README.md:198: SLH-DSA (FIPS 205, hash-only, 7–50 KB signatures) and ML-DSA / FN-DSA (FIPS 204 / 206, lattice-based, 0.7–4 KB signatures)
cardano-wiki/docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md:303: NIST PQ standards: ML-DSA (FIPS 204), ML-KEM (FIPS 203), SLH-DSA (FIPS 205)
```
- **Suggested fix:** Add a wiki/spec table that pins signature-size ranges by parameter set with source references, then link README.md to it, or remove the exact ranges from README.md until they are sourced locally.
- **Verification:** Run `rg -n "7–50 KB|0.7–4 KB|FIPS 205|FIPS 204|FIPS 206" README.md GOALS.md ARCHITECTURE.md cardano-wiki`.
