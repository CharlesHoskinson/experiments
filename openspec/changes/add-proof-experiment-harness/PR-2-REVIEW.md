# PR #2 Review — `feat: add omega claim verifier`

- **Branch**: `feat/proof-harness-goblins-v0.1`
- **Head**: `c91c1de3e04c80b541350802b5ceea6dbf8640e7`
- **Scope reviewed**: task group 3 of `add-proof-experiment-harness` (omega-claim-verifier crate + prover binding update + workspace + log + tasks.md ticks)
- **CI**: green at time of review
- **Verdict**: **request-changes** — there is a P0 soundness hole in the membership AIR that the new binding fix does not close.

---

## Summary

The PR cleanly lands a new pure-function `verify(commitment, public_inputs, proof) -> Result<(), VerifyError>` crate with no async, no I/O, no `unsafe`, and no `unwrap`/`expect` on adversarial input, and it correctly extends the prover's public values with a Blake3-derived binding digest over `(commitment, public_inputs)`. The three spec.md scenarios are exercised and the round-trip plus tampered-proof plus wrong-commitment plus public-input-mismatch plus envelope-rewrite tests all pass deterministically. The Plonky3 git rev is still pinned once at the workspace level. **However**, the underlying `OmegaMembershipAir` (introduced in PR #1, not modified here) constrains nothing about the Merkle path, the leaf preimage, the per-step Blake3 compression input/output relationship, or the path-to-bundle-root walk; the only constraints are first-row `sub_tree_id == public_value_0`, accumulator-recurrence, and last-row `acc == public_value_1`. The verifier accepts any proof whose envelope is consistent with the caller's `(commitment, public_inputs)` and whose accumulator is computed correctly from the trace bytes — but a malicious prover can put **arbitrary unconstrained bytes** in the `payload`, `BLAKE3_STATE`, `SIBLING`, and `CURRENT_NODE` columns and still produce an accepting proof. The "leaf-preimage gluing checked deterministically by the verifier" promise from the design is **not implemented in this PR**: the verifier never re-runs `walk_v1_path` or `leaf_hash_v2` against `commitment.sub_tree_roots_blake3`, never re-derives the Blake3 compression input from the public preimage, and the `membership_transcript_digest` field on the envelope is dead. This is a P0; without it, "verify(commitment, public_inputs, proof) == Ok(())" does not imply "public_inputs were validly committed in commitment", which is the entire point of the verifier.

---

## Strengths

- **Pure surface, zero panic surface**. `omega-commitment/crates/omega-claim-verifier/src/lib.rs:1` opens with `#![forbid(unsafe_code)]`. Searching the file for `unwrap | expect | panic | unreachable | todo` yields zero hits. Every fallible op (`postcard::from_bytes`, `verify_stark`) is mapped to a typed `VerifyError`, and `trace_height` (`src/lib.rs:119-124`) explicitly guards `degree_bits >= usize::BITS` before shifting. No tokio, no async, no I/O. Spec.md "deterministic and side-effect free (no I/O, no async, no global state)" satisfied.
- **Binding-digest fix is correct in shape**. `omega-commitment/crates/omega-claim-prover/src/lib.rs:530-567` adds `proof_binding_words` / `proof_binding_digest` over a Blake3 of `DOMAIN_PROOF_BINDING || bundle_root || sub_tree_roots || item_counts || leaf_counts || tree_depths || len(public_inputs) || (sub_tree_id, leaf_index, bundle_root, nullifier, recipient_starstream_addr)*`. Domain-separated, length-prefixed, every field of `ClaimPublicInputs` covered. `proof_public_values` (`src/lib.rs:518-528`) places these as 8 of the 10 public values written into the STARK; the verifier (`omega-claim-verifier/src/lib.rs:85-89`) recomputes them from the call-site `(commitment, public_inputs)` and compares against `envelope.public_values[2..]`. The new test `verifier_rejects_envelope_public_inputs_rewritten_with_matching_call_args` (`tests/verifier_roundtrip.rs:130-139`) exercises exactly the regression mode the implementation note describes.
- **Spec.md error-variant alignment is faithful**. `VerifyError::CommitmentMismatch` and `VerifyError::InvalidProof` (`src/lib.rs:46-59`) are the exact names the spec mandates; `PublicInputMismatch`, `PublicBundleRootMismatch`, and `UnsupportedVersion` round out the surface without bleeding past `#[non_exhaustive]`.
- **Workspace-level Plonky3 pin preserved**. The new crate's `Cargo.toml` (`omega-commitment/crates/omega-claim-verifier/Cargo.toml:13-27`) consumes every `p3-*` dep via `workspace = true`. Workspace `Cargo.toml:44-58` keeps the single `rev = "fc774b10eb66b1e4b75a1825e1af7acb98bcc71a"` for all 15 p3 crates. Closes M6 from QA-REVIEW.md.
- **Verifier instantiates the same StarkConfig as the prover**. `make_stark_config` in the verifier (`src/lib.rs:126-139`) is byte-for-byte identical to the prover's, parameterised on `(trace_height, rng_seed)` with `rng_seed` drawn from `envelope.config.rng_seed`. Round-tripping through the actual `verify_stark` is what the spec requires.
- **Test fixture is shared and deterministic**. `OnceLock<Fixture>` in `tests/verifier_roundtrip.rs:22` amortises one prove across five tests; each test mutates a clone, so cross-test interference is impossible. Cuts roughly 4× off cold test time vs naive re-prove-per-test.

