# Decision: Dual-track Shadow Hash for Ouroboros Omega

**Status:** Recorded
**Date:** 2026-05-01
**Decision-maker:** Charles Hoskinson (with Claude as sparring partner)
**Supersedes:** "Open question — dual-track shadow hash" (v0.1.0 / v0.2.0 / v0.3.x README)

---

## 1. The question

The Ouroboros Omega design spec (`docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md` §6) prescribes dual-track hashing — Blake2b-256 as primary, SHA3-256 as shadow — and says:

> "Both must be checked by verifiers; divergence means a bug or tampering."

Through v0.4.0, the implementation publishes ONLY the Blake2b-256 root in `commitment.json`. SHA3 lives in `hash::dual_hash` as a primitive but is never committed to. This left a real spec-vs-implementation gap that needed to be closed before track T2 (Plonky3 circuits) can lock its verifier shape.

**Three candidate options** were on the table:

- **Option 1 — Strong dual-track.** Every commitment publishes a tuple `(blake2b_root, sha3_root)`. Plonky3 claim circuits verify both. Doubles per-claim proof cost.
- **Option 2 — Deferred dual-track.** Single Blake2b root remains canonical. SHA3 reserved as a migration alternative if Blake2b is broken. Cheapest; least faithful to the spec text.
- **Option 3 — Selective dual-track (recorded as the chosen path below).** Dual-track at the **Ω-Commitment bundle root**; single-track at the per-sub-tree leaf and per-claim layer.

---

## 2. The decision

**Option 3 — Selective dual-track.**

- **Per-leaf hashing:** Blake2b-256 only. Unchanged from v0.4.0.
- **Per-sub-tree Merkle root:** Blake2b-256 only. Unchanged from v0.4.0.
- **Ω-Commitment bundle root** (the single 32-byte hash that roots the seven sub-trees and is attested by Mithril-PQ + recursive STARK + CIP-1694): published as a **tuple** `(blake2b_bundle_root, sha3_bundle_root)`. Both must be verified by anyone consuming the canonical Ω-Commitment. Divergence is treated as evidence of bug or tampering.
- **Plonky3 claim circuits** (track T2): single-track Blake2b. The dual-track lives one level above the claim layer, at the bundle attestation layer.
- **The `hash::dual_hash` API** stays as designed — it gains a real consumer in the bundle layer, becomes the canonical primitive for any future migration to fully-dual-track if Blake2b is broken.

---

## 3. Rationale

### 3.1 Where the legitimacy boundary actually lives

The Ω-Commitment bundle root is the single 32-byte (now 32+32-byte) value that:
- Mithril-PQ stake-attests at the fork height
- The recursive STARK proof binds via "I executed Praos and the resulting state matches THIS bundle"
- CIP-1694 governance ratifies as the canonical genesis

Everything below the bundle root inherits its legitimacy. Doubling the hash *at this layer* meaningfully hardens the part of the system that defines what Cardano even is on day-one of Omega. Doubling the hash on every claim circuit hardens vastly less, at vastly higher cost.

### 3.2 Cost asymmetry

- **Bundle-level dual-track cost:** seven extra Blake2b/SHA3 root computations (one per sub-tree, plus the bundle aggregation) at fork-time. Negligible. One-time. Re-run only if the genesis is regenerated.
- **Per-claim dual-track cost (Option 1):** every Plonky3 claim circuit must verify two Merkle paths against two roots. This roughly doubles proof size and proving time. Sustained over years and millions of claims, this is a multi-million-dollar real cost.

### 3.3 Migration ramp preserved

If Blake2b is ever compromised, the path to full dual-track is mechanical:
1. Republish per-sub-tree Merkle roots using SHA3 (using the existing `hash::dual_hash` primitive that has been wired up since v0.1.0).
2. Re-issue the Ω-Commitment bundle with both roots' worth of sub-tree roots.
3. Plonky3 claim circuits get a one-version upgrade adding the SHA3 path.

The point is: **the door is wide open.** We didn't burn the SHA3 primitive; we just didn't pay to use it everywhere on day-one.

### 3.4 Spec-text fidelity

The spec's "both must be checked by verifiers" is correct **at the bundle attestation layer** under this decision. We tighten the spec language to make this layer-specificity explicit (§4 below).

