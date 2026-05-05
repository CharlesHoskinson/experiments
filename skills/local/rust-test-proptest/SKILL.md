---
name: rust-test-proptest
description: Property-based testing in Rust with proptest. Use when invoked by rust-test-orchestrator (Q5=yes) or when the user explicitly asks for property tests, round-trip tests, or shrinking-aware regression tests for Rust code. Best for codec round-trips, idempotence, monotonicity, and apply/revert pairs. Not for exhaustive schedule exploration (use shuttle-loom) or untyped-byte attack surfaces (use cargo-fuzz).
license: Apache-2.0
metadata:
  author: charles
  version: 0.1.0
  pack: rust-test
  invoked-by: rust-test-orchestrator
---

# rust-test-proptest

## When this skill applies

Q5 (random-input property) on the orchestrator's matrix. Concrete shapes:
- Codec round-trips: `decode(encode(x)) == x`
- State-machine apply/revert: `revert(apply(s, t)) == s`
- Monotonicity: `f(x) <= f(y)` whenever `x <= y`
- Commutativity: `f(a, b) == f(b, a)`

## Authoring loop (TDD-shaped)

1. Read the target (orchestrator did this in Phase 1).
2. Identify the property: write it as a single sentence ("for all valid `Leaf`, `parse(serialize(leaf)) == leaf`").
3. Write the test scaffold with `proptest!` and a custom `Strategy` if the standard ones don't cover the input space.
4. Run: `cargo nextest run -p <crate> <test_name>`. Expect failure if the property doesn't actually hold.
5. Implement / patch. Re-run.
6. Hand back to orchestrator: `proptest: wrote N tests in <path>; <pass|fail>`.

## Idioms in this codebase

**Custom Strategy for sub-tree leaves.** The seven sub-trees in `omega-commitment-core` have distinct leaf encodings. Write one `Strategy` per sub-tree (e.g., `arb_utxo_leaf()`, `arb_header_leaf()`) that yields valid `(sub_tree_id, leaf_index, payload)` triples. See `references/strategies-cookbook.md` for the canonical pattern.

**Round-trip CBOR ↔ struct ↔ leaf-hash.** For each sub-tree: `proptest!(|leaf in arb_utxo_leaf()| { let bytes = cbor::to_vec(&leaf).unwrap(); let back: UtxoLeaf = cbor::from_slice(&bytes).unwrap(); assert_eq!(leaf, back); })`. Catches encoder/decoder asymmetry — exactly the bug class that broke `cardano-cli --whole-utxo` (16-bit TxIx asymmetry).

**Apply/revert idempotence on the mock ledger.** For the future `omega-mock-ledger` apply path: `apply` then `revert` should restore byte-equal state.

**Monotonic nullifier set.** Inserting any nullifier into the ledger's nullifier set must never decrease its size and must never allow a duplicate.

## Anti-patterns

- Using proptest where shrinking can't help (untyped fuzz inputs) → defer to `rust-test-cargo-fuzz`.
- Using proptest for code whose bug only surfaces under a specific thread schedule → defer to `rust-test-shuttle-loom`.
- Using proptest for code whose bug only surfaces under a specific network failure → defer to `rust-test-failpoints` or `rust-test-turmoil`.
- Defining proptest strategies that generate invalid inputs (the test should test logic, not parser robustness — that's cargo-fuzz's job).

## Hand-back format

Single line for the orchestrator: `proptest: wrote N tests in <path>; <pass|fail>`.
