# PR #3 Review — `docs: rustdoc convention + doc-pass over four soundness crates`

Branch: `docs/rustdoc-convention-and-pass`
Head: `627a2eddeb80d0abb3833e16da2c33369b6e4d44`
CI: both checks SUCCESS
Scope reviewed: SKILL + doc-pass over `omega-commitment-core`, `omega-claim-tx`, `omega-claim-prover`, `omega-claim-verifier`; workspace lint config; toolchain pin; task 12.7; HTML-tag fix in `omega-utxo-snapshot/src/main.rs`; wiki log entry. (Per the user's note, `omega-mock-ledger/*` files in this PR's diff were ignored — already on main via PR #4.)

## Summary

Recommendation: **approve with two follow-ups** (P2). No P0 or P1 blockers. The doc-pass is technically accurate against the code it describes, the SKILL is internally consistent and actionable, the workspace lint inheritance is correctly wired, and the soundness blocks I spot-checked match the function bodies. Two minor SKILL-internal inaccuracies (worked-example signature drift, leaf_hash_v2 module path in the example) are worth a doc-only follow-up but do not block this merge.

## Strengths

- **Soundness blocks are technically accurate.** The most load-bearing claims I spot-checked all hold:
  - `dual_hash` "drift-detection-not-break-hedge" framing matches `ARCHITECTURE.md:9` verbatim — same audit reframing language, same A1/F004 reference (`hash.rs:73-91` ↔ `ARCHITECTURE.md:9`).
  - `leaf_hash_v2` docstring's preimage layout (`DOMAIN_LEAF || sub_tree_id || canonical_index_be || payload_len_be || payload`) matches the function body byte-for-byte (`tree.rs:97-140`).
  - `node_hash_v2` "position-sensitive" claim is correct (`tree.rs:142-175`); the sibling order is preserved in the buffer build.
  - `EMPTY_INDEX_SENTINEL` padding-leaf-forgery framing matches `MerkleTree::build_v1`'s actual padding logic at `tree.rs:285-287` (which calls `leaf_hash_v2(sub_tree_id, EMPTY_INDEX_SENTINEL, &[])`, not `ZERO_HASH`).
  - `InclusionWitness::verify`'s honest disclosure that the legacy path uses raw `blake3(left || right)` rather than `node_hash_v2` is **correct and important**; line 187 of `witness.rs` confirms `current = blake3_256(&buf)` with no domain tag. The doc explicitly flags this as a documented gap (lines 140-152). This is exactly the kind of honest soundness disclosure the SKILL asks for.
- **`OmegaMembershipAir` 9-item constraint enumeration matches `Air<AB>::eval` line-by-line.** Walked the 9 numbered docstring items (`lib.rs:481-510`) against `eval`'s body (`lib.rs:632-756`):
  1. Boolean flags → `assert_bool` calls at L641-645. ✓
  2. First-row anchoring → `when_first_row` block at L657-660. ✓
  3. Public-input pin → `real_builder` block at L662-674. ✓
  4. Leaf-index bit decomposition → `first_builder` at L676-678. ✓
  5. Direction-bit / left-right swap → L680-697. ✓
  6. No-node-active rows → L698-706. ✓
  7. Last-row root match → `last_builder` at L708-715. ✓
  8. Transitions → L717-747. ✓
  9. Padding rows all-zero → L752-755. ✓
- **`prove_collection`'s `# Soundness` lists every public value the prover commits to** and matches both the `PUBLIC_*_OFFSET` constants (`lib.rs:130-146`) and `validate_envelope_public_values` in the verifier (`lib.rs:315-349`).
- **`verify`'s 4-numbered-condition list maps cleanly to what the verifier checks.** The verifier function (`lib.rs:262-284`) walks: envelope decode, version check, commitment match, public-input match, then `validate_public_inputs` (which enforces conditions 4 — bundle_root + per_sub_tree_root + tree_depth match), then `validate_envelope_public_values` (binding-digest envelope-rewrite check), then `verify_membership_batch` (Plonky3 verify). The 4 conditions in the doc are the witness-existence statement; the runtime checks correspond to those conditions plus the binding-digest envelope-rewrite check called out in the second paragraph. Coherent.
- **All 8 `VerifyError` variants are documented.** Variants in `lib.rs:118-187`: `UnsupportedVersion`, `CommitmentMismatch`, `PublicInputMismatch`, `UnknownSubTree`, `PublicBundleRootMismatch`, `WrongSubTreeRoot`, `DepthMismatch`, `InvalidProof`. All 8 appear in the `# Errors` block on `verify` (`lib.rs:198-219`). ✓
- **Workspace lint inheritance is correctly wired.** `omega-commitment/Cargo.toml:62-71` declares `[workspace.lints.rust] missing_docs = "warn"` and `[workspace.lints.rustdoc] broken_intra_doc_links = "warn"`, `missing_crate_level_docs = "warn"`. All four crates' `Cargo.toml` files use `[lints] workspace = true` (no per-crate `[lints.rust]` blocks).
- **Toolchain pin is correct and well-justified.** `omega-commitment/rust-toolchain.toml` pins `channel = "1.95.0"` (concrete version, not "stable"), with a comment explaining the CI lockstep (avoids drift like `result_large_err` from 1.95 surfacing only on PR).
- **Task 12.7 is in `tasks.md:138`** and mandates SKILL compliance, missing-docs warnings escalated to errors via `-D warnings`, and `cargo doc --workspace --no-deps --document-private-items` exits 0. Re-checks before every cargo-build-validation gate. Comprehensive.
- **`#[doc(hidden)]` is correctly applied.** `serde_helpers.rs:5` has `#![doc(hidden)]` at the module level plus per-item `#[doc(hidden)]` on every adapter and `serialize`/`deserialize` function. `prove_collection_with_trace_tamper` (`prover/src/lib.rs:961-962`) and `TraceTamper` enum (`lib.rs:303-314`) are both `#[doc(hidden)]` with usage notes. Correct.
- **HTML-tag fix is in the diff** at `omega-utxo-snapshot/src/main.rs:55`: ``<numeric magic>`` → `` `<numeric magic>` ``. Confirmed via `git show b90168b -- omega-commitment/crates/omega-utxo-snapshot/src/main.rs`.
- **No `#![allow(missing_docs)]` on any library file.** Grep confirms the attribute appears only on test/bench files (7 total: `bench_verify_p50.rs`, `bench_prove_p50.rs`, `verifier_roundtrip.rs`, `prover_smoke.rs`, `tree.rs` bench, `soundness_negative.rs`, `claim_tx_cbor.rs`). All four library `lib.rs` files use `#![warn(missing_docs)]`. Test infrastructure was not compromised as a doc-warning shortcut.
- **No AI-tells in the new prose.** Grep against the SKILL, the four lib.rs docstrings, hash.rs, tree.rs, witness.rs, the prover's `prove_collection`/`OmegaMembershipAir` docs, the verifier's `verify` doc, and the wiki log entry returns no matches for "vibrant", "tapestry", "delve", "underscore", "showcase", "pivotal", "leverage", "harness the power of". The single occurrence in the SKILL itself (line 266) is the forbidden-list entry. Voice matches the existing repo (terse, technically dense, no marketing prose).
- **Cargo.lock additions are consistent with the omega-mock-ledger merge.** Spot-checked the package list; all added deps (openraft, rusqlite, hashbrown, ahash, pallas-* family, tokio family, anyerror) are workspace-declared in `omega-commitment/Cargo.toml:21-40`. Nothing crept in outside the `[workspace.dependencies]` set.
- **Wiki log entry** in `cardano-wiki/wiki/log.md` is append-only, dated 2026-05-04, and accurately summarises the four-agent doc-pass plus the workspace lint consolidation. Voice matches the existing log.

## Findings

### P0 (must fix before merge)

None.

### P1 (should fix before merge)

None.

### P2 (suggestions / follow-ups)

#### P2-1 — SKILL worked-example `verify` signature drift

`skills/local/omega-rustdoc-style/SKILL.md:155-161` shows the worked example signature:

```rust
pub fn verify(
    commitment: &OmegaCommitment,
    public_inputs: &ClaimPublicInputs,
    proof: &ProofBytes,
) -> Result<(), VerifyError>
```

The actual signature in `omega-claim-verifier/src/lib.rs:262-266` is:

```rust
pub fn verify(
    commitment: &OmegaCommitment,
    public_inputs: &[ClaimPublicInputs],   // slice, not single
    proof: &ProofBytes,
) -> Result<(), VerifyError>
```

The SKILL's framing is "the actual `verify` function in `omega-claim-verifier`" (line 100) — i.e. the example is meant to be the ground truth. Two related drifts:

1. `public_inputs` is `&[ClaimPublicInputs]` (batch), not `&ClaimPublicInputs` (singleton).
2. The SKILL's `# Errors` block lists 4 variants (`CommitmentMismatch`, `WrongSubTreeRoot`, `DepthMismatch`, `InvalidProof`); the actual function has 8 variants and the actual docstring enumerates all 8.

The SKILL is markdown, not a Rust doctest — `cargo doc` doesn't compile this — so it does not break CI. But future readers using the SKILL as a template will copy the wrong signature. Fix: update the SKILL's worked example to match the actual `verify` (slice + 8 variants), or add a one-line note that the example is illustrative and may lag the implementation.

#### P2-2 — SKILL example for `leaf_hash_v2` references the wrong module path

`skills/local/omega-rustdoc-style/SKILL.md:41` shows:

```rust
use omega_commitment_core::hash::leaf_hash_v2;
```

The actual location is `omega_commitment_core::tree::leaf_hash_v2` (`tree.rs:132`). Same disposition as P2-1 — the SKILL is markdown, no compilation, but this misleads future authors. Fix: change `hash::` to `tree::` in the SKILL example.

#### P2-3 — SKILL's "Tier of trust" list lags task 12.7's list

The SKILL (line 189) names three soundness-bearing crates: `omega-commitment-core`, `omega-claim-prover`, `omega-claim-verifier`. Task 12.7 names seven: those three plus `omega-claim-tx`, `omega-mock-ledger`, `omega-toy-consensus`, `omega-network`. The discrepancy is intentional (task 12.7 looks forward at the harness as it grows), but a one-line note in the SKILL pointing to task 12.7 for the canonical list would prevent future ambiguity.

#### P2-4 — `omega-claim-prover/src/lib.rs` attribute placement is stylistically inconsistent with the other three crates

Three of the four crates put inner attributes after the crate-level docstring. `omega-claim-prover/src/lib.rs` puts `#![forbid(unsafe_code)]` at line 1 (BEFORE the `//!` docstring) and the rest of the inner attributes at lines 66-68 (AFTER the docstring). Compiles fine; just a parallel-agent inconsistency. Fix: move line 1's `#![forbid(unsafe_code)]` to sit alongside lines 66-68 after the docstring, matching the other three crates.

## Spec checklist (A-J from the brief)

| Item | Status | Notes |
|---|---|---|
| A. SKILL accuracy + worked-example consistency | Mostly clean | P2-1 (signature drift) + P2-2 (module path) |
| B. Soundness-block accuracy spot-checks | Clean | `dual_hash`, `leaf_hash_v2`, `node_hash_v2`, `EMPTY_INDEX_SENTINEL`, `InclusionWitness::verify`, `OmegaMembershipAir` 9-item, `prove_collection`, `verify` all match code |
| C. Workspace lint inheritance | Clean | `[workspace.lints.{rust,rustdoc}]` declared; all four crates `[lints] workspace = true` |
| D. Toolchain pin (1.95.0 concrete) | Clean | Concrete version, comment explaining CI lockstep |
| E. Task 12.7 | Clean | Present, mandates SKILL compliance, escalates to `-D warnings` |
| F. AI-tells in new prose | Clean | No matches across SKILL, four crate docstrings, wiki log |
| G. `#[doc(hidden)]` correctness | Clean | `serde_helpers` module + items hidden; `prove_collection_with_trace_tamper` + `TraceTamper` hidden |
| H. `omega-utxo-snapshot/src/main.rs` HTML-tag fix | Clean | Confirmed in diff at line 55 |
| I. Tests still pass | Clean | CI green; no library file silenced with `allow(missing_docs)`; only test/bench files use the allow |
| J. Cargo.lock sanity | Clean | All adds are workspace-declared deps for omega-mock-ledger (already on main via PR #4) |

## Recommendation

**Approve.** No P0 or P1 blockers; the doc-pass is technically accurate and the workspace lint plumbing is correct. P2-1 through P2-4 are doc-only and SKILL-internal — fix in a one-commit follow-up after merge so this PR doesn't bounce on cosmetic SKILL drift. The main load-bearing soundness work (the docstrings on `dual_hash`, `leaf_hash_v2`, `node_hash_v2`, `MerkleTree::build_v1`, `InclusionWitness::verify`, `OmegaMembershipAir`, `prove_collection`, `verify`) is accurate and matches the code.
