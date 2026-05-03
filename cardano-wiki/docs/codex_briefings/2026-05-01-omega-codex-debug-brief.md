# Codex Long-Running Debug Brief — Ouroboros Omega Program

> ⚠️ **PARTIALLY SUPERSEDED 2026-05-03** — sections describing v1.0 ingestion (Daedalus, single-LedgerState-CBOR dump path, `cardano-cli query ledger-state --output-cbor`, Conway LedgerState CBOR parsing as the unified input) are obsolete. The current v1.0 model is a **two-stream pipeline**: `cardano-cli conway query ledger-state` JSON for stake+governance, and the new `omega-utxo-snapshot` binary (pallas-network LSQ client) for utxo+token-policy+script. Read `docs/codex_briefings/2026-05-03-omega-codex-pipeline-update-brief.md` BEFORE acting on anything in this file. The v0.9.1 baseline (89 commits, 248 tests, golden vectors, leaf encodings, sub-tree roots, bundle assembly) remains valid and is unchanged by the pipeline pivot.
>
> **Read order: full document, then return to specific sections as needed during your work.**
> **Estimated reading time: 30–45 minutes. Estimated work time: 4–25 hours of autonomous debug + review.**

This document is a **handoff brief** for OpenAI Codex (GPT-5.5+) to perform a long-running, autonomous code review and debug pass on the Ouroboros Omega program. The program was built incrementally from v0.1.0 to v0.9.0 by Claude (Anthropic) over a single multi-hour session; v1.0 is a planned next step. Your job is to **pressure-test everything**.

**2026-05-02 status update:** v0.9.1 supersedes the original v0.9.0 environment gate after the Codex audit fixes. The current expected baseline is 89 commits and 248 passing tests. The original 236-test count in the first handoff was stale; Cargo discovered 228 tests at v0.9.0 before the v0.9.1 fixes added coverage.

---

## Section 1 — Project Identity

**Program:** Ouroboros Omega — a clean-slate post-quantum fork design for Cardano with ZK continuity to all prior eras. The code in this repo is the **commitment-tooling track (T1)** of a 12-track program. T1 produces the canonical Ω-Commitment that captures the entire pre-fork Cardano state.

**Repo:** `/home/hoskinson/omega-commitment` (git repo; 89 commits as of v0.9.1; ~10,000 lines of Rust). Standalone, not a fork.

**What this code IS:**
- A four-crate Rust workspace producing per-sub-tree commitments (UTXO, header, tx-index, token-policy, script, stake, governance) and a dual-track bundle root (Blake2b + SHA3) per the resolved 2026-05-01 dual-hash decision.
- Synthetic-CBOR ingestion paths for 5 of 7 sub-trees (the LedgerState-derivable ones).
- 248 passing tests, including pinned golden vectors at three layers.
- Clippy + rustfmt clean; CI workflow committed.

**What this code is NOT (yet):**
- Real Cardano mainnet ingestion (planned v1.0 — Daedalus 8.0 + Mithril fast-bootstrap).
- Plonky3 claim circuits (separate track T2).
- Any cryptographic attestation (Mithril-PQ, recursive STARK, CIP-1694 voting machinery — separate tracks).

**Strategic intent:** Prove the architecture for Ouroboros Omega's commitment layer. Lock golden vectors that any future change must preserve. Make every encoding, ordering, and aggregation decision concrete enough that Plonky3 circuit authors and CIP-1694 governance ratifiers can build against a stable target.

**Hard constraints (do not change without recorded justification):**
1. **PQ-only crypto** — Blake2b-256 + SHA3-256. Zero curve operations anywhere in the workspace.
2. **Plonky3-friendly tree layout** — binary, fixed-arity, sorted-padded to power of two.
3. **Selective dual-track at bundle layer** — per-leaf and per-sub-tree are Blake2b-only; bundle root is the tuple `(blake2b_bundle, sha3_bundle)`. Per the 2026-05-01 decision: `docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md`.
4. **Lazy/pull-based migration** — the chain semantics being modeled.
5. **Golden vectors are sacred** — any test failure in `tests/golden_*.rs` is a P0 issue. Do NOT update the pinned hex values without recording the encoding change as a SemVer-major decision.

