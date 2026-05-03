---
name: plonky3-friendly-rust
description: Patterns for writing Rust code that compiles cleanly to Plonky3 STARK constraints. Use when designing or implementing primitives that will execute inside a STARK circuit (Merkle trees, hash functions, signature verification, ledger transitions). Covers fixed-arity tree traversals, hash-only operations (no curve ops), deterministic serialisation, domain separation, witness/public-input split, FFI-free pure Rust, plonky3-native hash AIRs (Poseidon2, Keccak, Blake3), and the gap between "works in Rust" and "constrains cheaply in a STARK." Invoke when working on omega-commitment, plonky3 circuits, JOLT/RISC0 zkVM code, or any cryptographic primitive that will eventually be proven inside a STARK.
license: Apache-2.0
metadata:
  author: charles hoskinson
  version: "0.1.0"
  domain: cryptography
  triggers: plonky3, STARK, zk circuit, Merkle proof, omega-commitment, hash AIR, proof system, recursion proof, zkVM, JOLT, RISC0
  related-skills: rust-engineer, rust-skills, ab-m04-zero-cost, ab-m15-anti-pattern
---

# Plonky3-friendly Rust

Most Rust patterns that work fine for ordinary code break or become expensive when the same code must execute inside a STARK circuit. This skill catalogues the patterns that work and the anti-patterns that quietly explode the constraint count.

## Mental model

A plonky3 circuit is a polynomial constraint system over a small prime field. Every operation your Rust code performs at proving time becomes a row in a trace and a constraint that must be satisfied. Hash invocations are the dominant cost. Curve operations have no native AIR and must be emulated, which is catastrophic. Branching that depends on private witness data is more expensive than a straight-line computation.

The key question for every primitive: "what does the constraint count look like when this runs inside a STARK?" If the answer is "I do not know," do not write it; find out first.

## Hash function selection

Plonky3 ships native AIRs for **Poseidon2**, **Keccak-f**, and **Blake3**. Pick from this set whenever the hash will execute inside the circuit. Approximate cost ratios on commodity hardware:

| Hash | Native AIR | In-circuit cost (relative to Poseidon2) |
|---|---|---|
| Poseidon2 | yes | 1x |
| Blake3 | yes | ~24x |
| Keccak-f / SHA3 | yes | ~30x |
| Blake2b | **no** | requires custom AIR or generic field-emulation; ~50–100x |
| SHA-256 | no native AIR | similar to Blake2b without optimisation |

**Practical rule:** every hash that runs inside the verifier circuit should be Poseidon2 unless there is an explicit interoperability reason to use one of the standard hashes. Standard hashes (Blake2b, SHA3) are fine **outside** the circuit (snapshot construction, off-chain commitment building). The dual-hash discipline is: hash with the standard at the snapshot/interop boundary, hash with Poseidon2 inside the circuit, with a single conversion at the boundary.

## Merkle tree patterns

**Use binary, fixed-arity trees, zero-padded to the next power of two.** Variable-arity trees (Patricia trie, sparse Merkle tree with variable branching) require the verifier to handle multiple node shapes inside the circuit, which expands the constraint count by a factor of (number of supported shapes). Fixed arity collapses to a tight loop.

**Bind every leaf to its position before hashing.** The standard pattern:

```rust
fn leaf_hash(sub_tree_id: u8, leaf_index: u64, payload: &[u8]) -> [u8; 32] {
    let mut h = Blake2b256::new();   // or Poseidon2 if in-circuit
    h.update(&[0x00]);                // domain tag: 0x00 = leaf, 0x01 = internal node
    h.update(&[sub_tree_id]);
    h.update(&leaf_index.to_be_bytes());
    h.update(payload);
    h.finalize().into()
}
```

The domain tag is non-negotiable. Without it, an attacker can present an internal node's preimage (two child digests, 64 bytes) as a "leaf" and the Merkle membership check accepts it. This bug has bitten Uniswap, OpenZeppelin, and several airdrop systems publicly. Adding the byte costs nothing inside the circuit; omitting it is a soundness failure.

**Internal nodes get a different tag:**

```rust
fn internal_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = Blake2b256::new();
    h.update(&[0x01]);                // domain tag: internal
    h.update(left);
    h.update(right);
    h.finalize().into()
}
```

