---
date: 2026-05-03
kind: design-doc
topic: Migrate Ω-Commitment hash from Blake2b-256 to Blake3
status: drafted
supersedes: bundle-layer dual-hash framing in 2026-05-01-omega-dual-hash-decision.md (Blake3 replaces Blake2b as the primary; SHA3 still drift-detection)
---

# Blake2b → Blake3 migration design

## Why

The 2026-05-03 spike against `Plonky3/Plonky3` HEAD confirmed Plonky3 ships AIRs for Blake-3, Keccak, and Poseidon2 out of the box. It does not ship Blake2b. The 2026-05-01 commitment design fixed Blake2b-256 as the primary leaf and node hash; that choice now imposes a load-bearing T6 engineering item (write `p3-blake2b-air`, ~1 week port from `blake3-air/`).

Switching to Blake3 eliminates the engineering item, gives us a measurably faster hash on every modern platform, and preserves every other property of the construction (domain separation, fixed-arity tree, deterministic ordering, padded-to-pow2). The cost is regenerating the v0.x golden vectors and bumping the domain tag from `omega:v1:*` to `omega:v2:*` so post-migration preimages are not collision-equivalent to pre-migration ones.

## Decisions

### Hash family

- **Primary**: Blake3-256 replaces Blake2b-256 across all leaf and internal-node hashes.
- **Drift-detection**: SHA3-256 stays in place at the bundle layer, computed over the same Blake3 leaf hashes. Reframing from `audit/findings/A1-cryptographic-correctness.md` (F004) carries over verbatim — the SHA3 root catches divergence in the aggregation step, not a Blake3 break.
- **In-circuit (when proving membership)**: still Blake3 for now. Plonky3's `p3-blake3-air` handles Blake3 inside a STARK natively. A future v2 of the verifier circuit may swap to Poseidon2 for prover speed, with a Blake3 → Poseidon2 wrapper at the boundary; that is a v2.0 question, not v1.0.

### Domain tags

- `omega:v1:leaf` / `omega:v1:node` → **`omega:v2:leaf` / `omega:v2:node`**.
- Bumping the version makes the on-the-wire preimage incompatible with v0.9.x outputs, which is the right behaviour: a verifier that knows the v2 tags will refuse a v1 inclusion proof and vice versa. Mixing tags across a migration would be a foot-gun.
- The empty-leaf sentinel (`EMPTY_INDEX_SENTINEL = u64::MAX`) and the leaf preimage layout (`tag || sub_tree_id || canonical_index_be || payload_len_be || payload`) are unchanged.

### Bundle root

```
bundle_blake3 = blake3(root_1 || root_2 || ... || root_7)   (primary)
bundle_sha3   = sha3  (root_1 || root_2 || ... || root_7)   (drift-detection)
Ω-Commitment  = (bundle_blake3, bundle_sha3)                 (still 64 bytes)
```

Variable name and JSON field rename: `blake2b_bundle_root` → `blake3_bundle_root`, `bundle_root_blake2b` → `bundle_root_blake3`. The 64-byte tuple shape and on-disk JSON schema otherwise unchanged.

### Workspace version

v0.9.1 (Blake2b, v1 tags) becomes a frozen tag, callable as `omega-commitment-core 0.9.x`. The Blake3 + v2 tags branch is **v0.10.0-rc1**, then **v1.0.0** at the real-mainnet golden-vector landing (Task 14 of the v1.0 plan).

### Test goldens

All three layers regenerate:

- `core/tests/golden_vectors.rs` — per-sub-tree roots for the seven synthetic fixtures.
- `core/tests/golden_per_leaf.rs` — canonical bytes + v2 leaf hashes for the example-per-sub-tree plus the three pinned edge cases.
- `bundle/tests/golden_bundle.rs` — bundle root tuple under both Blake3 and SHA3.
- `ingest/tests/golden_ingest.rs` — ingestion-layer roots and the hybrid bundle root.

Regeneration is mechanical: run the existing tests under `cargo test -- --ignored regenerate-goldens` (or whichever switch the v0.9.1 codebase uses; if there is no switch, the test fixtures themselves carry the new vectors and `cargo test` simply asserts equality after the hash is swapped).

### Cargo dep changes

```toml
# Before
blake2 = "0.10"

# After
blake3 = { version = "1", default-features = false }
```

Workspace `Cargo.toml` becomes the single edit; every per-crate manifest inherits via `workspace = true`. The `default-features = false` strip removes the `rayon` parallel-hash feature for now (the AIR cost is per-block, not per-leaf-set, and CI runs single-threaded anyway).

### Code shape

`omega-commitment-core/src/hash.rs`:

```rust
// Before
use blake2::{Blake2b, Digest};
type Blake2b256 = Blake2b<digest::consts::U32>;

pub fn blake2b_256(input: &[u8]) -> [u8; 32] {
    let mut h = Blake2b256::new();
    h.update(input);
    h.finalize().into()
}

// After
pub fn blake3_256(input: &[u8]) -> [u8; 32] {
    *blake3::hash(input).as_bytes()
}
```

`omega-commitment-core/src/tree.rs` — `leaf_hash_v1` and `node_hash_v1` rename to `leaf_hash_v2` and `node_hash_v2`, with the domain tags bumped:

