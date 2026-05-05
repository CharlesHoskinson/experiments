# Bound tuning for Kani harnesses in omega-commitment

Kani's tractability depends on the bound on input sizes and loop counts. These are the project-pinned bounds for omega-commitment.

## Default unwind

`--default-unwind 4` — most loops in commitment code iterate over fixed small structures (7 sub-trees, 8 verifier constraints). Bound 4 catches off-by-one errors without exploding state.

## Payload size bounds

| Surface | Bound | Rationale |
|---|---|---|
| v0.1 leaf preimages | `≤ 64 bytes` | One Blake3 compression block; matches v0.1 chunk-free design |
| v0.2 variable-length leaves | `≤ 4096 bytes` | One typical UTxO with a small asset bundle |
| Claim transaction body | `≤ 16384 bytes` | Max plausible claim with multi-asset proofs |
| Plonky3 public inputs | `≤ 256 bytes` | Bounded by genesis params |

## Index bounds

| Surface | Bound | Rationale |
|---|---|---|
| `leaf_index` | `< 2^20` | ~1M leaves per sub-tree; matches v1.0 mainnet sizes |
| `sub_tree_id` | `< 7` | 7 sub-trees, fixed |
| `EMPTY_INDEX_SENTINEL` | `== u64::MAX` | Reserved padding sentinel |

## Solver choice

`--solver minisat` is the default for omega harnesses. Cadical is faster on some queries but less stable; minisat is the safe baseline.

## When to relax the bound

If a harness times out, first check whether the bound is unnecessarily large. Most omega properties hold under the bounds above; if you need a larger bound, the property may not be appropriate for Kani — defer to proptest.

## When to skip Kani entirely

- Property requires reasoning over Blake3's collision resistance (cryptographic, not structural)
- Property requires Plonky3 circuit invariants (state explodes)
- Property requires `unsafe` reasoning over raw pointers (Kani's `unsafe` support is incomplete)

In these cases, the orchestrator should mark Q4 as no in the matrix.