---

## Section 2 — Current System State

### What's built and shipping

| Version | Released | What |
|---|---|---|
| v0.1.0 | 2026-05-01 | omega-commitment-core: hash module, UTXO leaf encoding, Plonky3-friendly Merkle tree, inclusion witness; CLI commit subcommand |
| v0.2.0 | 2026-05-01 | Header sub-tree (block header chain) |
| v0.3.0 | 2026-05-01 | Tx-index sub-tree |
| v0.3.1 | 2026-05-01 | Hardening sprint (path traversal, atomic write, dispatcher refactor, CI) |
| v0.4.0 | 2026-05-01 | Token policies sub-tree |
| v0.5.0 | 2026-05-01 | Script registry sub-tree |
| v0.6.0 | 2026-05-01 | Stake + governance sub-trees (all 7 commitment layers complete) |
| v0.7.0 | 2026-05-01 | Bundle assembly tool with dual-track aggregation |
| v0.8.0 | 2026-05-01 | UTXO ingestion from synthetic CBOR; golden vectors framework |
| v0.9.0 | 2026-05-01 | Token-policy / script / stake / governance ingestion from extended fixtures |
| v0.9.1 | 2026-05-02 | Codex audit fixes: ingest CLI outputs, UTXO native assets, strict CBOR end checks, re-pinned affected ingestion goldens |

### What's planned

- **v1.0** — Real mainnet ingestion via Daedalus 8.0 + cardano-cli LedgerState dumps. Plan exists at `/home/hoskinson/cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`. NOT YET IMPLEMENTED.

### Test count breakdown (v0.9.1 final state)

248 passing, 0 failed, 0 ignored. Per-suite:
- `omega-commitment-core` unit tests: 108 (covers all 7 sub-tree leaf encoders, hash, tree, witness, serde_helpers)
- `omega-commitment-core` golden_vectors: 8 (7 per-sub-tree roots + 1 witness round-trip)
- `omega-commitment-cli` unit + smoke: 11
- `omega-commitment-bundle` unit + integration + golden: 25
- Per-sub-tree integration tests: 26 (utxo + header + tx-index + token-policy + script + stake + governance)
- `omega-commitment-ingest` unit tests: 47
- `omega-commitment-ingest` CLI output integration: 4
- Per-sub-tree ingestion integration: 11
- qa_pipeline: 2
- golden_ingest: 6

### Pinned golden vectors

**Per-sub-tree roots** (synthetic fixtures, `omega-commitment-core/tests/golden_vectors.rs`):
- UTXO: `74be699a17928cfae6a9301b96e033c5b75ccc841b2eeb4d3e9ab4484694c044`
- Header: `ed2eaedffc3833afbe0d7727f66c1b824bec77139f9e0a965b81e30dd349f1de`
- Tx-index: `76fc602782a80bb5e425bf22d32cdcf0ababa46e9129c76d470b990fb62fe6c1`
- Token-policy: `c8d27987a53df992eebc37a6b1ad4549009cf5916618d869161b7e659a3a3c2a`
- Script: `92cc8f368cf40d6d00ab1524d1d5715786f563b2cfb8756dc29ffab41fd74bab`
- Stake: `b903889b884b4e33dfd3a2c7c3736cd16100cdcd0328d91874cfab473e196322`
- Governance: `cee7d743ecd1367142aab991e55e67f0a40835eecc0661ccec6f9617b99734b4`

**Bundle root tuple** (synthetic fixtures, `omega-commitment-bundle/tests/golden_bundle.rs`):
- `blake2b_bundle_root = ee308b538b26e6d87b115ffac5676f39d0059f75dd8c79221b6b80186aebd712`
- `sha3_bundle_root    = 189826cfa4be57615db0ac4e5fab2602291921d54365198847927e5461638b77`

**Ingestion-layer roots** (CBOR fixtures, `omega-commitment-ingest/tests/golden_ingest.rs`):
- UTXO: `3db453610cddde4f799a7bd5e5757fe7b66c71510c2f55d10d1a8c577b94f6f7`
- Token-policy: `2b093effe91ecb6d1dbae52e566914e629dd37bc3e1f76457087232790593157`
- Script: `d4362524462727386a3f6892e1cc07b813b97ad2e8b19d56c0c31e4c703df381`
- Stake: `56d68a45319ec728ff99d8510f02d20c17c6d88335caf9f93fedeb4502997f85`
- Governance: `bee53b24965867c9fb877eccb925695d65cf15485c8000cb08ee64218700317d`