**Lexicographic leaf ordering for determinism.** Two impls must produce the same root from the same logical leaf set regardless of insertion order. Sort by the natural identifier (tx_id, slot, policy_id) at construction time.

**Zero-padding to the next power of two.** Pad with the all-zero leaf hash, NOT with the empty-string hash. The all-zero leaf hash is a known constant the verifier can baked-in; the empty-string hash is a value you have to compute. Document the padding convention in the spec.

## Serialisation patterns

**Deterministic encoding only.** No reliance on `HashMap` iteration order. No `serde_json::Value` round-trips that re-order map keys. No floating-point. No timestamps that include sub-second precision unless the spec pins them.

**Fixed-width integers, big-endian.** `u64::to_be_bytes()` everywhere. Little-endian also works, but pick one and stick to it; the spec must say which. Big-endian matches the IETF-tradition for cryptographic protocols and is the default in most Rust crypto crates.

**Length prefixes are explicit, not derived.** Never rely on the receiver computing the length from "everything after this point"; always prefix variable-length fields with their byte length. This makes the parser O(1) instead of O(N) and makes the encoding self-describing.

**Reject trailing bytes.** Every CBOR or custom-format decoder must call an explicit `expect_end()` after parsing the expected structure. Silently ignored trailing bytes are a class-A bug surface; the cardano-cli `--whole-utxo` failure that bit this project was a related pattern.

## Domain separation across primitives

Every cryptographic primitive that consumes user-controlled data binds a domain tag into the input. The taxonomy:

| Primitive | Domain tag input |
|---|---|
| Leaf hash | `(sub_tree_id, leaf_index)` |
| Internal node hash | constant byte distinct from leaf tag |
| Sub-tree root commitment | `sub_tree_id` |
| Bundle root | constant byte distinct from per-tree tags |
| Signature verification | `(chain_id, message_purpose)` |
| Random oracle queries | challenge label, request count |

The domain tag is one byte and it is not optional. The constraint cost inside the circuit is one extra byte of input to the hash; that is a rounding error.

## Witness vs public input

Inside a circuit, every value is either **public input** (visible to the verifier and committed in the proof envelope) or **witness** (private to the prover, only constrained, never revealed). The split matters because:

- Anything in the public input must be re-derivable by the verifier from chain state. Including a leaf preimage as public input means the verifier needs to know the leaf preimage.
- Anything in the witness is not authenticated by the proof envelope alone; the proof must constrain it to match a hash-committed value. Forgetting to constrain a witness is the classic "underconstrained circuit" bug.

**Pattern for claim transactions:**

```text
public_input:  (sub_tree_id, leaf_index, bundle_root, omega_recipient, chain_id)
witness:       (leaf_preimage, merkle_path[24], pq_signature)
constraints:   1. leaf_hash(sub_tree_id, leaf_index, leaf_preimage) is computed
               2. merkle_path walk reproduces sub_tree_root
               3. bundle_hash(7 sub_tree_roots) matches public bundle_root
               4. pq_verify(witness.sig, public_input, leaf_preimage.pubkey) holds
               5. nullifier (sub_tree_id, leaf_index) is fresh on-chain
```

Every line of the constraints column must be implemented inside the circuit. Skipping any one of them is a soundness failure.

## Code patterns to avoid

**No curve operations inside the circuit.** No `k256`, `p256`, `bls12_381`, `curve25519_dalek`, no Pedersen commitments, no ECDSA, no Schnorr. These have no native AIR and emulating them costs ~10,000-100,000x a hash. If you find yourself needing one, either move it outside the circuit or accept the cost with eyes open.

**No `String` allocations in the hot path.** Inside the prover trace, `String::push_str` and `format!` are heap allocations that fragment the prover's working memory. Use `&[u8]` or fixed-size arrays.

**No `Box<dyn Trait>` in primitive code.** The circuit cannot reason about virtual dispatch. Use generics or const-generics for any abstraction over hash functions or curves.

**No `unwrap()` or `panic!` in primitive code.** A panic during proof generation produces an undefined state, not a recoverable error. Return `Result` with a typed error and let the caller decide.

