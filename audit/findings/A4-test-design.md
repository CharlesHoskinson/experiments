---
agent: A4
lane: code
title: test-design
files-reviewed: [omega-commitment/crates/omega-commitment-core/tests/golden_vectors.rs, omega-commitment/crates/omega-commitment-core/tests/utxo_integration.rs, omega-commitment/crates/omega-commitment-core/tests/header_integration.rs, omega-commitment/crates/omega-commitment-core/tests/tx_index_integration.rs, omega-commitment/crates/omega-commitment-core/tests/token_policy_integration.rs, omega-commitment/crates/omega-commitment-core/tests/script_registry_integration.rs, omega-commitment/crates/omega-commitment-core/tests/stake_state_integration.rs, omega-commitment/crates/omega-commitment-core/tests/governance_state_integration.rs, omega-commitment/crates/omega-commitment-core/tests/fixtures/utxo_set_small.json, omega-commitment/crates/omega-commitment-core/tests/fixtures/header_chain_small.json, omega-commitment/crates/omega-commitment-core/tests/fixtures/tx_index_small.json, omega-commitment/crates/omega-commitment-core/tests/fixtures/token_policies_small.json, omega-commitment/crates/omega-commitment-core/tests/fixtures/script_registry_small.json, omega-commitment/crates/omega-commitment-core/tests/fixtures/stake_state_small.json, omega-commitment/crates/omega-commitment-core/tests/fixtures/governance_state_small.json, omega-commitment/crates/omega-commitment-bundle/tests/golden_bundle.rs, omega-commitment/crates/omega-commitment-bundle/tests/end_to_end_integration.rs, omega-commitment/crates/omega-commitment-ingest/tests/golden_ingest.rs, omega-commitment/crates/omega-commitment-ingest/tests/qa_pipeline.rs, omega-commitment/crates/omega-commitment-ingest/tests/utxo_ingest_integration.rs, omega-commitment/crates/omega-commitment-ingest/tests/token_policy_ingest_integration.rs, omega-commitment/crates/omega-commitment-ingest/tests/script_ingest_integration.rs, omega-commitment/crates/omega-commitment-ingest/tests/stake_ingest_integration.rs, omega-commitment/crates/omega-commitment-ingest/tests/governance_ingest_integration.rs, omega-commitment/crates/omega-commitment-ingest/tests/cli.rs, omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_minimal.cbor.md, omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor.md, omega-commitment/crates/omega-commitment-ingest/tests/fixtures/stake_snapshot.cbor.md, omega-commitment/crates/omega-commitment-ingest/tests/fixtures/governance_snapshot.cbor.md, omega-commitment/crates/omega-commitment-cli/tests/cli.rs, omega-commitment/crates/omega-commitment-core/src/hash.rs, omega-commitment/crates/omega-commitment-core/src/tree.rs, omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs, omega-commitment/crates/omega-commitment-core/src/header_leaf.rs, omega-commitment/crates/omega-commitment-core/src/tx_index_leaf.rs, omega-commitment/crates/omega-commitment-core/src/token_policy_leaf.rs, omega-commitment/crates/omega-commitment-core/src/script_registry_leaf.rs, omega-commitment/crates/omega-commitment-core/src/stake_state_leaf.rs, omega-commitment/crates/omega-commitment-core/src/governance_state_leaf.rs, omega-commitment/crates/omega-commitment-core/src/witness.rs, omega-commitment/crates/omega-commitment-ingest/src/cbor.rs, omega-commitment/crates/omega-commitment-ingest/src/utxo.rs, omega-commitment/crates/omega-commitment-ingest/src/token_policy.rs, omega-commitment/crates/omega-commitment-ingest/src/script.rs, omega-commitment/crates/omega-commitment-ingest/src/stake.rs, omega-commitment/crates/omega-commitment-ingest/src/governance.rs, omega-commitment/crates/omega-commitment-bundle/src/recompute.rs, omega-commitment/crates/omega-commitment-bundle/src/bundle.rs, omega-commitment/crates/omega-commitment-core/Cargo.toml]
findings-count: { p0: 0, p1: 1, p2: 2, p3: 1 }
---