**Hybrid bundle root tuple** (5 from CBOR + 2 from JSON, `golden_ingest.rs`):
- `blake2b_bundle_root = 18d6a6a299849d0c832f5f3094037099f2ad7997f05b1a471bb49b9cbb714a2c`
- `sha3_bundle_root    = 7831f4008d79f9211c89424d5c0ddfb16438b0a9f2c6a45c30d623a7dae2b3e3`

These 18 hashes are your regression net. Any change that causes any of them to drift is either a bug or a deliberate breaking change requiring documented justification.

### What's known to be partial / scaffolded

- `omega-commitment-ingest` has `pallas-primitives` and `pallas-traverse` declared as deps but only `pallas-codec::minicbor::Decoder` is actually used (v1.0 fixes).
- `first_issuance_slot` (token-policy) and `deployment_slot` (script) are pinned to 0 in synthetic fixtures (real values require chain history).
- `scripts/download_snapshot.sh` exists but is human-invoked only; v0.9.0 cannot parse a real Mithril snapshot.
- `pallas-primitives` and `pallas-traverse` deps unused in v0.9.0 (cleanup deferred to v1.0).

---

## Section 3 — Architecture & Technical Map

### Workspace layout

```
omega-commitment/
├── Cargo.toml                           Workspace manifest (4 members)
├── README.md                            Per-version release notes
├── rust-toolchain.toml                  Pin: stable + clippy + rustfmt
├── .cargo/config.toml                   Aliases: cargo lint, cargo fmt-check
├── .github/workflows/ci.yml             CI: build + test + clippy -D warnings + fmt-check
├── scripts/
│   └── download_snapshot.sh             Human-invoked Mithril testnet downloader
├── crates/
│   ├── omega-commitment-core/           Library: hash, leaf encoders, tree, witness
│   ├── omega-commitment-cli/            Binary: omega-commitment commit
│   ├── omega-commitment-bundle/         Library + binary: omega-bundle assemble/verify
│   └── omega-commitment-ingest/         Library + binary: omega-ingest <subcommand>
└── docs/                                Currently empty in repo (plans live in cardano-wiki)
```

### `omega-commitment-core` modules

| Module | Responsibility | Public API |
|---|---|---|
| `hash` | PQ-only hashing primitives | `blake2b_256`, `sha3_256`, `dual_hash`, `Hash = [u8; 32]` |
| `serde_helpers` | Hex serde adapters | `hex_vec_hash` for `Vec<Hash>`, `opt_hex` for `Option<[u8;32]>` |
| `tree` | Plonky3-friendly Merkle tree | `MerkleTree::build`, `root`, `depth`, `leaf_count`, `leaves`, `layers` (private fields, accessor methods) |
| `witness` | Inclusion witness | `InclusionWitness::build`, `build_at_index`, `verify` (with bounds-guard) |
| `utxo_leaf` | UTXO encoding | `Utxo`, `LeafError` (`#[non_exhaustive]`), `encode -> Vec<u8>`, `leaf_hash -> Result<Hash, LeafError>` |
| `header_leaf` | Block header encoding | `BlockHeader` (4 fields, 80-byte fixed-width), `validate_chain_links` |
| `tx_index_leaf` | Transaction index encoding | `TxIndexEntry` (4 fields, 76-byte fixed-width), `validate_tx_uniqueness` |
| `token_policy_leaf` | Native token policy encoding | `TokenPolicy` (3 fields, 52-byte fixed-width with 28-byte policy_id), `PolicyId = [u8;28]`, `validate_policy_id_uniqueness` |
| `script_registry_leaf` | Plutus script entry encoding | `ScriptEntry` (4 fields, 41-byte fixed-width with 28-byte script_hash + u8 language byte), `validate_script_hash_uniqueness` |
| `stake_state_leaf` | Per-credential stake state encoding | `StakeEntry` (5 fields, 93-byte fixed-width), `CredentialHash = [u8;28]`, `validate_stake_credential_uniqueness` |
| `governance_state_leaf` | Heterogeneous governance facts | `GovernanceFact` (4 fields, 57-byte fixed-width with kind discriminant: 0=treasury, 1=CC seat, 2=ratified, 3=in-flight), `validate_governance_keys_unique_per_kind` |