**No `HashMap` iteration in deterministic code paths.** `HashMap` iteration order is not stable across allocations; if your sub-tree root depends on iteration order, your sub-tree root is non-deterministic. Use `BTreeMap` or sort an explicit `Vec<(K, V)>`.

**No `chrono::Local::now()` or any wall-clock-dependent value.** Slot-time and block-height come from chain state, not from the prover's clock.

## Code patterns to prefer

**Pure functions with explicit inputs and outputs.** Every primitive is a function from `(input bytes) → output bytes`. No global state, no mutable singletons, no `lazy_static`.

**Const-generic array sizes.** `[u8; 32]` everywhere a hash is expected. The compiler enforces the size; the circuit code does not need to runtime-check it.

**Trait abstraction at the *outside* of the primitive, not inside.** Define `trait Hasher { type Output; fn hash(&self, &[u8]) -> Self::Output; }` and let the primitive code be generic over `H: Hasher`, with a concrete `Blake2bHasher` impl for the off-chain path and a concrete `Poseidon2Hasher` impl for the in-circuit path. Same primitive code, two execution paths.

**`#![forbid(unsafe_code)]` at the crate root.** Cryptographic primitives have no business using unsafe. If you find a real performance need that requires unsafe, isolate it in a separately-audited crate.

**Property-based tests for algebraic invariants.** Use `proptest` for round-trip properties (encode-decode), determinism properties (Merkle root insensitivity to input permutation), and idempotence properties (claim verifier is a no-op on already-consumed nullifiers). Use `propproof` to drive the same harnesses through Kani for symbolic verification of the highest-stakes properties.

**Domain-typed wrappers around byte arrays.** `pub struct UtxoTxId([u8; 32]);` not `[u8; 32]` directly. The newtype prevents accidentally passing a `LeafIndex` where a `TxId` was expected. The compiler enforces; the runtime cost is zero.

## Verification stack

Beyond `cargo test`, the verification budget for primitive code is:

| Tool | What it catches | When to run |
|---|---|---|
| `cargo clippy -- -D warnings` | API mistakes, anti-patterns | every commit |
| `cargo fmt --check` | style drift | every commit |
| `cargo test` | unit tests, doctests | every commit |
| `cargo test --features proptest` | property-based tests | every commit |
| `cargo +nightly miri test` | undefined behaviour in safe AND unsafe code | weekly |
| `cargo kani` | model-checked safety + correctness properties | per primitive change |
| `cargo fuzz run <target>` | coverage-guided fuzzing on parsers | per parser change |
| `cargo mutants` | mutation testing (which lines have no test that catches them) | per release |
| `cargo bench` (criterion) | regression on the benchmark budget | per release |

Skipping the proptest layer for primitive code is the most common source of "looks fine" bugs that bite later.

## Writing for cross-implementation reproducibility

Every primitive in the omega-commitment line needs a second implementation (likely Lean 4 extracting to runnable code) that produces byte-identical output. To make this tractable:

1. **Document the byte format end-to-end.** Not just "this is the leaf encoding"; the full sequence of bytes including domain tags, length prefixes, and endianness.
2. **Provide test vectors.** A `tests/vectors/` directory with input → expected-output pairs that any second implementation can run against.
3. **Avoid Rust-specific encoding shortcuts.** `serde_cbor` is fine; `bincode` (a Rust-native format with no spec) is not.
4. **Pin the hash function parameter set.** Blake2b-256 with default IV; SHA3-256 (NIST FIPS 202); Poseidon2 with the specific MDS matrix and round constants in `tests/vectors/poseidon2.json`.
5. **Make every magic number documented.** No bare constants; every domain tag, every length-cap, every parameter has a name and a docstring explaining its origin.

## When to invoke this skill

- Designing or modifying any code in `omega-commitment-core/src/`, `omega-utxo-snapshot/src/`, or any future plonky3 circuit code
- Reviewing a PR that touches Merkle construction, hash invocation, or leaf encoding
- Writing the spec for a new primitive (use the skill's checklist as the spec template)
- Auditing a downstream consumer of a commitment for membership-proof correctness

The skill is not for general Rust code (use `rust-engineer` or `rust-skills` for that). Invoke this skill specifically when the code in question will end up inside or directly adjacent to a STARK circuit.
