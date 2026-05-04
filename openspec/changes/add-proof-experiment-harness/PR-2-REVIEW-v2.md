# PR #2 Re-Review (v2) — `feat: add omega claim verifier` (HEAD `96939e7`)

- **Branch**: `feat/proof-harness-goblins-v0.1`
- **Head**: `96939e7377cb2e79afef63da81d7df0470276efe`
- **Round 1 verdict**: `request-changes` — P0 soundness gap (membership AIR was content-free).
- **Round 2 scope**: verify the option-1(a) close (real per-step Merkle constraints + Plonky3 LogUp-glued Blake3 compression).
- **CI**: both `test` checks SUCCESS at HEAD.
- **Verdict**: **approve (comment-only, since the user is the PR author)** — the P0 is closed end-to-end, the soundness contract is now mechanically enforced by the AIR plus a real cross-table LogUp permutation argument, and the new negative test exercises real AIR-layer mutations rather than envelope tampering. A handful of P1/P2 hygiene items remain (see Findings).

---

## Summary

Codex closed the PR-1 P0 by replacing `OmegaMembershipAir`'s content-free three-constraint shape with a fully-bound Merkle-walk AIR: every `IS_REAL_STEP` row now constrains `(LEFT, RIGHT) ↔ (PREV, SIBLING)` swap by a `DIRECTION_BIT` column tied to the path-index bit decomposition; the first real row's `LEFT`/`RIGHT` feed a leaf-compression LogUp lookup; subsequent rows' compressions are split into two node-compression LogUp lookups; and the last real row asserts `CURRENT_NODE == public[PUBLIC_ROOT_OFFSET..]` and `path_step_index + has_node == tree_depth`. The Blake3 compression circuit is glued via a real `p3-lookup` permutation argument (LogUp): `OmegaMembershipAir::get_lookups` registers three `Direction::Receive` global lookups per row against the interaction name `"omega_blake3_compression_v1"`, and `OmegaBlake3Air::get_lookups` registers a `Direction::Send` lookup whose tuple matches byte-for-byte. This is glued through `p3-batch-stark`, which sums the cumulated LogUp values across all AIRs and rejects unless they balance (`LookupGadget::verify_global_final_value`). Result: a malicious prover who skips `validate_witnesses` and writes arbitrary bytes into `payload`/`sibling`/`current_node`/`leaf_index_be` cannot satisfy the AIR — either an algebraic constraint violates directly (e.g., `local[COL_LEAF_INDEX_BE + i] != public[...]`), or the LogUp cumulative sum is non-zero. The new `tests/soundness_negative.rs` exercises this in code by tampering each of the four trace columns through a `prove_collection_with_trace_tamper` doc-hidden hook and asserting the verifier rejects.

---

## P0 closure verification

### 1. Per-step Merkle constraint (contract item 1)

**Direction-bit column and swap.** `omega-commitment/crates/omega-claim-prover/src/lib.rs:84` declares `COL_DIRECTION_BIT`. It is constrained boolean at `lib.rs:318`. The swap is expressed at `lib.rs:354-370`:

```rust
node_builder.assert_eq(local[COL_DIRECTION_BIT], local[COL_REMAINING_INDEX_BITS]);
let direction_zero_active = has_node * (AB::Expr::ONE - local[COL_DIRECTION_BIT].into());
let direction_one_active = has_node * local[COL_DIRECTION_BIT];
{
    let mut direction_zero = builder.when(direction_zero_active);
    for byte in 0..HASH_LEN {
        direction_zero.assert_eq(local[COL_LEFT_NODE + byte], local[COL_PREV_NODE + byte]);
        direction_zero.assert_eq(local[COL_RIGHT_NODE + byte], local[COL_SIBLING + byte]);
    }
}
{
    let mut direction_one = builder.when(direction_one_active);
    for byte in 0..HASH_LEN {
        direction_one.assert_eq(local[COL_LEFT_NODE + byte], local[COL_SIBLING + byte]);
        direction_one.assert_eq(local[COL_RIGHT_NODE + byte], local[COL_PREV_NODE + byte]);
    }
}
```

The direction bit is forced equal to the `REMAINING_INDEX_BITS[0]` bit (`lib.rs:354`), and the LSB of `REMAINING_INDEX_BITS` is the next path step's parity. The `REMAINING_INDEX_BITS` shift by one between transitions is at `lib.rs:414-420` — `next[COL_REMAINING_INDEX_BITS + bit] == local[COL_REMAINING_INDEX_BITS + bit + 1]` for `bit in 0..63`, with `next[COL_REMAINING_INDEX_BITS + 63] == 0`. Bit decomposition is bound to the public leaf index at `lib.rs:983-993` (`packed == public[PUBLIC_LEAF_INDEX_OFFSET + LEAF_INDEX_LEN - 1 - byte_index]`).