### `omega-commitment-cli`

Binary `omega-commitment`, single subcommand `commit` with `--sub-tree {utxo,header,tx-index,token-policy,script,stake,governance}` (now `#[non_exhaustive]`). Reads JSON input matching the per-sub-tree shape, builds the tree, emits `commitment.json` (root + metadata + input_digest) and per-leaf witness JSONs. Path-canonicalized + size-capped + atomic-write.

### `omega-commitment-bundle`

Library + binary `omega-bundle`. Subcommands `assemble` and `verify`. Reads 7 per-sub-tree input files, recomputes both Blake2b and SHA3 sub-tree roots, aggregates into the canonical `(blake2b_bundle_root, sha3_bundle_root)` tuple per the 2026-05-01 dual-hash decision. Bundle JSON schema v1.

### `omega-commitment-ingest`

Library + binary `omega-ingest`. v0.9.0 subcommands: `utxo`, `token-policy`, `script`, `stake`, `governance` (all currently parse simplified-CBOR fixtures only). v1.0 adds `--format {auto,synthetic,mainnet}` and per-sub-tree mainnet parsers reading real Conway LedgerState CBOR.

### Cryptographic conventions

- All multi-byte integers: **big-endian**.
- u128 over the wire (governance facts, JSON serialization): 16-byte big-endian bytestring (CBOR has no native u128).
- Cardano-native hashes (policy_id, script_hash, stake credential, pool id, drep id): **28 bytes** (Blake2b-224). Internal Merkle node hashes and leaf hashes: **32 bytes** (Blake2b-256). The asymmetry is intentional and documented.
- Tree padding leaf: `ZERO_HASH = [0u8; 32]`.

---

## Section 4 — Recent Work With Reasoning

### Why this commitment shape (vs. a single flat tree)?

**Decision (locked in spec §7):** seven sub-trees, each with its own Merkle root, then a bundle root over the seven. Reason: each sub-tree has different update cadence and proof patterns. UTxO claims need O(log n) inclusion proofs; governance claims need O(log m) where m is much smaller. A flat tree mixing all data would force every claim to pay the cost of the largest sub-tree.

### Why dual-track only at the bundle layer?

**Decision (resolved 2026-05-01):** see `docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md`. Per-leaf and per-sub-tree roots are Blake2b-only. Only the bundle root is published as a `(blake2b_bundle_root, sha3_bundle_root)` tuple. Reason: doubling per-leaf hashing would double Plonky3 circuit work (millions of claims) for marginal security gain. The legitimacy boundary lives at the bundle layer (Mithril-PQ + recursive STARK + CIP-1694 ratification) — that's where dual-track attests. Per-claim circuits verify single-track Blake2b paths against the Blake2b half of the bundle tuple.

### Why hand-crafted CBOR fixtures (vs. real mainnet) for v0.9.0?

A real Mithril mainnet snapshot is a multi-GB Cardano node DB (immutable + ledger), not a single LedgerState CBOR file. Parsing one requires `cardano-node` to load it (multi-day initial sync), pallas-traverse Conway support that's currently partial, or a REST indexer like Koios. The hand-crafted simplified-CBOR fixtures in v0.8.0 / v0.9.0 are deterministic, in-tree, CI-friendly, and exercise every code path. Real-mainnet ingestion is the v1.0 plan.

### Why the workspace synchronizes versions

All four crates bump together (workspace-synchronized). One workspace, one version. Avoids dependency-version juggling and makes "which version is installed" easy to answer.

### Why `#[non_exhaustive]` on `LeafError` and `SubTree`

Pre-empt SemVer breaks. Both enums grow (e.g., a new sub-tree variant). Marking them `#[non_exhaustive]` means downstream pattern matchers must include `_ =>` arms today; adding variants tomorrow doesn't break their builds.

### Why `Cargo.lock` is gitignored