---

## Findings

### P0 — Membership AIR is content-free; verifier accepts proofs of arbitrary leaves

**Where**: `omega-commitment/crates/omega-claim-prover/src/lib.rs:228-252` (AIR `eval`), `omega-commitment/crates/omega-claim-prover/src/lib.rs:454-483` (`fill_real_row`), `omega-commitment/crates/omega-claim-verifier/src/lib.rs:97-117` (verifier path).

**Constraint inventory** in `OmegaMembershipAir::eval`:
1. `local[COL_IS_REAL_STEP]` is bool.
2. First row: `local[COL_SUB_TREE_ID] == public_values[0]`.
3. Transition: `next[COL_IS_REAL_STEP]` is bool, and `local.acc * 257 + next.checksum == next.acc` where `checksum = sum of row[0..COL_ACCUMULATOR]`.
4. Last row: `local.acc == public_values[1]`.

The columns `COL_LEAF_INDEX_BE` (8 bytes), `COL_PAYLOAD_LEN`, `COL_PAYLOAD` (40 bytes), `COL_BLAKE3_STATE` (16 words), `COL_SIBLING` (32 bytes), `COL_CURRENT_NODE` (32 bytes) **contribute their numeric values to the row checksum** but are **not constrained otherwise**. Specifically:
- No constraint that `COL_BLAKE3_STATE` equals Blake3 of any preimage.
- No constraint that `COL_CURRENT_NODE` equals `node_hash_v2(prev_current, sibling)` according to the parity bit of `leaf_index`.
- No constraint that the last row's `COL_CURRENT_NODE` equals any sub-tree root in `commitment.sub_tree_roots_blake3`.
- No constraint that `COL_LEAF_INDEX_BE` matches `public_inputs[i].leaf_index`.
- No constraint that `COL_PAYLOAD` is the actual leaf preimage that hashes to a leaf included in the tree.
- The standalone `Blake3Air` proof (`omega-commitment/crates/omega-claim-prover/src/lib.rs:293-312`) is built on a `Blake3Air::generate_trace_rows::<Val>(rows, 1)` matrix that is **synthesised by the AIR itself, not from the witness preimages**. It proves only "Blake3 compression was correctly evaluated on some inputs", not on any specific inputs tied to this claim.

**Verifier-side gluing**: the verifier checks `envelope.commitment == call_commitment`, `envelope.public_inputs == call_public_inputs`, `public_inputs[i].bundle_root_blake3 == commitment.bundle_root_blake3`, the binding-words match, and the two STARK proofs verify. **It never re-runs `walk_v1_path` against `commitment.sub_tree_roots_blake3`**, never re-derives `leaf_hash_v2` from the public inputs, and never even reads `envelope.membership_transcript_digest` — the `membership_transcript_digest` field is dead in the verifier (`Grep` for `membership_transcript` and `walk_v1_path` in the verifier returns no matches; `Grep` for `leaf_hash_v2 | node_hash_v2 | merkle_path | terminal_root` likewise none).