# Summary

The suite has useful happy-path integration coverage and pins per-sub-tree roots, ingestion roots, and bundle roots. It also has some source-level unit coverage for empty/single trees, witness rejection, malformed CBOR helpers, and basic deterministic encoding. The main gaps are at the audit boundary: domain separation is not enforced by tests, per-leaf golden vectors are absent, and the fixture corpus omits several ledger edge variants that downstream consumers are likely to depend on.

# Findings

## F001 — Hash-domain separation is not test-locked

- **Severity:** P1
- **Confidence:** medium
- **Location:** `omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs:80-82`; `omega-commitment/crates/omega-commitment-core/src/tree.rs:39-43`
- **Issue:** Leaf hashes and internal-node hashes both call raw Blake2b-256 without an explicit domain tag, and the hash tests only check known vectors / dual-hash plumbing. If untagged leaf and node hashing is intentional, there is still no regression test or spec fixture making that decision explicit; if it is not intentional, this is the P1 "missing domain separator" class.
- **Evidence:**

```rust
/// Compute the leaf hash: Blake2b-256 of canonical encoding.
pub fn leaf_hash(&self) -> Result<Hash, LeafError> {
    Ok(blake2b_256(&self.encode()?))
}
```

```rust
buf[..32].copy_from_slice(&chunk[0]);
buf[32..].copy_from_slice(&chunk[1]);
next.push(blake2b_256(&buf));
```

- **Suggested fix:** Add domain-separated hash helpers such as `hash_leaf(domain, encoded)` and `hash_node(left, right)`, then add tests that prove leaf preimages, internal nodes, and bundle aggregation cannot share the same hash domain. If the protocol intentionally remains untagged, add an explicit decision test that pins this behavior and links to the design rationale.
- **Verification:** `rg -n "hash_leaf|hash_node|domain|separator|blake2b_256\\(&self\\.encode|blake2b_256\\(&buf" omega-commitment/crates/omega-commitment-core/src omega-commitment/crates/omega-commitment-core/tests`

## F002 — Golden vectors skip the per-leaf layer