This is *not* a softening of the spec; it's a clarification of where the verification boundary lies. Anyone validating the canonical Ω-Commitment WILL check both hashes, full stop. Anyone running a per-claim verification trusts the bundle (which they validated dual-track) and verifies a single-track Merkle path against it.

### 3.5 Belt-and-braces stays belt-and-braces

The trust stack at fork-time is unchanged in structure:
- **Mithril-PQ** stake-attests the bundle (now a `(blake2b, sha3)` tuple).
- **Recursive STARK** proves Praos execution produced the bundle (now binds the tuple).
- **CIP-1694 governance** ratifies the bundle (now ratifies the tuple).

If any of the three attestations sees the wrong tuple — or if Blake2b and SHA3 disagree — the bundle is invalid. This is exactly the design intent.

---

## 4. Spec changes required

The following text changes go into `docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md`:

### §6 (Cryptographic primitives audit)

Update the "Hash function" row:

> | **Hash function** | Blake2b-256 | **Per-leaf and per-sub-tree:** Blake2b-256. **Bundle root:** dual-track tuple `(Blake2b-256, SHA3-256)`. Verifiers of the canonical Ω-Commitment must check both. | Mature |

### §7 (Genesis commitment bundle)

Update the bundle definition:

> **Bundle = (root_of(7 sub-tree blake2b roots), root_of(7 sub-tree sha3 roots))** → "Ω-Commitment"
>
> Per-sub-tree roots are computed once with Blake2b-256 and once with SHA3-256 (using the existing `hash::dual_hash` primitive). The bundle aggregates each into a separate root; the canonical Ω-Commitment is the pair.

### §8 (Trust stack)

Add a one-liner under §8.1 (Mithril-PQ) and §8.2 (Recursive STARK): "the attested artifact is the dual-track tuple, not a single hash."

### §9 (Claim transactions)

Add a clarifying sentence at the start: "Claim transactions verify against the **Blake2b-half** of the Ω-Commitment tuple. The SHA3-half is consumed only at the bundle attestation layer (see §8); per-claim circuit cost remains single-track."

---

## 5. Implementation impact

### 5.1 Code changes triggered by this decision

These are NOT in the scope of v0.4.x — they will be planned and shipped as part of the bundle assembly tooling, which is a future plan in track T1.

| Component | Change |
|---|---|
| `omega-commitment-core::tree` | No change. Blake2b only. |
| `omega-commitment-core::witness` | No change. Single-track. |
| `omega-commitment-cli::commit` (per sub-tree) | No change. Outputs Blake2b root. |
| **NEW:** bundle-assembly tool | Aggregates 7 sub-tree roots, computes both `bundle_blake` and `bundle_sha3`, publishes the tuple. Will be a separate plan after sub-tree 7 lands. |
| Plonky3 claim circuits (track T2) | Verify single-track Blake2b paths. Now unblocked. |

### 5.2 Per-sub-tree CLI: optional `--also-sha3` flag (deferred)

A future minor release may add `--also-sha3` to the `commit` subcommand to emit a parallel `commitment_sha3.json`. This is NOT urgent — the bundle-assembly tool can compute SHA3 sub-tree roots from the same input on demand.

---

## 6. What this unblocks immediately

- **Track T2 (Plonky3 claim circuits):** can now lock to v0.4.0 single-track Blake2b root format. Specifically, the `claim_utxo`, `claim_token_policy`, `claim_tx`, etc. circuits should be designed against single-track Merkle paths. Bundle-level dual-track concerns sit one layer above the claim circuit and are out of scope for circuit authors.
- **Sub-trees 5–7:** continue using the established Blake2b-only per-leaf and per-sub-tree model. No re-architecture needed.
- **CIP-Ω-1 (commitment format spec):** can now be drafted with concrete language. Ω-Commitment is a 64-byte tuple; per-sub-tree roots are 32 bytes Blake2b-only.

---

## 7. What remains pending after this decision

None at the program level. The dual-hash question is closed. Future revisits should be triggered only by:
- A credible cryptanalytic break of Blake2b-256, or
- A community governance action explicitly requesting full dual-track migration.

Both pathways are documented and the migration is mechanical.

---

## 8. Sign-off

This decision document is the canonical record. The spec, README, program roadmap, and wiki are updated to reference it.

**Recorded as authoritative on 2026-05-01.**