**Attack**. A malicious prover (one byte-for-byte identical to the honest prover except `validate_witnesses` is bypassed and `walk_v1_path` is skipped) constructs:
1. A `MembershipWitness` with `public.sub_tree_id = 1`, `public.leaf_index = 999_999_999`, `public.bundle_root_blake3 = commitment.bundle_root_blake3`, `nullifier = 0xFF…`, `recipient = victim_addr`, `leaf_payload = b"give me your money"`, `merkle_path = [random; 20]` (no relationship to any tree).
2. A trace where every row's `COL_PAYLOAD`/`COL_BLAKE3_STATE`/`COL_SIBLING`/`COL_CURRENT_NODE` is filled with **arbitrary bytes** (or all zeros). The accumulator constraint is satisfied because it's purely arithmetic over whatever bytes are in the row — the malicious prover computes `acc` honestly over its arbitrary trace.
3. Public values: `[first_sub_tree_id, final_acc, ...proof_binding_words(commitment, public_inputs)]`. The binding words are public, so the malicious prover trivially knows them.
4. Run `prove(&stark_config, &OmegaMembershipAir, malicious_trace, &public_values_as_fields(public_values))`. Plonky3 accepts because the trace satisfies all four constraints listed above.
5. Run `prove_blake3_compressions` — same as honest, since `Blake3Air::generate_trace_rows` is internally synthesised.
6. Pack into a `ProofEnvelope` with `commitment` and `public_inputs` matching the call-site arguments.
7. Submit. The verifier's commitment-equality check passes (envelope.commitment == call_commitment), the public-input check passes (envelope.public_inputs == call_public_inputs), the binding-word check passes (binding words were correctly computed), the inner STARK verifies. Returns `Ok(())`.

**Impact**. The verifier's contract — "returns `Ok(())` exactly when the proof attests to inclusion of the listed leaves under the listed bundle root" (spec.md:43) — is **violated**. A claim against any (sub_tree_id, leaf_index, nullifier, recipient) is acceptable regardless of whether the leaf actually exists in the tree. Once `omega-mock-ledger` lands and apply takes verifier acceptance as gospel, every nullifier slot is forgeable, every Starstream UTxO is mintable, every recipient address is hijackable. This is the same class of bug as a smart-contract verifier that only checks `commitment == claimed_commitment` instead of `H(witness) == commitment && witness ∈ allowed_set`.

The implementation note on tasks.md:32 ("`(commitment, public_inputs)` is bound into the proof public values") describes a real upgrade — it closes the envelope-rewrite class — but does **not** close the membership-AIR-is-empty class. The two are independent.

**Why the existing tests pass anyway**. Each of the five tests in `tests/verifier_roundtrip.rs` calls the **honest** `prove_collection`, which runs `validate_witnesses` (`omega-commitment/crates/omega-claim-prover/src/lib.rs:259-260`) and produces a trace where the unconstrained columns *happen* to contain the correct bytes. The tests never construct a malicious trace, so they never observe the gap. CI green is consistent with the soundness hole.

**Recommendation** — pick one:

- **(a)** Add real constraints to `OmegaMembershipAir`: at minimum, that the last-row `CURRENT_NODE` equals the bundle's `sub_tree_roots_blake3[sub_tree_id - 1]` (passed as additional public values), and that each transition step reflects a correct `node_hash_v2`. This requires constraining Blake3 compressions inside the AIR via permutation argument — which is exactly what task 2.4 explicitly says is **still open** (tasks.md:19). Closing 3.4 cannot be claimed safely without 2.4.
- **(b)** Add deterministic verifier-side gluing: in `omega-claim-verifier::verify`, re-run `walk_v1_path(leaf_hash_v2(public.sub_tree_id, public.leaf_index, ???), public.leaf_index, merkle_path)` and assert the result equals `commitment.sub_tree_roots_blake3[idx]`. The blocker is that `merkle_path` and `leaf_payload` are **not in `ClaimPublicInputs`** and therefore not available to the verifier — they live in `ClaimWitness` (`omega-commitment/crates/omega-claim-tx/src/lib.rs:55-67`), which the verifier deliberately never sees. So option (b) requires either widening the public-inputs surface or smuggling the path/preimage through the proof envelope as additional public values.
- **(c)** Mark v0.1 as **not yet sound** in `spec.md` and `cardano-wiki/wiki/log.md`. Add an explicit warning in the verifier's lib doc-comment ("v0.1 verifier checks proof-envelope consistency only; the membership AIR is structural and v0.2 closes the gluing constraint"). Re-tick task 3.4 only after v0.2.