```rust
const DOMAIN_LEAF: &[u8] = b"omega:v2:leaf";
const DOMAIN_NODE: &[u8] = b"omega:v2:node";

pub fn leaf_hash_v2(sub_tree_id: u8, canonical_index: u64, payload: &[u8]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(DOMAIN_LEAF.len() + 1 + 8 + 8 + payload.len());
    buf.extend_from_slice(DOMAIN_LEAF);
    buf.push(sub_tree_id);
    buf.extend_from_slice(&canonical_index.to_be_bytes());
    buf.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    buf.extend_from_slice(payload);
    blake3_256(&buf)
}

pub fn node_hash_v2(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut buf = [0u8; DOMAIN_NODE.len() + 64];
    buf[..DOMAIN_NODE.len()].copy_from_slice(DOMAIN_NODE);
    buf[DOMAIN_NODE.len()..DOMAIN_NODE.len() + 32].copy_from_slice(left);
    buf[DOMAIN_NODE.len() + 32..].copy_from_slice(right);
    blake3_256(&buf)
}
```

The padding-leaf sentinel:

```rust
pub const EMPTY_INDEX_SENTINEL: u64 = u64::MAX;
let pad = leaf_hash_v2(sub_tree_id, EMPTY_INDEX_SENTINEL, &[]);
```

### What stays unchanged

- The seven sub-trees and their leaf encodings (UTXO 81+, header 80, tx-idx 76, token-policy 52, script 41, stake 93, governance 57).
- Sort-then-pad-to-pow2 binary tree topology.
- Duplicate-payload rejection at the tree builder.
- Trailing-byte rejection in CBOR ingest.
- The whole audit-finding closure trail. A1/F001-F005 (domain separation, sentinel padding, dual-hash framing) carry over identically — the same logic, with Blake3 instead of Blake2b in the H slot.
- The four-layer architecture, the no-backdoor stance, the two-stream v1.0 pipeline, the consensus stack, the mirror partnerchain spec.

### What changes downstream

- **T6 verifier circuit**: Plonky3's `p3-blake3-air` is now the in-circuit hash gadget. C1 and C3 (leaf hash and node hash) compile directly without a custom AIR.
- **Wiki**: every page mentioning Blake2b updates to Blake3 with a one-line migration note where appropriate.
- **README + ARCHITECTURE**: every reference to `blake2b_*`, `H_blake2b`, "Blake2b-256", "Blake2b break-hedge" updates to Blake3.
- **omega-commitment workspace tests**: 282 tests still run; a fraction (the goldens) need their pinned bytes regenerated. Functional tests pass unchanged.
- **omega-commitment README**: bumps version table and notes the migration.

### Migration steps (in order)

1. Bump workspace dep: `blake2 = "0.10"` → `blake3 = "1", default-features = false`.
2. Replace `blake2b_256` with `blake3_256` in `core/src/hash.rs`. Keep the `sha3_256` function as-is.
3. Rename `leaf_hash_v1` / `node_hash_v1` → `leaf_hash_v2` / `node_hash_v2`. Bump `DOMAIN_LEAF` / `DOMAIN_NODE` constants.
4. `cargo build --workspace` — fix every call site that referenced the old name. Mostly mechanical.
5. `cargo test --workspace` — every functional test passes; goldens fail.
6. Regenerate goldens: edit each `tests/golden_*.rs` file with the new hex. Run with the new vectors pinned. (One reviewer-pair-eyes pass to confirm the new bytes are derived from the new code, not copy-pasted from a stale run.)
7. Bump per-crate `version = "0.9.1"` → `version = "0.10.0-rc1"`.
8. Rebuild + retest. Tag.
9. Update README, ARCHITECTURE, wiki pages, audit findings cross-references in one final docs sweep.
10. Append a wiki log entry recording the migration with the new bundle-root tuple from a freshly-recomputed v0.10.0-rc1 synthetic fixture.

## What this does not change

- The 2026-05-03 audit closure trail. A1/F001-F005 are about domain separation, padding-leaf sentinel, and dual-hash framing — none of which depend on the choice of hash function. The findings are closed against a tagged-and-padded tree; that property survives the migration.
- The deferred P1 (`A2/F001`, real mainnet `GetUTxOWhole` decoder). Independent.
- The PQ-VRF gap (`RESEARCH-QUESTIONS.md` Q1). Independent.
- The dual-hash framing as drift-detection rather than break-hedge. Same SHA3 layer; same caveat; same v2.0 follow-up for a truly-independent SHA3 tree.

## Acceptance

- `cargo test --workspace` green on the new code with regenerated goldens.
- `omega-bundle assemble` against a fresh synthetic fixture set produces a `(blake3_bundle_root, sha3_bundle_root)` tuple that matches the new pinned golden.
- `Plonky3/Plonky3` HEAD's `prove_prime_field_31 --objective blake3-permutations` runs against an instance derived from one of the new sub-tree roots, demonstrating that the in-circuit hash gadget is a drop-in.
- README, ARCHITECTURE, and the testnet e2e wiki page contain no stale Blake2b references.
- `cardano-wiki/wiki/log.md` carries a migration entry dated 2026-05-03 with the new bundle root and the version bump.