- **Severity:** P2
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-core/tests/golden_vectors.rs:53-136`
- **Issue:** The three golden-vector layers currently pin per-sub-tree roots and bundle roots, but not canonical encoded bytes or leaf hashes for each leaf type. Root-level goldens catch drift, but they localize failures poorly and do not give circuit authors or external implementations per-leaf vectors to test their encoders against.
- **Evidence:**

```rust
#[test]
fn golden_utxo_root() {
    let f: UtxoIn = serde_json::from_str(&read_fixture("utxo_set_small.json")).unwrap();
    let leaves: Vec<_> = f.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    let root = MerkleTree::build(leaves).root();
    // GOLDEN: regenerate via Step 1 if encoding semantics change.
    assert_eq!(
        hex::encode(root),
```

The same file continues with one root golden per sub-tree through governance, then only a UTXO witness JSON round-trip:

```rust
// Serialize the first leaf's witness; confirm it round-trips.
let w = InclusionWitness::build(&tree, leaves[0]).unwrap();
let json = serde_json::to_string(&w).unwrap();
let w2: InclusionWitness = serde_json::from_str(&json).unwrap();
assert_eq!(w, w2, "witness JSON round-trip diverged");
```

- **Suggested fix:** Add a `golden_leaf_vectors.rs` test or fixture file that pins, for each leaf type, the input JSON object, canonical encoded bytes as hex, and leaf hash as hex. Include at least one rich UTXO with assets and datum metadata, plus header, tx-index, policy, script, stake, and governance examples.
- **Verification:** `rg -n "golden_.*leaf|encoded_bytes|leaf_hash.*[0-9a-f]{64}" omega-commitment/crates/omega-commitment-core/tests omega-commitment/crates/omega-commitment-ingest/tests`

## F003 — Ledger edge-case fixture corpus is too narrow

- **Severity:** P2
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor.md:21-47`
- **Issue:** The fixture corpus covers small synthetic happy paths, but it does not cover several required edge cases: maximum-depth trees, non-UTF8 asset names, Byron addresses, pointer addresses, inline datums, reference scripts, or AlwaysAbstain DReps. Source unit tests cover empty and single-leaf trees, and CBOR helper tests cover some malformed lengths/trailing bytes, but those cases are not promoted into golden or end-to-end fixture vectors.
- **Evidence:**

```text
| # | tx_id | value | multi_assets | script_credential |
|---|---|---|---|---|
| 0 | `1111…11` | 1_000_000 | empty | null |
| 1 | `2222…22` | 5_000_000 | policy_a:{COIN→100} | null |
| 2 | `3333…33` | 25_000_000 | policy_a:{COIN→50,NFT→1}, policy_b:{TOKEN→999} | script_one (Plutus V2, 1024 B) |
| 3 | `4444…44` | 10_000_000 | empty | script_two (native multi-sig, 256 B) |
```

```text
- **utxo** sub-tree: 4 entries (one per UTXO). Multi-assets are surfaced in
  each UTXO's `assets` array using `asset_id = policy_id_28 || asset_name`;
  script credentials are consumed by the script sub-tree and are not surfaced
  in the UTXO JSON output.
- **token-policy** sub-tree: 2 entries (policy_a and policy_b).
```

The stake fixture likewise documents only zero/no-DRep and synthetic DRep bytes:

```text
All-zero pool means undelegated; all-zero DRep means no DRep delegation.
```

- **Suggested fix:** Add table-driven edge fixtures under `tests/fixtures/edge_cases`: empty and single-entry JSON inputs per sub-tree, a generated maximum-depth tree case, malformed CBOR cases for wrong major type / indefinite arrays / wrong arity / trailing bytes, an extended UTXO with raw non-UTF8 asset-name bytes, Byron and pointer-address bytes if the model accepts raw address hashes, datum/reference-script coverage, and a stake entry with the canonical AlwaysAbstain DRep bytes. If any item is intentionally out of model scope, add a rejection or normalization test plus fixture documentation.
- **Verification:** `rg -n "non-UTF|Byron|pointer address|inline datum|reference script|AlwaysAbstain|always-abstain|max.*depth|maximum-depth" omega-commitment/crates/*/tests omega-commitment/crates/*/src`

## F004 — Some unit tests discard the computed value

- **Severity:** P3
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-core/src/tree.rs:166-174`
- **Issue:** There are no `Ok(_)` test assertions in the searched surface, but three unit-test paths compute into `_` and therefore only check for no panic. The tree test is the clearest example: it says root-byte drift is handled elsewhere, then discards the root rather than asserting a pinned value or even nonzero value.
- **Evidence:**

```rust
#[test]
fn build_preserves_root_after_perf_change() {
    // Pin the structural shape for a known input. The integration
    // tests across all 3 sub-trees catch root-bytes drift; this test
    // catches structural changes (depth, leaf_count).
    let leaves: Vec<Hash> = (0..16u8).map(|i| blake2b_256(&[i])).collect();
    let t = MerkleTree::build(leaves);
    assert_eq!(t.depth(), 4);
    assert_eq!(t.leaf_count(), 16);
    let _ = t.root();
}
```

`rg` also finds `let _ = s.leaf_hash();` in `script_registry_leaf.rs` and `let _ = f.leaf_hash();` in `governance_state_leaf.rs`.
- **Suggested fix:** Replace the discarded values with assertions. For the tree test, pin `hex::encode(t.root())` for the known 16-leaf input or assert it equals a direct manual aggregation. For future language/kind tests, assert the returned hash length and a pinned hash or nonzero value.
- **Verification:** `rg -n "let\\s+_\\s*=.*(root|leaf_hash)" omega-commitment/crates/omega-commitment-core/src omega-commitment/crates/omega-commitment-core/tests`