Option (a) is the design's stated long-term plan and the right answer; option (b) is the v0.1 escape valve the design.md:104 calls "checked deterministically by the verifier" — but that path requires extending `ClaimPublicInputs` (or the proof envelope's public-input view) with the leaf payload + merkle path bytes, which is out of scope for this PR. **Until either lands, the verifier should not be ticked complete.**

This is the same class of issue QA-REVIEW.md flagged in P1 ("`p3-blake3-air` proves Blake3 *compressions*, not Blake3 *hashes*; the C1/C3 mapping is hand-waved", QA-REVIEW.md:67-86) — except the implementation has gone further than the spec by also leaving the *path-walk* gluing unimplemented. The QA pre-flag did not call out this specific regression because the design.md text suggested (incorrectly, as it turns out) that the verifier would re-run the path. It does not.

---

### P1 — Tampered-proof test is fragile and does not exercise the binding check

**Where**: `omega-commitment/crates/omega-claim-verifier/tests/verifier_roundtrip.rs:99-107`.

`fixture.proof.0[midpoint] ^= 0x01` flips one byte at the midpoint of the postcard-encoded envelope. The envelope layout is `version(1) || config(8) || commitment(~300) || public_inputs(~80*N) || transcript_digest(32) || public_values(40) || stark_proof(~MB) || blake3_compression_proof(~MB)`. With `payloads(16)` and 1 leaf, the proof bytes are dominated by the two STARK proofs (each multi-MB), so midpoint reliably lands in `stark_proof` or `blake3_compression_proof` and the test reliably returns `InvalidProof`. **However**, the test does not exercise the case where the tampered byte lands in a field whose mismatch is caught by an earlier check — which would surface a different error variant (e.g., `CommitmentMismatch` if it lands in `commitment.bundle_root_blake3`, `PublicInputMismatch` if it lands in `public_inputs[0]`, `UnsupportedVersion` if it lands in `version`). The spec.md scenario "any byte of a valid proof byte string is flipped" implies **any byte**, not "midpoint of a multi-MB envelope". A property test that picks a random index in `0..proof.0.len()` and asserts the result is `Err(_)` (not specifically `Err(InvalidProof)`) would more honestly cover the spec.

The risk is small (the verifier returns `Err` for all tamper sites), but the asserted error variant is not reliably `InvalidProof` for sites in the early envelope fields. **Recommendation**: replace `assert_eq!(err, VerifyError::InvalidProof)` with `assert!(matches!(err, VerifyError::InvalidProof | VerifyError::CommitmentMismatch | VerifyError::PublicInputMismatch | VerifyError::UnsupportedVersion { .. }))`, or run the test as a proptest over flip-index. The current `assert_eq!` is brittle to envelope-layout changes (e.g., a future field reorder or a change in `sample_size` would shift midpoint location and could break the test for unrelated reasons).

---

### P1 — `bench_verify_p50.rs` does not assert the spec's p50 < 500 ms target

**Where**: `omega-commitment/crates/omega-claim-verifier/benches/bench_verify_p50.rs:79-95`.

The bench measures `verify(...)` at 1, 16, 256 leaves and reports criterion samples — but never asserts `p50 < 500 ms`. tasks.md 3.5 says "assert verify p50 < 500 ms on the developer laptop"; the implementation note says "the task remains open until the benchmark is run and the p50 assertion is backed by local measurements." Marking 3.5 still open is honest. The bench will compile-and-run via `cargo bench`, but it is currently a measurement tool, not a gate. `Criterion::default().sample_size(10)` (`bench_verify_p50.rs:99`) is also too few samples for a statistically meaningful p50 — criterion's default is 100; 10 makes the median estimator noisy.