Repo's `.gitignore` excludes it. Standard for libraries (binaries usually commit it). Workspace ships both libs and a binary; the v0.1.0 final reviewer flagged this as appropriate either way for our workspace style. Document if you want to revisit.

---

## Section 5 — Known Risks and Open Questions

### High-priority items for your debug pass

1. **Determinism across platforms.** All encoding uses `to_be_bytes()` and `BTreeMap` for sorting. Run the test suite on a non-x86 platform (ARM Mac, RISC-V if available) and confirm golden vectors match. If they don't, the encoding has a non-determinism bug.

2. **CBOR parser robustness.** `cbor.rs` helpers were written for hand-crafted fixtures. Probe for issues with malformed CBOR: indefinite-length arrays, extremely large length prefixes, deeply-nested structures, allocator-exhaustion attacks. The CBOR fixtures we ship are trusted; what happens with adversarial input?

3. **Padding semantics.** `tree.rs` pads to next power of two with `ZERO_HASH = [0u8; 32]`. Audit: can an attacker construct a leaf set where one of the leaves naturally hashes to `ZERO_HASH`? (Probability is negligible — Blake2b collision with all-zeros — but document it.)

4. **Witness verification bounds-guard.** `witness.rs::verify` rejects depth ≥ 32 and `leaf_index >= 2^depth`. Audit the exact arithmetic for off-by-ones at depth boundaries (depth = 0 special case, depth = 31 maximum).

5. **u128 serialization edge cases.** governance values flow through CBOR as 16-byte bytestrings. Audit that `u128::MAX` round-trips cleanly through serde + CBOR + back. There IS a fixture exercise for `u128::MAX - 1` but verify the exact-MAX case.

6. **Bundle root canonical ordering.** `omega-commitment-bundle::bundle::aggregate_*` concatenates sub-tree roots in the order defined by `sub_tree_id::ALL`. If anything reorders this list, the bundle root drifts. There's a `golden_bundle_canonical_order_unchanged` test — confirm it's actually checking the order, not just the set.

7. **Token-policy `total_supply_at_h` overflow.** v0.9.0 uses `u128::checked_add` correctly in `ingest_token_policies`. Audit for any other path (e.g., a future pallas-traverse extension) where overflow could silently truncate.

8. **JSON round-trip stability.** Many tests round-trip via `serde_json`. Confirm field ordering, whitespace, and number representation are stable across serde versions. A `serde_json` MSRV bump could change output.

### Medium-priority items

9. **`memmap2` portability.** v1.0 plan uses memory-mapped files for streaming UTXO parsing. Confirm `memmap2` works on the user's actual platform (macOS, Linux, possibly Windows). The current workspace has not yet pulled `memmap2`; v1.0 adds it.

10. **Pallas API drift.** `pallas-codec` 0.30.2 is pinned (loosely) via `"0.30"`. Check whether 0.30.x has had patch releases that change minicbor re-exports. The `Decoder` import path in `cbor.rs` may break.

11. **CI workflow effectiveness.** `.github/workflows/ci.yml` runs build + test + clippy -D warnings + fmt-check. Does it actually run? (Repo is local-only; no GitHub remote configured.) If the user pushes to a real repo, validate that the workflow triggers correctly.

12. **Validator helpers being optional.** Each sub-tree's `validate_*_uniqueness` returns `Some(idx)` on first duplicate. The CLI never invokes them — commitment generation accepts duplicates silently. Audit whether this is the intended UX (the spec says "optional sanity helper").

### Low-priority items / discussion

13. **`first_issuance_slot` / `deployment_slot` pinned to 0.** Documented limitation. v1.1 may add accuracy via Koios. Audit whether downstream consumers (Plonky3 circuit authors) treat 0 as "unset" or "actually slot 0" — could cause confusion.

14. **Bundle does not carry attestations.** Mithril-PQ signatures, recursive STARK proofs, and CIP-1694 ratification are separate artifacts. Audit whether the bundle's `schema_version` field gives downstream tools enough info to validate against the right schema.

15. **`scripts/download_snapshot.sh` Mithril verification.** Script downloads but doesn't verify the Mithril certificate. Flagged as future hardening. Audit whether this is acceptable for the script's stated purpose (manual human inspection).

