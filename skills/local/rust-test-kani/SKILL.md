---
name: rust-test-kani
description: Bounded model checking for Rust with Kani. Use when invoked by rust-test-orchestrator (Q4=yes) or when the user asks for formal verification of a small bounded function (collision resistance, monotonicity, no-panic guarantees). Bundles a script with omega-commitment-pinned bounds. Not for code calling Plonky3 (state explodes) or distributed protocols (use stateright).
license: Apache-2.0
metadata:
  author: charles
  version: 0.1.0
  pack: rust-test
  invoked-by: rust-test-orchestrator
---

# rust-test-kani

## When this skill applies

Q4 (bounded invariant) on the orchestrator's matrix. Concrete shapes:
- Pure functions with input space ≤ a few hundred bytes
- Collision-resistance claims over fixed-width inputs
- Monotonicity / commutativity / associativity over small enums
- No-panic guarantees on bounded loops

## Authoring loop

1. Identify the bounded property as a single statement ("for all `(sub_tree_id, idx, payload)` with `payload.len() ≤ 4096`, `leaf_hash_v2` returns `Ok`").
2. Write a Kani harness with `#[kani::proof]` and `kani::any()` for inputs.
3. Run `bash scripts/kani-bound.sh <crate>`.
4. If Kani returns SUCCESSFUL, the property holds within the bound. If FAILED, the counterexample is in the output.
5. Hand back to orchestrator: `kani: wrote N harnesses in <path>; <pass|fail>`.

## Idioms in this codebase

**Bound on payload size.** v0.1 of omega-commitment caps leaf preimages at 64 bytes (one Blake3 compression block). Kani harnesses use `payload.len() ≤ 64` for v0.1 surfaces and `≤ 4096` for v0.2 surfaces. See `references/bound-tuning.md`.

**Collision-resistance over `(sub_tree_id, idx, payload)` triples.** For `leaf_hash_v2`:

```rust
#[kani::proof]
fn leaf_hash_distinct_inputs_distinct_outputs() {
    let id1: u8 = kani::any(); kani::assume(id1 < 7);
    let id2: u8 = kani::any(); kani::assume(id2 < 7);
    let idx1: u32 = kani::any(); kani::assume(idx1 < 1024);
    let idx2: u32 = kani::any(); kani::assume(idx2 < 1024);
    let payload1: [u8; 32] = kani::any();
    let payload2: [u8; 32] = kani::any();

    kani::assume((id1, idx1, payload1) != (id2, idx2, payload2));

    let h1 = leaf_hash_v2(id1, idx1, &payload1).unwrap();
    let h2 = leaf_hash_v2(id2, idx2, &payload2).unwrap();
    assert!(h1 != h2);
}
```

Kani can't prove cryptographic collision-resistance in the absolute sense, but it can prove that the *encoding* never produces equal preimages from distinct inputs (the leaf-as-internal-node second-preimage swap defence).

**Monotonic nullifier set.** Insert any nullifier; assert the set's size never decreases.

## Anti-patterns

- Trying to verify code that calls Plonky3 → defer to `rust-test-proptest`. Plonky3's circuit construction is too large for Kani's bounded model checker.
- Trying to verify `unsafe` code without `#[kani::should_panic]` annotations.
- Picking unbounded loops without a `kani::assume(loop_count < N)` cap.
- Using Kani as a substitute for proptest on functions whose input space is genuinely unbounded.

## Hand-back format

Single line for the orchestrator: `kani: wrote N harnesses in <path>; <pass|fail>`.

## Script in this skill

- `scripts/kani-bound.sh <crate>` — runs `cargo kani` with omega-pinned bounds (`--default-unwind 4 --solver minisat`).