`proof_fixture` (`bench_verify_p50.rs:48-77`) builds a 256-leaf tree and proves the first `count` of them in **one** prove call. For `count = 1`, this is the same as a single-leaf proof; for `count = 256`, this is the largest single-batch case. That matches the spec scenarios. Note: the function calls `prove_collection` once per iteration of the outer `for count in [1, 16, 256]` loop, so the bench setup is dominated by three prove calls (each potentially multi-second), but the inner `b.iter` runs only `verify` — so the wall-time of `cargo bench -p omega-claim-verifier` is roughly `3 × prove_time + 30 × verify_time`.

**Recommendation**: bump `sample_size` to at least 100 for p50; either run the bench once locally and capture the numbers into the implementation note, or add an assertion in a separate `tests/verify_p50.rs` (run as a regular test, with `#[ignore]` if it's slow) that calls `verify` 100 times and asserts the median is under 500 ms. The current bench is necessary infrastructure but does not satisfy 3.5.

---

### P1 — `walk_v1_path` direction-bit comparison: prover validates, but the AIR encodes a different ordering

**Where**: `omega-commitment/crates/omega-claim-prover/src/lib.rs:379-391` (`walk_v1_path`) vs `omega-commitment/crates/omega-claim-prover/src/lib.rs:454-483` (`fill_real_row`).

`walk_v1_path` (used by `validate_witness`, off-circuit) decides `if idx & 1 == 0 { node_hash_v2(&current, sibling) } else { node_hash_v2(sibling, &current) }`. `fill_real_row` (used to populate the trace, the data the AIR will sign over) writes `current = if idx & 1 == 0 { node_hash_v2(&current, &sibling) } else { node_hash_v2(&sibling, &current) }` (line 416-419). The two match. Good — but neither is exposed to the AIR's constraints (see P0). The redundancy means the prover's trace is **internally consistent** with the off-circuit walk, which is *necessary* for verifier acceptance under a real path-walk constraint, but is *not sufficient* for soundness because no AIR constraint forces the relationship.

Restating P0 from a different angle: this is two implementations of the same Merkle walk, side by side, with no constraint linking them. If a follow-up patch closes P0 by adding constraints to the AIR, it must explicitly check the parity-bit branch — bit 0 of the row's `COL_LEAF_INDEX_BE` low byte selecting between `node_hash_v2(current, sibling)` and `node_hash_v2(sibling, current)`. That's the constraint that's currently missing and that turns the row data from "hint bytes" into "proof bytes".

---

### P2 — Negative-case coverage gaps

**Where**: `omega-commitment/crates/omega-claim-verifier/tests/verifier_roundtrip.rs`.

The five tests cover: valid, tampered (single byte mid-envelope), wrong commitment (one byte XOR in bundle_root), public-input mismatch (one byte XOR in nullifier), envelope-rewrite (XOR in recipient, then re-encode envelope with matching call args). Distinct, each surfacing a different code path. **Missing**:

- **Empty proof bytes**: `verify(&c, &p, &ProofBytes(vec![]))` — should return `InvalidProof`, not panic.
- **Truncated proof bytes**: `verify(&c, &p, &ProofBytes(proof.0[..proof.0.len()/2].to_vec()))` — postcard partial decode behaviour.
- **Oversized proof bytes**: `verify(&c, &p, &ProofBytes(vec![0u8; 64 * 1024 * 1024]))` — should not allocate or hang.
- **Wrong sub-tree root**: change `commitment.sub_tree_roots_blake3[6]` (an unrelated sub-tree); the bundle root shifts so this is caught by `CommitmentMismatch` — but a follow-up should also exercise "valid proof against sub_tree 1, submit with public_input claiming sub_tree 2" to surface the sub-tree-id mismatch path explicitly.
- **Empty public-inputs slice**: `verify(&c, &[], &proof_built_for_one_leaf)` — should return `PublicInputMismatch` because envelope public_inputs has length 1 and call has length 0.
- **Mismatched public_input length** (longer or shorter than envelope): same.
- **Version forgery**: build a `ProofEnvelope` with `version: 99`, postcard-encode, verify — should return `UnsupportedVersion { version: 99 }`. (`UnsupportedVersion` exists in `VerifyError` but no test exercises it.)

These are not P0/P1; they are routine defensive-coding tests that strengthen the surface.

**Recommendation**: add at least the empty-proof, truncated-proof, oversized-proof, and version-forgery cases. They each take three lines.

---

### P2 — `verify` does not assert envelope `version` against `PROOF_ENVELOPE_VERSION` const before commitment-equality

**Where**: `omega-commitment/crates/omega-claim-verifier/src/lib.rs:69-79`.

The verifier checks `version` first, then `commitment`, then `public_inputs`, then per-input `bundle_root_blake3`, then binding words, then the two STARK proofs. The order is fine, but `PROOF_ENVELOPE_VERSION` is declared **twice** — once in the prover (`omega-claim-prover/src/lib.rs:50`) and once in the verifier (`omega-claim-verifier/src/lib.rs:24`), both as `1`. If the prover bumps to `2` and the verifier doesn't, both crates compile but `UnsupportedVersion { version: 2 }` is the result. That's the right behaviour, but the duplication is a synchronisation hazard. **Recommendation**: re-export `PROOF_ENVELOPE_VERSION` from the prover and `pub use` it in the verifier (the prover already exports `PROOF_BINDING_WORD_OFFSET` and `PROOF_BINDING_WORDS` as `pub const` for exactly this reason).

---

### P2 — `Blake3Air` proof in the envelope is ceremonial

**Where**: `omega-commitment/crates/omega-claim-prover/src/lib.rs:293-312`, `omega-commitment/crates/omega-claim-verifier/src/lib.rs:111-117`.

The `blake3_compression_proof` proves that a **synthesised** trace of Blake3 compressions evaluates correctly. The trace is built by `Blake3Air::generate_trace_rows::<Val>(rows, 1)` — the upstream Plonky3 helper that fills a Blake3 trace with **its own internal test vectors**, not with any compression that has anything to do with the actual claim's leaf preimage or path-step inputs. So the `blake3_compression_proof`:

1. Verifies, always, regardless of which witness is being proven.
2. Conveys zero information about the claim.
3. Doubles the proof size.
4. Doubles verify time.

This is consistent with task 2.4 still being open ("the membership AIR and Blake3 compression rows are joined by an actual Plonky3 permutation argument"), but it should be called out: the `blake3_compression_proof` is currently a placeholder slot, not a soundness-relevant artifact. A reader of the verifier would reasonably assume the Blake3 proof is binding the per-step compression inputs to those in the AIR; it is not.

**Recommendation**: add a doc comment to `prove_blake3_compressions` and to `verify_blake3_compression_proof` saying explicitly "v0.1 placeholder; the trace is synthesised by Blake3Air's helper and is not yet bound to the membership trace via permutation argument; the proof exercises the Blake3 compression circuit but does not constrain the claim". This is what task 2.4's implementation note implies, but the code itself is silent.

---

### P2 — `bench_verify_p50.rs` rebuilds the 256-leaf tree three times

**Where**: `omega-commitment/crates/omega-claim-verifier/benches/bench_verify_p50.rs:48-77`.

`proof_fixture(count)` is called three times (once per `count`), and each call runs `MerkleTree::build_v1(SUB_TREE_ID_UTXO, payloads.clone())` from scratch. The tree is identical across calls (same 256 payloads, same sub_tree_id), so two of three builds are wasted. Not a correctness issue; just slow for `cargo bench` warm-up. **Recommendation**: hoist the tree build out of the per-`count` loop. Probably saves 5-15 seconds of bench setup time.

---

## Tasks.md audit (group 3 ticks)

- **3.1 — create crate, same Plonky3 deps as prover**: ticked, real. `Cargo.toml` exists, all 15 `p3-*` crates from the prover are listed via `workspace = true`. ✓
- **3.2 — `verify(commitment, public_inputs, proof) -> Result<(), VerifyError>`**: ticked, real. Function signature exact match (`omega-claim-verifier/src/lib.rs:61-65`). Uses `verify_stark` with the same AIR (`OmegaMembershipAir`). ✓
- **3.3 — no tokio/async/I/O**: ticked, real. Greps for `tokio | async | spawn | mpsc | oneshot` return zero hits in the crate. `#![forbid(unsafe_code)]` on line 1. ✓
- **3.4 — `tests/verifier_round_trip.rs` covers prove→verify accept, tampered, wrong commitment**: ticked, **caveats**.
  - File is `tests/verifier_roundtrip.rs` (no underscore between `round` and `trip`); the task said `verifier_round_trip.rs`. Cosmetic.
  - All three required scenarios are present.
  - The implementation note adds a fifth test (envelope-rewrite). Real, useful.
  - **The tick does not reflect the P0 finding above**: the tests only exercise the *honest* prover, so they cannot catch the membership-AIR soundness gap. Marking 3.4 done without first closing 2.4 (still open) means the verifier round-trip is structurally complete but semantically unsound. Strictly, 3.4 should be re-opened until 2.4 lands.
- **3.5 — `bench_verify_p50.rs` measuring 1/16/256 leaves; assert p50 < 500 ms**: **not** ticked. Implementation note says "exists and compiles, p50 not asserted, measurements not captured." Honest. ✓ for honesty; the task is correctly still open.

Net: 3.1, 3.2, 3.3 ticks correspond to real code. 3.4 tick corresponds to real test code but the soundness claim it implies (verifier rejects everything except valid proofs of valid memberships) is not yet true given the AIR is empty. 3.5 correctly remains open.

---

## Recommendation

**request-changes.**

The PR delivers a clean pure verifier crate, a well-shaped binding-digest fix for the public-values channel, and good purity hygiene (no async, no panics, no unsafe, no I/O). All five P0s from QA-REVIEW.md were design-level and were either closed in earlier PRs (workspace pin, version pins, snapshot wire protocol) or are out of scope for this PR (mock-ledger, libp2p). However, this PR ticks task 3.4 ("verifier round-trip") with tests that exercise only honest-prover paths, while the underlying `OmegaMembershipAir` (introduced in PR #1) constrains nothing about the Merkle path, the leaf preimage, the per-step Blake3 inputs, or the path-to-bundle-root walk. The verifier also does not perform the deterministic gluing the design (`design.md:104`) explicitly relies on for v0.1 soundness ("checked deterministically by the verifier because it fits in one compression block"). Combined, this means a malicious prover can produce an accepting proof for any `(sub_tree_id, leaf_index, nullifier, recipient)` regardless of whether the leaf exists in the tree. This must be closed (or the v0.1 limitation flagged loudly in spec.md and the verifier's doc-comment) before 3.4 can be considered done. The other P1s (fragile tamper test, missing p50 assertion, AIR-vs-walk redundancy) are smaller and can be addressed in follow-ups.

Concrete next actions, in order:

1. Either close P0 by extending `OmegaMembershipAir` with constraints that force `last_row.CURRENT_NODE == commitment.sub_tree_roots_blake3[sub_tree_id - 1]` and per-step Blake3 compression bindings (the right answer; aligns with task 2.4) — **or** add deterministic verifier-side gluing by extending the proof envelope's public-input view with `leaf_payload + merkle_path` so the verifier can run `walk_v1_path` itself (the v0.1 escape valve; matches design.md:104 verbatim) — **or** demote 3.4 to "structurally complete" and document the soundness gap explicitly in the verifier crate's lib doc-comment, in spec.md, and in `cardano-wiki/wiki/log.md`.
2. Strengthen the tampered-proof test (P1) to a proptest over flip indices, asserting `Err(_)` rather than a specific variant.
3. Either run `bench_verify_p50` locally and capture numbers, or add a `verify_p50` integration test with an explicit assertion. (P1; this is what 3.5 actually requires.)
4. Hoist `MerkleTree::build_v1` out of the per-`count` loop in the bench (P2).
5. Re-export `PROOF_ENVELOPE_VERSION` from the prover instead of duplicating (P2).
6. Add doc-comments to `prove_blake3_compressions` and `verify_blake3_compression_proof` clarifying that the Blake3 STARK is currently a placeholder slot until task 2.4 lands (P2).