16. **`omega-commitment` CLI doesn't validate input integrity.** If `--input` is corrupted JSON, what happens? Does the user get a meaningful error message?

---

## Section 6 — Do Not Touch (Without Recorded Justification)

These are load-bearing and changing them breaks downstream consumers:

1. **The 18 pinned golden vectors** (Section 2). Drift = bug or breaking change.
2. **Per-sub-tree leaf encoding byte layouts** (e.g., UTXO is `tx_id ‖ output_index BE ‖ address_hash ‖ value_lovelace BE ‖ asset_count BE ‖ assets ‖ datum_marker`). Documented in each `*_leaf.rs` module docstring. Changing layout = SemVer-major.
3. **`SubTreeId::ALL` canonical order** in `omega-commitment-bundle`. Changing order changes the bundle root.
4. **`hash::Hash = [u8; 32]` type alias.** Used uniformly across the workspace.
5. **`Cargo.toml` workspace structure.** All four crates bump together.
6. **The dual-hash decision (selective dual-track at bundle layer only).** Changing this is a program-level decision requiring a new decision document.
7. **`#[non_exhaustive]` on `LeafError` and `SubTree`.** Removing these breaks SemVer guarantees.
8. **CI configuration.** `cargo lint = clippy --workspace --all-targets -- -D warnings` is the standard. Don't loosen.

---

## Section 7 — Suggested Debug Workflow

Estimated time: 4–25 hours depending on depth. Use durable Markdown notes in `docs/codex_findings/` for each finding so a future session can pick up where you left off.

### Phase 0 (30 min): Orient

1. Read this brief end-to-end.
2. Skim `cardano-wiki/wiki/log.md` for a chronological summary of how the codebase grew.
3. `cd /home/hoskinson/omega-commitment && cargo test --workspace 2>&1 | tail` — verify 248 tests pass on your machine.
4. `git log --oneline | wc -l` — confirm 89 commits.
5. Skim each `Cargo.toml`. Confirm versions all at 0.9.1.

### Phase 1 (2–4 hours): Static analysis

1. **Audit each leaf encoder's byte layout against its docstring.** For each `*_leaf.rs` module:
   - Read the `//!` docstring describing the byte layout.
   - Read the `encode` fn implementation.
   - Confirm the bytes written match the docstring exactly. Look for off-by-ones.
2. **Audit `tree.rs::build` for invariants.** Confirm: (a) padding to power of two is always exact; (b) `H(left ‖ right)` order is consistent; (c) `leaves[0] == layers[0]`.
3. **Audit `witness.rs::verify` bounds-guard arithmetic.** Walk through depth=0, depth=1, depth=2, depth=31. Confirm leaf_index limits are correct.
4. **Audit `bundle.rs::aggregate_*` for ordering.** Confirm sub-tree roots are concatenated in `SubTreeId::ALL` order; confirm the test that pins this is actually checking what it says.
5. Document each finding with `severity: critical|high|medium|low`, `file:line`, `proposed fix`.

### Phase 2 (2–6 hours): Dynamic analysis

