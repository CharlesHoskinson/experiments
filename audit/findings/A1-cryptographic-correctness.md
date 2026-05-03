---
agent: A1
lane: code
title: cryptographic-correctness
files-reviewed: [omega-commitment/crates/omega-commitment-core/src/lib.rs, omega-commitment/crates/omega-commitment-core/src/hash.rs, omega-commitment/crates/omega-commitment-core/src/tree.rs, omega-commitment/crates/omega-commitment-core/src/witness.rs, omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs, omega-commitment/crates/omega-commitment-core/src/header_leaf.rs, omega-commitment/crates/omega-commitment-core/src/tx_index_leaf.rs, omega-commitment/crates/omega-commitment-core/src/token_policy_leaf.rs, omega-commitment/crates/omega-commitment-core/src/script_registry_leaf.rs, omega-commitment/crates/omega-commitment-core/src/stake_state_leaf.rs, omega-commitment/crates/omega-commitment-core/src/governance_state_leaf.rs, omega-commitment/crates/omega-commitment-bundle/src/lib.rs, omega-commitment/crates/omega-commitment-bundle/src/bundle.rs, omega-commitment/crates/omega-commitment-bundle/src/recompute.rs, omega-commitment/crates/omega-commitment-bundle/src/sub_tree_id.rs, omega-commitment/crates/omega-commitment-cli/src/main.rs, omega-commitment/crates/omega-commitment-core/tests/golden_vectors.rs, omega-commitment/crates/omega-commitment-bundle/tests/golden_bundle.rs, omega-commitment/crates/omega-commitment-bundle/tests/end_to_end_integration.rs]
findings-count: { p0: 0, p1: 4, p2: 1, p3: 0 }
---

# Summary

The current Merkle commitments hash typed payload encodings directly, then sort and pad those hashes with an all-zero placeholder. Sub-tree identity, canonical item index, leaf-vs-node role, and padding role are not cryptographically committed in the leaf or node preimages. The bundle-level SHA3 track is useful for catching implementation drift in aggregation, but it does not defend against a Blake2b break at the payload-to-leaf layer because SHA3 consumes the same Blake2b leaf hashes.

# Findings