**`current_node = node_hash_v2(left, right)`.** This is enforced not algebraically but via two LogUp lookups (`node_first_lookup_exprs` at `lib.rs:1062`, `node_second_lookup_exprs` at `lib.rs:1078`). The first lookup tuple includes `LEFT`, `RIGHT[0..32]`, IV, counter=0, block_len=64, flags=`NODE_FIRST_FLAGS`, and asserts the produced output equals `NODE_MID`. The second tuple feeds `RIGHT[32..]` (which is the `node:` domain prefix + 32-byte right hash for the 96-byte preimage's second block), `NODE_MID` as the chaining value, flags=`NODE_SECOND_FLAGS`, and the output is `CURRENT_NODE`. Because both lookups are `Direction::Receive` against `OmegaBlake3Air`'s `Direction::Send`, the LogUp cumulated sum balances only when the actual `(input, cv, counter, len, flags) → output` mapping in `CURRENT_NODE` matches the Blake3 compression on the AIR's input — i.e., `CURRENT_NODE` equals the second-block compression output, which equals `node_hash_v2(left, right)`.

### 2. First real row leaf hash (contract item 2)

**Constraint:** `lib.rs:467-475` registers a global `Direction::Receive` lookup with multiplicity `local[COL_IS_FIRST_STEP]`, whose tuple (`leaf_lookup_exprs` at `lib.rs:1032`) is the leaf preimage block: `DOMAIN_LEAF || sub_tree_id || leaf_index_be (8B) || zero pad to 17 || payload_len (1B) || payload (40B) || zero || IV || zero counter || block_len = (DOMAIN_LEAF + 1 + 8 + 8) + payload_len || flags = LEAF_FLAGS || PREV_NODE`. Because `PREV_NODE` carries the compression output, asserting that this tuple matches the Blake3 trace forces `PREV_NODE` (which equals `CURRENT_NODE` at step 0 when `has_node = 0`, or feeds the next row when `has_node = 1`) to equal `leaf_hash_v2(sub_tree_id, leaf_index_be, payload)`.

The first-row gating: `lib.rs:330-333`:

```rust
let mut first = builder.when_first_row();
first.assert_one(real);
first.assert_one(first_step);
first.assert_zero(local[COL_PATH_STEP_INDEX]);
```

forces the first matrix row to be a real first step with `path_step_index = 0`.

### 3. Last real row root match (contract item 3)

**Constraint:** `lib.rs:381-388`:

```rust
let mut last_builder = builder.when(stop_active.clone());
last_builder.assert_eq(local[COL_PATH_STEP_INDEX] + has_node, local[COL_TREE_DEPTH]);
for byte in 0..HASH_LEN {
    last_builder.assert_eq(
        local[COL_CURRENT_NODE + byte],
        public[PUBLIC_ROOT_OFFSET + byte],
    );
}
```

`stop_active = real * last_step`. The `path_step_index + has_node == tree_depth` check covers both the path-walk case (`has_node = 1`, last step at index `tree_depth - 1`, sum = `tree_depth`) and the depth-0 single-leaf case (`has_node = 0`, step 0, sum = 0 = tree_depth). The 32-byte equality binds `CURRENT_NODE` to the public input.

### 4. `path_step_index` increment (contract item 4)

**Constraints:**
- First-row start: `lib.rs:333` `first.assert_zero(local[COL_PATH_STEP_INDEX])`.
- Increment: `lib.rs:407-410` `continue_builder.assert_eq(next[COL_PATH_STEP_INDEX], local[COL_PATH_STEP_INDEX] + AB::F::ONE)`.
- End-at-depth: `lib.rs:382` `last_builder.assert_eq(local[COL_PATH_STEP_INDEX] + has_node, local[COL_TREE_DEPTH])` (combined with `local[COL_TREE_DEPTH] == public[PUBLIC_TREE_DEPTH_OFFSET]` at `lib.rs:337`, which ties `tree_depth` to a public input).

### 5. Blake3 lookup permutation argument (contract item 5)

**It's real.** `lib.rs:467-494` registers three `Direction::Receive` global lookups per membership row, all sharing the interaction name `BLAKE3_LOOKUP_NAME = "omega_blake3_compression_v1"` (`blake3_trace.rs:12`). `lib.rs:516-525` in `OmegaBlake3Air::get_lookups` registers one `Direction::Send` lookup against the same name, with a multiplicity expression `flag0 + flag1 - flag0*flag1` (i.e., 1 when at least one of the leaf/node-first flags is active, 0 only for the dummy padding rows where both bits are zero). The tuple width is `COMPRESSION_LOOKUP_WIDTH = BLOCK_BYTES + CV_BYTES + COUNTER_BYTES + U32_BYTES + U32_BYTES + CV_BYTES = 64+32+8+4+4+32 = 144` bytes (`blake3_trace.rs:39-40`).

Both AIRs are passed through `p3_batch_stark::prove_batch` (`lib.rs:600`), which the upstream code at `var/upstream/Plonky3/batch-stark/src/prover.rs:115` instantiates a `LogUpGadget` for — i.e., a real LogUp permutation argument. The verifier-side balancing happens in `verify_batch` via `LookupGadget::verify_global_final_value` (`var/upstream/Plonky3/lookup/src/lookup_traits.rs:34-37`): all per-AIR cumulated values must sum to zero per interaction name, otherwise verification fails. This is the gluing step that PR-1's standalone `Blake3Air` proof did not provide.

### 6. `ClaimPublicInputs` v2 shape (contract item 6)

`omega-claim-tx/src/lib.rs:39-58` declares `ClaimPublicInputs` with the new fields:

```rust
pub struct ClaimPublicInputs {
    pub sub_tree_id: u8,
    pub leaf_index: u64,
    pub tree_depth: u8,                                  // NEW
    #[serde(with = "hex::serde")]
    pub per_sub_tree_root: Hash,                         // NEW
    #[serde(with = "hex::serde")]
    pub bundle_root_blake3: Hash,
    #[serde(with = "hex::serde")]
    pub nullifier: Hash,
    #[serde(with = "hex::serde")]
    pub recipient_starstream_addr: Hash,
}
```

`CLAIM_TX_WIRE_VERSION` bumped 1 → 2 at `omega-claim-tx/src/lib.rs:9`. CBOR encode at `lib.rs:262-276` writes a 7-tuple including the two new fields. `omega-claim-prover/src/lib.rs:54` bumps `PROOF_ENVELOPE_VERSION` to `2`. The verifier surfaces both new error variants:
- `VerifyError::WrongSubTreeRoot { index }` (`omega-claim-verifier/src/lib.rs:58-59`), returned at `lib.rs:107-109`.
- `VerifyError::DepthMismatch { index, expected, actual }` (`lib.rs:60-65`), returned at `lib.rs:112-118`.

### 7. `tests/soundness_negative.rs` (contract item 7)

`omega-commitment/crates/omega-claim-prover/tests/soundness_negative.rs:73-101` builds a 256-leaf v1 sub-tree, proves leaf 42, then iterates over `[PayloadByte, SiblingByte, CurrentNodeByte, LeafIndexByte]` and calls a doc-hidden `prove_collection_with_trace_tamper(&commitment, &[witness], &config, tamper)` (`omega-claim-prover/src/lib.rs:553-561`) which threads a `TraceTamper` enum into `build_traces` → `build_membership_trace` → `tamper_trace` (`lib.rs:838-846`). `tamper_trace` adds 1 to a single Val cell at `COL_PAYLOAD`, `COL_SIBLING`, `COL_CURRENT_NODE`, or `COL_LEAF_INDEX_BE + LEAF_INDEX_LEN - 1` of the first row. The test asserts `verify(&commitment, &public_inputs, &proof).is_err()` for each variant. This exercises **AIR-layer mutation**, not envelope tampering: the proof object is constructed from a malformed trace, and the verifier's rejection is forced by the AIR constraints + LogUp imbalance.

The workspace adds `[profile.test.package.p3-batch-stark] debug-assertions = false` (`omega-commitment/Cargo.toml:54-55`) — required because `prove_batch` panics in `check_constraints` under debug builds when a row violates the AIR; disabling debug assertions lets the prover produce a malformed proof object that the verifier can then reject through normal `verify_batch` flow. This is the right plumbing choice for the soundness test.

### 8. Dead-field cleanup (contract item 8)

`Grep` for `membership_transcript_digest` and `Blake3Air::generate_trace_rows` in `omega-commitment/crates/` returns zero hits. The old separate `Blake3Air` proof bytes are gone — replaced by a single `stark_proof` field on `ProofEnvelope` (`omega-claim-prover/src/lib.rs:230-238`). The `OmegaBlake3Air` trace is now a participant in the same batched STARK as the membership AIR(s), glued via LogUp. The dead transcript-digest field is gone too.

### 9. Pure verifier surface (contract item 9)

- `#![forbid(unsafe_code)]` on `omega-claim-verifier/src/lib.rs:1` and `omega-claim-prover/src/lib.rs:1`.
- `Grep` for `tokio | async | unwrap | expect | panic | unreachable | todo` against the verifier crate returns zero hits in production code; the only matches are in `tree_depth: u8 expected = u8::try_from(commitment.tree_depths[sub_tree_index])` (a fallible conversion that maps to `VerifyError::InvalidProof`, `lib.rs:110-111`) and the `Option::unwrap_or` in the prover at `lib.rs:743` which provides a default value, not a panic.
- No I/O, no global state, no async.

### 10. v0.1 leaf-preimage soundness boundary (contract item 10)

`omega-claim-prover/src/lib.rs:632-639`:

```rust
let leaf_preimage_len = leaf_preimage_len(witness.leaf_payload.len());
if leaf_preimage_len > MAX_V01_LEAF_PREIMAGE_LEN {
    return Err(ProverError::LeafTooLargeForV01 {
        actual: leaf_preimage_len,
        limit: MAX_V01_LEAF_PREIMAGE_LEN,
        witness_index,
    });
}
```

Tested at `tests/prover_smoke.rs:93-107`. Module-header doc at `lib.rs:1-10` documents the cap explicitly.

---

## Strengths

- **Real LogUp permutation, not ceremonial AIR**. `lib.rs:467-494` and `lib.rs:516-525` register matched send/receive global lookups against `BLAKE3_LOOKUP_NAME`. Verifier-side balancing happens in `verify_batch` via `LookupGadget::verify_global_final_value` (upstream `lookup/src/lookup_traits.rs:34-37`). This is the design's stated v0.1 → v0.2 migration target landing in v0.1.
- **Direction-bit construction is correct and economical**. `REMAINING_INDEX_BITS` is a 64-element bit-decomposition column shifted by one between transitions (`lib.rs:414-420`), with the LSB driving `DIRECTION_BIT` (`lib.rs:354`). This avoids an explicit `leaf_index_be / 2^step` arithmetic decomposition while still binding the swap to the public input.
- **Padding rows zeroed**. `lib.rs:425-428` `padding.assert_zero(*value)` zeros every column when `IS_REAL_STEP = 0`, eliminating the "junk in unused rows" attack surface.
- **Payload-length one-hot selector**. `lib.rs:957-976` packs the 1-byte payload length as a `LEAF_PAYLOAD_LEN_CHOICES` (=41) one-hot selector and binds it to `COL_PAYLOAD_LEN`, with `assert_zero(local[COL_PAYLOAD + i] * len_lte_index)` zeroing payload bytes past the declared length. This prevents the prover from claiming `payload_len = 5` while smuggling 40 bytes into the lookup tuple. Clean.
- **Per-input wire-format bump tracked transparently**. CBOR golden test in `omega-claim-tx/tests/claim_tx_cbor.rs` regenerated with `tree_depth: 8` and `per_sub_tree_root: [0x33; 32]` fields. `proptest` strategies (`tests/claim_tx_cbor.rs:116-136`) include both new fields. Wire version constant at `omega-claim-tx/src/lib.rs:9` is the single source of truth.
- **New error variants exercised by real round-trip tests**. `verifier_roundtrip.rs:144-153` and `:156-172` mutate the envelope's public inputs through postcard re-encoding and assert the exact `WrongSubTreeRoot { index: 0 }` / `DepthMismatch { ... }` variants. These didn't exist in PR-1.
- **Workspace pin still single source of truth**. `Cargo.toml:36-52` lists 17 `p3-*` crates all at `rev = "fc774b10eb66b1e4b75a1825e1af7acb98bcc71a"`. The new `p3-batch-stark` and `p3-lookup` deps follow the same pattern. Each crate consumes via `workspace = true`. Closes the round-1 P0/M6 hygiene check.
- **Honest sub-task ticking with implementation note carry-over**. `tasks.md:19-20` keeps task 2.4 itself open with a 2026-05-03 implementation note, while ticking the seven 2.4.x sub-tasks that decomposed it. 3.4 is re-ticked at `tasks.md:46`, which is the correct step now that 2.4.1-2.4.7 land. 3.5 still openly notes "p50 not measured." 2.8 still open. This is the right discipline.

---

## Findings

### P0 — none

The round-1 P0 is closed. I attempted to reason about the four mutation classes:

| Mutation | Why it's caught |
|---|---|
| `COL_PAYLOAD` first byte | `leaf_lookup_exprs` (`lib.rs:1032`) feeds the payload bytes into the leaf compression input tuple. The tuple now disagrees with the actual Blake3 compression in `OmegaBlake3Air`'s send-side, so LogUp cumulated sum is non-zero. |
| `COL_SIBLING` first byte | `direction_zero/direction_one` builders at `lib.rs:357-370` force `RIGHT_NODE` (or `LEFT_NODE`) to equal the bytes in `SIBLING`. The `node_first_lookup_exprs` then feeds those bytes into the node compression input. Tampered sibling → wrong node compression input → LogUp imbalance. |
| `COL_CURRENT_NODE` first byte | `node_second_lookup_exprs` (`lib.rs:1078`) puts `CURRENT_NODE` as the *output* of the second compression in the lookup tuple. A wrong byte means the Send side never produces this tuple → LogUp imbalance. **Plus** in a depth-1 single-step tree, `last_builder` (`lib.rs:381-388`) directly checks `CURRENT_NODE == public[PUBLIC_ROOT_OFFSET..]`, so the first-and-last row case is doubly caught. |
| `COL_LEAF_INDEX_BE` LSB byte | `real_builder.assert_eq(local[COL_LEAF_INDEX_BE + offset], public[PUBLIC_LEAF_INDEX_OFFSET + offset])` (`lib.rs:339-342`). Tamper → algebraic constraint immediately violated. |

All four reject paths are mechanically forced.

### P1 — `LeafIndexByte` tamper test could be strengthened

**Where**: `omega-commitment/crates/omega-claim-prover/src/lib.rs:838-846` and `tests/soundness_negative.rs:82-99`.

`tamper_trace` mutates only the LSB of `COL_LEAF_INDEX_BE` by adding 1 to a single Val cell. That hits the algebraic `assert_eq` at `lib.rs:339-342` directly. But the test doesn't verify that mutating the corresponding `REMAINING_INDEX_BITS` bit (at `COL_REMAINING_INDEX_BITS + 0` for the LSB of `leaf_index`) is also rejected — and that's a more interesting case because the algebraic packed-byte check at `lib.rs:983-993` will catch any inconsistency between the bits and the BE bytes. A future strengthening could add a `RemainingBitFlip` variant or a `DirectionBitFlip` variant. Not blocking.

### P1 — Tampered-proof byte test from PR-1 still fragile

**Where**: `omega-commitment/crates/omega-claim-verifier/tests/verifier_roundtrip.rs:101-109`.

PR-1's P1 finding ("midpoint XOR may fall in early envelope fields with `--no-default-features` postcard or future layout changes") was not addressed. The test still flips `proof.0[midpoint] ^= 0x01` and asserts `VerifyError::InvalidProof`. The new `WrongSubTreeRoot` and `DepthMismatch` variants make the assertion-vs-reality space larger: a midpoint flip is statistically very unlikely to land in the new public-input fields, but the test's `assert_eq!(err, VerifyError::InvalidProof)` is brittle. **Recommendation**: change to `assert!(matches!(err, VerifyError::InvalidProof | VerifyError::CommitmentMismatch | VerifyError::PublicInputMismatch | VerifyError::UnsupportedVersion { .. } | VerifyError::WrongSubTreeRoot { .. } | VerifyError::DepthMismatch { .. } | VerifyError::PublicBundleRootMismatch { .. } | VerifyError::UnknownSubTree { .. }))` or run a proptest over flip indices.

### P1 — `bench_*_p50.rs` are still measurement tools, not gates (carry-over)

**Where**: `omega-commitment/crates/omega-claim-prover/benches/bench_prove_p50.rs:92-95` and `omega-commitment/crates/omega-claim-verifier/benches/bench_verify_p50.rs:99-102`.

Both still use `Criterion::default().sample_size(10)` and report rather than assert. tasks.md 2.8 and 3.5 are correctly still open in this PR — but these benches will need to actually be run before either can tick.

### P2 — `cardano-wiki/wiki/log.md` entry for the soundness fix is missing

**Where**: `cardano-wiki/wiki/log.md` ends at line 396 with the original PR-2 entry; no follow-up entry describes the option-1(a) fix.

The PR-1 review (`PR-2-REVIEW.md`) was committed (`ae74b2e`) and the `tasks.md` was extended with sub-tasks 2.4.1-2.4.8 (`2592659`), but the new `96939e7` "fix: constrain omega claim membership air" commit didn't append a log entry. Recommendation: append a `## [2026-05-03] resolve | proof-experiment harness — close PR-2 P0 soundness gap (option 1a)` entry summarising the AIR-constraint expansion + LogUp gluing + new public inputs + new negative test.

### P2 — `prover_smoke.rs` tests didn't add a soundness scenario

**Where**: `omega-commitment/crates/omega-claim-prover/tests/prover_smoke.rs`.

Three smoke tests: (1) honest 256-leaf prove non-empty, (2) tampered path → `PathMismatch`, (3) oversized payload → `LeafTooLargeForV01`. The new soundness test is in a separate file (`soundness_negative.rs`) and exercises four trace mutations against the verifier. That's fine, but the prover smoke now reads as testing only the off-circuit `validate_witness` rejection path, with no link to the AIR-layer soundness from inside the prover crate. A simple cross-reference comment in `prover_smoke.rs` pointing to `soundness_negative.rs` would help readers.

### P2 — `proof_airs(membership_count)` documentation

**Where**: `omega-commitment/crates/omega-claim-prover/src/lib.rs:256-264`.

The function returns `Vec<OmegaProofAir>` with `membership_count` membership AIRs followed by exactly one Blake3 AIR. This contract is implicit; a doc comment saying "the Blake3 AIR is always last and is shared across all membership AIRs via the global LogUp interaction `omega_blake3_compression_v1`" would prevent future call-site bugs. The verifier (`omega-claim-verifier/src/lib.rs:162`) calls `proof_airs(envelope.public_inputs.len())` and then `proof.degree_bits.len() != airs.len()` — so the membership-AIR-count is always equal to `public_inputs.len()`, which is also undocumented.

### P2 — Profile override comment

**Where**: `omega-commitment/Cargo.toml:54-55`:

```toml
[profile.test.package.p3-batch-stark]
debug-assertions = false
```

This is intentional and correct — it disables `check_constraints`'s panic-on-bad-row in test builds so the soundness_negative test can exercise the verifier rejection path rather than tripping a debug panic in the prover. But the override is silent. Recommendation: add a comment block explaining why, e.g., `# Required for tests/soundness_negative.rs: prove_batch panics in debug-mode check_constraints when the trace violates the AIR; the test mutates trace columns and relies on verify_batch to reject the resulting malformed proof.`

### P2 — `OmegaBlake3Air` send-multiplicity expression

**Where**: `omega-commitment/crates/omega-claim-prover/src/lib.rs:513-515`:

```rust
let flag0 = expr(local[B3_FLAGS_OFFSET]);
let flag1 = expr(local[B3_FLAGS_OFFSET + 1]);
let multiplicity = flag0.clone() + flag1.clone() - flag0 * flag1;
```

This is `flag0 OR flag1` expressed arithmetically, treating both bits as boolean. The leaf-flag bit (CHUNK_START, bit 0) and node-second flag bit (CHUNK_END, bit 1) drive multiplicity. Dummy padding rows have flags=0 → multiplicity 0 → no contribution. This is correct, but a comment explaining "multiplicity = is-leaf OR is-node-second-block; node-first-block also has CHUNK_START set, so its contribution flows through this same expression" would help. Today the reader has to cross-reference `LEAF_FLAGS = CHUNK_START | CHUNK_END | ROOT`, `NODE_FIRST_FLAGS = CHUNK_START`, `NODE_SECOND_FLAGS = CHUNK_END | ROOT` (`blake3_trace.rs:13-18`) to convince themselves padding is the only zero-multiplicity case.

---

## Soundness checklist (A–H)

### A. AIR per-step constraints

- **`BIT` column**: `COL_DIRECTION_BIT` at `lib.rs:84`. Constrained boolean at `lib.rs:318`. Tied to `REMAINING_INDEX_BITS[0]` at `lib.rs:354`.
- **(left, right) ↔ (sibling, prev_current_node) swap by BIT**: `lib.rs:357-370` two `builder.when` branches with byte-by-byte `assert_eq`. ✓
- **First real row `current_node = leaf_hash_v2(...)`**: enforced indirectly via `leaf_lookup_exprs` LogUp (`lib.rs:467-475` register, `lib.rs:1032` tuple) sending the leaf preimage block and receiving a compression output at `PREV_NODE`. Combined with `direction_zero` / `direction_one` constraints, `LEFT/RIGHT` at step 1 derive from step 0's `PREV_NODE`. ✓ (modulo correctness of the LogUp glue, which I confirm by inspecting the upstream batch-stark prover at `var/upstream/Plonky3/batch-stark/src/prover.rs:115` instantiating `LogUpGadget`).
- **Last real row `current_node == public[PUBLIC_ROOT_OFFSET..]`**: `lib.rs:381-388`. ✓
- **`path_step_index` start/increment/end**: starts at 0 (`lib.rs:333`), increments by 1 (`lib.rs:407-410`), ends at `tree_depth - has_node` (`lib.rs:382`). ✓

### B. Blake3 lookup is a real Plonky3 permutation argument

**Yes.** Three pieces of evidence:
1. `omega-claim-prover/src/lib.rs:447-496` implements `LookupAir<F> for OmegaMembershipAir` with `register_lookup(Kind::Global("omega_blake3_compression_v1"), &[(tuple, multiplicity, Direction::Receive)])` for leaf, node-first, and node-second.
2. `lib.rs:498-527` implements `LookupAir<F> for OmegaBlake3Air` with `Direction::Send` against the same global name.
3. `prove_collection_inner` (`lib.rs:600`) calls `prove_batch(&stark_config, &instances, &prover_data)`. Upstream `var/upstream/Plonky3/batch-stark/src/prover.rs:115` instantiates `LogUpGadget` and runs the LogUp permutation argument. The verifier's `verify_batch` (`omega-claim-verifier/src/lib.rs:180`) consumes the same data.

This is **not** a separate-trace ceremonial proof. The Blake3 trace and membership trace are part of the same batched STARK and are forced to agree on the lookup tuples by the LogUp running-sum constraint, whose final cumulated value is checked to be zero by `LookupGadget::verify_global_final_value` (upstream `lookup/src/lookup_traits.rs:34-37`).

### C. `soundness_negative.rs` exercises real AIR-layer mutations

**Yes.** `omega-claim-prover/tests/soundness_negative.rs:88-94` calls `prove_collection_with_trace_tamper`, a `#[doc(hidden)]` hook in `omega-claim-prover/src/lib.rs:553-561` that threads a `TraceTamper` enum into `build_traces` → `build_membership_trace` → `tamper_trace` (`lib.rs:838-846`). `tamper_trace` adds 1 to a single Val cell at `COL_PAYLOAD`, `COL_SIBLING`, `COL_CURRENT_NODE`, or `COL_LEAF_INDEX_BE + LEAF_INDEX_LEN - 1` of the first row. The verifier rejects in all four cases. This is **not** a tautological envelope-byte mutation; it's a trace-column mutation made before the proof is generated.

The test relies on `[profile.test.package.p3-batch-stark] debug-assertions = false` (`Cargo.toml:54-55`) to prevent `prove_batch`'s debug-mode `check_constraints` from panicking before the prover gets a chance to produce the malformed proof. This is the right plumbing decision.

### D. Verifier `WrongSubTreeRoot` / `DepthMismatch` exercised

- `WrongSubTreeRoot`: `verifier_roundtrip.rs:144-153` mutates `envelope.public_inputs[0].per_sub_tree_root[0] ^= 0x01`, re-encodes the envelope (so the call-site `public_inputs == envelope.public_inputs` check passes), and asserts `Err(WrongSubTreeRoot { index: 0 })`. ✓
- `DepthMismatch`: `verifier_roundtrip.rs:156-172` mutates `tree_depth += 1`, re-encodes, and asserts `Err(DepthMismatch { ... })`. ✓

Both new error variants have direct round-trip coverage.

### E. Dead-field cleanup

- `envelope.membership_transcript_digest`: `Grep` for `membership_transcript` across `omega-commitment/crates/` returns 0 hits. ✓ Removed.
- Standalone `Blake3Air` proof bytes: `Grep` for `Blake3Air::generate_trace_rows` returns 0 hits. The `ProofEnvelope` (`lib.rs:230-238`) has a single `stark_proof: Vec<u8>` field — the previous `blake3_compression_proof: Vec<u8>` field is gone. ✓ Subsumed.

### F. Pure surface

- `#![forbid(unsafe_code)]`: present at `omega-claim-verifier/src/lib.rs:1` and `omega-claim-prover/src/lib.rs:1`. ✓
- No `unwrap` / `expect` / `panic` / `unreachable` / `todo` in the public verifier surface (the only `unwrap_or` in the prover at `lib.rs:743` is `Option::unwrap_or` providing a default, not a panic).
- No `tokio` / `async`. ✓
- No I/O (no `std::fs`, no `std::io::{Read, Write}`, no `std::process`). ✓
- No global mutable state. ✓

### G. Workspace hygiene

- All 17 `p3-*` workspace deps pinned to the same `rev = "fc774b10eb66b1e4b75a1825e1af7acb98bcc71a"` (`Cargo.toml:36-52`). ✓
- New deps `p3-batch-stark` and `p3-lookup` consumed via `workspace = true` in both prover and verifier `Cargo.toml`. ✓
- New profile override `[profile.test.package.p3-batch-stark] debug-assertions = false` (`Cargo.toml:54-55`) — unusual but justified (P2 finding above).

### H. Tasks.md audit

- 2.4.1 — `BIT` swap + first-row leaf-hash: ticked, real (`lib.rs:354-370`, `lib.rs:1032-1060`).
- 2.4.2 — `path_step_index` + `tree_depth` public + last-row root match: ticked, real (`lib.rs:333`, `lib.rs:407-410`, `lib.rs:381-388`, `lib.rs:337`).
- 2.4.3 — Plonky3 permutation argument gluing leaf/node hash to Blake3 compression rows: ticked, real (`lib.rs:447-527`, upstream `LogUpGadget` in `batch-stark/src/prover.rs:115`).
- 2.4.4 — `ClaimPublicInputs` v2 + verifier surface: ticked, real (`omega-claim-tx/src/lib.rs:39-58`, `omega-claim-verifier/src/lib.rs:107-118`).
- 2.4.5 — `tests/soundness_negative.rs`: ticked, real (`tests/soundness_negative.rs:73-101`).
- 2.4.6 — module-header doc on 64-byte cap: ticked, real (`omega-claim-prover/src/lib.rs:1-10`).
- 2.4.7 — existing tests still pass: ticked, real (CI green, `prover_smoke.rs` and `verifier_roundtrip.rs` exercised under the new shape with the two new fields plumbed).
- 2.4.8 — re-tick 3.4: ticked. tasks.md 3.4 is now ticked at `tasks.md:46`.
- Task 2.4 itself: still open with implementation note. This is the right call — even though the option-1a sub-tasks land, the "(this constrains the compression function only; preimage gluing for ≤ 64-byte leaves is checked deterministically by the verifier (no separate AIR needed because the preimage fits in one compression block))" wording in 2.4 reflects an alternate v0.1 closure path that the option-1a delivery effectively obsoletes — the gluing now happens in-circuit via LogUp, not at the verifier boundary. **Suggestion**: re-tick 2.4 too with a follow-up note, or rewrite 2.4 to reflect "subsumed by 2.4.1-2.4.8."
- 2.8 (prove p50 bench): correctly still open.
- 3.5 (verify p50 bench): correctly still open.

---

## Recommendation

**comment-only (effectively approve).**

The PR-1 P0 is closed end-to-end with no hand-waving: the `OmegaMembershipAir`'s `Air<AB>::eval` now writes ~13 distinct algebraic constraints binding `LEAF_INDEX_BE`, `PAYLOAD`, `SIBLING`, `CURRENT_NODE`, `DIRECTION_BIT`, `PATH_STEP_INDEX`, `TREE_DEPTH`, and the boolean flags; three LogUp lookups per row glue the leaf and node compressions to a real `OmegaBlake3Air` participant in the same batched STARK; `verify_batch` rejects unless cumulated LogUp values balance. The four mutation classes from the round-1 attack model are mechanically rejected. The seven 2.4.x sub-tasks all correspond to real code with file:line evidence. Wire-format, golden tests, and verifier error variants all updated coherently. Findings are P1/P2 hygiene only (sample_size still low for benches; tampered-byte test still uses `assert_eq` against a single variant; missing log.md entry; profile override missing comment; minor cross-reference docs).

Concrete next actions:
1. Append a `cardano-wiki/wiki/log.md` entry summarising the soundness fix (P2).
2. Loosen the `assert_eq!(err, VerifyError::InvalidProof)` in `verifier_roundtrip.rs:108` to a `matches!` over the full `VerifyError` set, or drive it via proptest (P1).
3. When `bench_prove_p50` and `bench_verify_p50` finally get measured, lift `sample_size` to 100 and capture a markdown report; tick 2.8 / 3.5 only after that. (P1, carry-over.)
4. Add a comment to `Cargo.toml`'s `[profile.test.package.p3-batch-stark]` block explaining the rationale (P2).
5. Re-tick task 2.4 (or rewrite it as "subsumed by 2.4.1-2.4.8") — the implementation note's "deterministic verifier-boundary gluing" is no longer the v0.1 strategy (P2).