1. **Fuzz the CBOR parser.** Write `proptest`-style tests that feed random CBOR to each `ingest_*` function. Are there inputs that cause panics, OOM, or infinite loops? Document any.
2. **Fuzz leaf encoding.** Generate random `Utxo`, `BlockHeader`, etc., and confirm `encode(decode_via_serde(serialize(x))) == encode(x)`. Catch any non-determinism in serde.
3. **Cross-platform check.** If you have access to a non-x86 machine (e.g., ARM via Codex's sandbox), run the full test suite there. Confirm golden vectors match.
4. **Memory profiling.** Use `valgrind --tool=massif` or similar on the CLI against the largest synthetic fixture. Confirm RAM usage is reasonable.
5. **Concurrency check.** None of the libraries are explicitly thread-safe. Audit for any shared mutable state. Document if any.

### Phase 3 (2–8 hours): Spec/code drift

1. **Re-read the design spec** at `/home/hoskinson/cardano-wiki/docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md`.
2. **For each spec requirement (§6, §7, §9), find the implementation.** Document any spec/code drift.
3. **Re-read the dual-hash decision** at `/home/hoskinson/cardano-wiki/docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md`.
4. **Audit the decision's three commitments** (per-leaf Blake2b only, per-sub-tree Blake2b only, bundle dual-track) against the actual code. Any deviation is a P0 issue.

### Phase 4 (1–4 hours): Recommendations

1. Compile findings into `docs/codex_findings/2026-MM-DD-omega-codex-review.md`.
2. Use this template:
   ```markdown
   # Codex Review — Omega v0.9.0

   ## Summary
   <One paragraph: did the architecture survive contact?>

   ## Findings (by severity)

   ### Critical
   <bug + reproduction + proposed fix>

   ### High
   ...

   ### Medium
   ...

   ### Low / Suggestions
   ...

   ## What works well
   <positive findings — surprisingly clean code, good test discipline, etc.>

   ## v1.0 readiness assessment
   <can the v1.0 plan proceed against this codebase, or are there blockers?>
   ```
3. Commit your findings to a branch (`codex-review-YYYY-MM-DD`) and push.

### Phase 5 (1–3 hours): Independent reproduction

For the highest-priority findings: write tests that REPRODUCE the bug and FAIL on master, PASS after your proposed fix. Don't just describe — show.

---

## Section 8 — Hard Limits and Confidence Flags

**Do not, without explicit user authorization:**
- Push to any remote (`git push`)
- Create or merge pull requests
- Modify the `main` branch directly — work on a `codex-review-YYYY-MM-DD` branch
- Install new dependencies that change the `Cargo.lock` shape
- Run any script in `scripts/` (they trigger network and disk operations)
- Touch `docs/superpowers/decisions/*.md` (these are program-level decisions)

**Confidence flags for sections of this brief:**

| Section | Confidence | Notes |
|---|---|---|
| Project Identity | HIGH | Authoritative |
| Current System State | HIGH | Verified via `cargo test --workspace` |
| Architecture Map | HIGH | Direct from code |
| Recent Work | HIGH | Sourced from wiki log.md |
| Known Risks | MEDIUM | Educated guesses; some may be non-issues, others may miss real bugs |
| Do Not Touch | HIGH | These are load-bearing |
| Debug Workflow | MEDIUM | Suggested approach; adapt as you discover what matters |
| Hard Limits | HIGH | Authoritative |

---

## Section 9 — Useful Files

If you only have time to read 10 files, these are the most informative:

1. `/home/hoskinson/cardano-wiki/docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md` — the design spec
2. `/home/hoskinson/cardano-wiki/docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md` — the dual-hash decision
3. `/home/hoskinson/cardano-wiki/wiki/log.md` — chronological build log
4. `/home/hoskinson/omega-commitment/README.md` — per-version release notes
5. `/home/hoskinson/omega-commitment/crates/omega-commitment-core/src/tree.rs` — the Merkle tree (load-bearing)
6. `/home/hoskinson/omega-commitment/crates/omega-commitment-core/src/witness.rs` — witness build + verify (security-critical)
7. `/home/hoskinson/omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs` — most complex leaf encoder (variable-length assets)
8. `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/src/bundle.rs` — bundle aggregation
9. `/home/hoskinson/omega-commitment/crates/omega-commitment-core/tests/golden_vectors.rs` — the regression net
10. `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/tests/golden_bundle.rs` — the bundle regression net

If you have time for 30 more files, prioritize the other `*_leaf.rs` modules and their corresponding integration tests.

---

## Section 10 — Output Format

Your final deliverable is a single markdown document in `docs/codex_findings/2026-MM-DD-omega-codex-review.md`. Structure per Phase 4 above. Each finding must include:

- **Severity** (Critical / High / Medium / Low)
- **File:line** (exact location)
- **Reproduction** (exact commands to demonstrate the bug)
- **Proposed fix** (concrete diff or pseudocode)
- **Test pinning** (a test that fails before fix, passes after)

Commit findings to `codex-review-YYYY-MM-DD` branch. Do not push. The user will review and merge selectively.

---

## End of brief

Good hunting. The codebase is small, well-tested, and built on a tight set of decisions — but eight versions of feature growth in a single session means there's almost certainly something that drifted. Find it.