## F001 — Leaf hashes do not bind sub_tree_id or canonical leaf_index

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-bundle/src/recompute.rs:87-134`, `omega-commitment/crates/omega-commitment-core/src/*_leaf.rs`
- **Issue:** Each sub-tree root builder maps parsed items directly to `leaf_hash()`, and each `leaf_hash()` hashes only that item encoding. `SubTreeId` only selects the parser, and `leaf_index` is assigned later from the sorted, padded hash array, so neither `(sub_tree_id, leaf_index)` is in the leaf preimage.
- **Evidence:**

```rust
fn build_leaves(sub_tree: SubTreeId, raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    match sub_tree {
        SubTreeId::Utxo => {
            let parsed: UtxoInput = serde_json::from_str(raw)?;
            let leaves: Vec<Hash> = parsed
                .utxos
                .iter()
                .map(|u| u.leaf_hash())
                .collect::<Result<Vec<_>, _>>()?;
```

```rust
pub fn leaf_hash(&self) -> Hash {
    blake2b_256(&self.encode())
}
```

```rust
let leaf_index = leaves.iter().position(|h| h == &leaf)? as u32;
```

- **Suggested fix:** Make the leaf hash API take the sub-tree id and canonical item index, and hash a versioned domain-separated preimage such as `omega:v1:leaf || sub_tree_id_u8 || canonical_index_u64_be || payload_len_u32_be || payload`. Define whether `canonical_index` is the semantic sorted-input index or the Merkle tree position; if it is semantic, rename the witness field to avoid conflating it with the post-sort tree index.
- **Verification:** From repo root, run `rg -n "map\\(\\|.*leaf_hash\\(|pub fn leaf_hash|leaf_index = leaves\\.iter\\(\\)\\.position" omega-commitment/crates/omega-commitment-bundle/src/recompute.rs omega-commitment/crates/omega-commitment-core/src omega-commitment/crates/omega-commitment-cli/src/main.rs` and confirm the new code hashes sub-tree id and index before tree construction.

## F002 — Leaf and internal-node hashes share the same untagged domain

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-core/src/tree.rs:28-44`, `omega-commitment/crates/omega-commitment-core/src/*_leaf.rs`
- **Issue:** Leaf hashes and internal nodes are both raw Blake2b-256 calls, with no distinct domain tags. The project invariant being checked here requires distinct leaf and internal-node domains; without that, the construction relies on current payload lengths and Blake2b collision resistance rather than an explicit Merkle-tree separation boundary.
- **Evidence:**

```rust
pub fn leaf_hash(&self) -> Result<Hash, LeafError> {
    Ok(blake2b_256(&self.encode()?))
}
```

```rust
for chunk in prev.chunks(2) {
    let mut buf = [0u8; 64];
    buf[..32].copy_from_slice(&chunk[0]);
    buf[32..].copy_from_slice(&chunk[1]);
    next.push(blake2b_256(&buf));
}
```

- **Suggested fix:** Introduce tagged helpers and use them everywhere, including witness verification and SHA3 recomputation: `leaf_hash = H("omega:v1:leaf" || ...)`, `node_hash = H("omega:v1:node" || left || right)`. Add a regression test that no leaf encoder calls `blake2b_256(&self.encode())` directly and no internal node hashes bare `left || right`.
- **Verification:** `rg -n "blake2b_256\\(&self\\.encode|blake2b_256\\(&buf|sha3_256\\(&buf" omega-commitment/crates/omega-commitment-core/src omega-commitment/crates/omega-commitment-bundle/src` should show only tagged helper calls after the fix.

## F003 — Zero padding can be proven as membership unless verifiers add external item_count checks

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-core/src/tree.rs:14-34`, `omega-commitment/crates/omega-commitment-core/src/witness.rs:63-90`
- **Issue:** Padding uses the literal all-zero hash as a leaf, and `InclusionWitness::verify` only checks that `leaf_index < 2^depth`. A witness for a padded position with `leaf = ZERO_HASH` verifies against the root, so the witness layer cannot distinguish real membership from an empty padded slot; empty input also has root `ZERO_HASH`.
- **Evidence:**

```rust
pub const ZERO_HASH: Hash = [0u8; 32];
```

```rust
let target = input.len().max(1).next_power_of_two();
while input.len() < target {
    input.push(ZERO_HASH);
}
```

```rust
let max_index = if depth == 0 { 1u64 } else { 1u64 << depth };
if (self.leaf_index as u64) >= max_index {
    return false;
}
let mut current = self.leaf;
let mut idx = self.leaf_index as usize;
for sib in &self.siblings {
    let mut buf = [0u8; 64];
    if idx & 1 == 0 {
        buf[..32].copy_from_slice(&current);
        buf[32..].copy_from_slice(sib);
    } else {
        buf[..32].copy_from_slice(sib);
        buf[32..].copy_from_slice(&current);
    }
    current = blake2b_256(&buf);
    idx /= 2;
}
current == root
```

- **Suggested fix:** Use a domain-separated empty-leaf value, for example `empty_hash(sub_tree_id, tree_index) = H("omega:v1:empty" || sub_tree_id || tree_index)`, and require membership verification to know and enforce `leaf_index < item_count`. Also make the empty-tree root a versioned domain-separated root, not `[0u8; 32]`.
- **Verification:** `rg -n "ZERO_HASH|input\\.push\\(ZERO_HASH\\)|leaf_index|item_count|current == root" omega-commitment/crates/omega-commitment-core/src omega-commitment/crates/omega-commitment-bundle/src omega-commitment/crates/omega-commitment-cli/src/main.rs` should show a verifier-visible `item_count` bound and no raw zero padding leaf.

## F004 — Bundle SHA3 track is cosmetic against a Blake2b leaf-hash break

- **Severity:** P1
- **Confidence:** high
- **Location:** `omega-commitment/crates/omega-commitment-bundle/src/recompute.rs:11-13`, `omega-commitment/crates/omega-commitment-bundle/src/recompute.rs:71-77`, `omega-commitment/crates/omega-commitment-bundle/src/recompute.rs:138-156`, `omega-commitment/crates/omega-commitment-bundle/src/bundle.rs:106-120`
- **Issue:** The SHA3 sub-tree root is computed over the same Blake2b leaf hashes, not over independently SHA3-hashed payload preimages. If Blake2b admits a chosen-prefix collision or second preimage at the leaf layer, replacing a payload with another payload that has the same Blake2b leaf hash leaves both the Blake2b root and the SHA3 root unchanged.
- **Evidence:**

```rust
//! Per the dual-hash decision (2026-05-01): per-leaf hashing stays
//! Blake2b-only. The SHA3 root is a SHA3 Merkle aggregation over the
//! SAME Blake2b leaf hashes. Only the aggregation step runs in SHA3.
```

```rust
let (leaves, item_count) = build_leaves(sub_tree, raw)?;
let tree = MerkleTree::build(leaves.clone());
let blake2b_root = tree.root();
let sha3_root = sha3_root_of(leaves);
```

```rust
for chunk in current.chunks(2) {
    let mut buf = [0u8; 64];
    buf[..32].copy_from_slice(&chunk[0]);
    buf[32..].copy_from_slice(&chunk[1]);
    next.push(sha3_256(&buf));
}
```

- **Suggested fix:** Build two independent leaf sets from the same canonical payloads: `blake_leaf = Blake2b(tagged_leaf_preimage)` and `sha3_leaf = SHA3(tagged_leaf_preimage)`. Then build Blake2b and SHA3 trees over their respective leaf sets, with tagged internal nodes, and aggregate the bundle roots with sub-tree id, counts, and both roots in a single versioned bundle preimage.
- **Verification:** `rg -n "per-leaf hashing stays|SAME Blake2b leaf hashes|sha3_root_of\\(leaves\\)|sha3_256\\(&buf\\)|extend_from_slice\\(&r\\.sha3_root" omega-commitment/crates/omega-commitment-bundle/src` should be replaced by code that computes SHA3 leaves from canonical payload bytes, not from Blake2b leaves.

## F005 — Duplicate semantic keys are documented as data errors but accepted by root builders

- **Severity:** P2
- **Confidence:** medium
- **Location:** `omega-commitment/crates/omega-commitment-core/src/tx_index_leaf.rs:43-52`, `omega-commitment/crates/omega-commitment-bundle/src/recompute.rs:87-134`, `omega-commitment/crates/omega-commitment-cli/src/main.rs:149-199`
- **Issue:** Several leaf modules define duplicate-key validators and describe duplicates as data errors, but both the bundle recomputation path and CLI commit path build roots without calling those validators. A malformed snapshot can therefore commit conflicting leaves for the same semantic key, leaving claim verifiers to resolve ambiguity outside the commitment.
- **Evidence:**

```rust
/// Cardano transaction hashes are deterministic functions of the tx
/// body and should be unique across the whole chain. Duplicate input
/// is a data error (e.g., a snapshot with overlapping epoch ranges).
/// This is an OPTIONAL sanity helper; commitment generation does NOT
/// require uniqueness.
pub fn validate_tx_uniqueness(entries: &[TxIndexEntry]) -> Option<usize> {
```

```rust
SubTreeId::TxIndex => {
    let parsed: TxIndexInput = serde_json::from_str(raw)?;
    let leaves: Vec<Hash> = parsed.entries.iter().map(|e| e.leaf_hash()).collect();
    let n = parsed.entries.len();
    Ok((leaves, n))
}
```

```rust
fn build_tx_index_leaves(raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    let parsed: TxIndexInput = serde_json::from_str(raw)?;
    let leaves: Vec<Hash> = parsed.entries.iter().map(|e| e.leaf_hash()).collect();
    let n = parsed.entries.len();
    Ok((leaves, n))
}
```

- **Suggested fix:** Treat duplicate semantic keys as root-construction errors for sub-trees whose modules already define uniqueness helpers: tx-index, token-policy, script-registry, stake-state, and governance. For UTXO assets, reject or coalesce duplicate `asset_id` values before encoding so one semantic multi-asset bundle has one canonical leaf preimage.
- **Verification:** `rg -n "validate_.*uniqueness|validate_.*unique|build_.*leaves|SubTreeId::" omega-commitment/crates/omega-commitment-core/src omega-commitment/crates/omega-commitment-bundle/src/recompute.rs omega-commitment/crates/omega-commitment-cli/src/main.rs` should show validators invoked before `.map(|...| ...leaf_hash())`, with tests that duplicate semantic keys fail commitment generation.
