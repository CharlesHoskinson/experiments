# Comprehensive README + ARCHITECTURE + GOALS overhaul plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land a single comprehensive update across `README.md`, `ARCHITECTURE.md`, `GOALS.md`, the ASCII diagrams, the To-Do list, and a new `RESEARCH-QUESTIONS.md`, integrating every cumulative design change since the previous version: the no-backdoor stance, Crypsinous + Chronos + Minotaur consensus stack, Starstream as native UTXO + zkVM, mass-MPC genesis ceremony, Filecoin-fork mirror partnerchain, three-layer constitutional binding, chunked anchoring, per-claim-type assignment policy, and the ten open research questions each treated in five-paragraph depth. All prose passes the humanizer rules.

**Architecture:** Eight tasks, sequenced. Tasks 1-3 update the top-level docs in place. Task 4 authors the new RESEARCH-QUESTIONS.md with 10×5 paragraphs of dedicated coverage. Tasks 5-6 redraw the diagrams. Task 7 runs humanizer across everything new. Task 8 commits and pushes. The plan is doc-only — no code touched, no tests affected.

**Tech Stack:** Markdown, ASCII box-drawing per the `ascii-visualizer` skill, humanizer-skill rules at `~/.claude/skills/humanizer/SKILL.md`, links to IACR ePrint papers + GitHub repos.

---

## File structure

```
experiments/
├── README.md                         (overhauled — status table, what-is, diagram, To-Do all rewritten)
├── ARCHITECTURE.md                   (overhauled — gain consensus stack section, Starstream section, mirror section)
├── GOALS.md                          (overhauled — track table updates per cumulative design)
├── RESEARCH-QUESTIONS.md             (NEW — 10 open issues, 5 paragraphs each)
└── cardano-wiki/
    └── wiki/log.md                   (append plan-execution entry)
```

No code files. No `Cargo.toml` changes. No tests affected.

## Pre-flight: source material

The cumulative source spec is `cardano-wiki/docs/superpowers/specs/2026-05-03-omega-archive-anchored-claims-design.md` at HEAD `83e3ea8`. Every claim in the new docs traces back to a §N reference in that spec.

Papers and repos to link, with stable URLs:

| Reference | URL |
|---|---|
| Ouroboros Crypsinous | https://eprint.iacr.org/2018/1132 |
| Ouroboros Chronos | https://eprint.iacr.org/2019/838 |
| Minotaur | https://eprint.iacr.org/2022/104 |
| Plonky3 | https://github.com/Plonky3/Plonky3 |
| Starstream (LFDT-Nightstream) | https://github.com/LFDT-Nightstream/Starstream |
| Nightstream meta | https://github.com/LFDT-Nightstream/Nightstream |
| Filecoin | https://github.com/filecoin-project |
| Cardano partnerchains SDK | https://github.com/input-output-hk/partner-chains |
| PLUME nullifier (ERC-7524) | https://eips.ethereum.org/EIPS/eip-7524 |
| FIPS 204 ML-DSA | https://csrc.nist.gov/pubs/fips/204/final |
| FIPS 205 SLH-DSA | https://csrc.nist.gov/pubs/fips/205/final |
| FIPS 206 FN-DSA (IPD) | https://csrc.nist.gov/presentations/2025/fips-206-fn-dsa-falcon |
| Mithril | https://mithril.network |
| CIP-1694 governance | https://cips.cardano.org/cip/CIP-1694 |
| CIP-19 addresses | https://cips.cardano.org/cip/CIP-19 |
| CIP-32 inline datums | https://cips.cardano.org/cip/CIP-32 |
| CIP-33 reference scripts | https://cips.cardano.org/cip/CIP-33 |
| EDPB Guidelines 02/2025 | https://www.edpb.europa.eu/system/files/2025-04/edpb_guidelines_202502_blockchain_en.pdf |
| Buser X-VRF break (FC 2024) | https://www.ifca.ai/fc24/preproceedings/213.pdf |
| Project Eleven BIP-85 → PQ derivation | https://blog.projecteleven.com/posts/generating-post-quantum-keypairs-from-a-single-24word-seed-phrase |
| Privacy Pools (Buterin et al.) | https://papers.ssrn.com/sol3/papers.cfm?abstract_id=4563364 |
| RuntimeVerification Plutus Core semantics | https://github.com/runtimeverification/plutus-core-semantics |

## Humanizer rules in scope

Read `~/.claude/skills/humanizer/SKILL.md` end-to-end before any prose work. Apply during writing, not after; a final audit pass per Task 7 catches survivors. The patterns to actively avoid (high-frequency-AI tells):

- Em dashes (`—`) — replace with comma, period, or restructure
- Copula avoidance: "stands as", "serves as", "marks a", "represents a" → just use "is"
- AI vocabulary: "delve", "underscore", "showcase", "vibrant", "pivotal", "key" used as adjective, "additionally", "fostering", "intricate", "tapestry", "landscape" used as abstract noun
- Superficial `-ing` endings: "highlighting...", "ensuring...", "reflecting...", "underscoring..."
- Negative parallelism: "not just X, it's Y"
- Forced rule-of-three lists
- Title case in headings (use sentence case)
- Emojis in prose (acceptable in status tables only if minimally used)
- Curly quotes (use straight)
- Generic positive conclusions ("future is bright", "exciting times")
- Sycophantic openers, knowledge-cutoff disclaimers, "I hope this helps"

Voice constraints: first-person "I" is acceptable when natural. Confident, direct claims preferred over hedged. Acknowledge uncertainty where uncertainty exists.

---

## Task 1: Overhaul `README.md`

**Files:**
- Modify: `/home/hoskinson/experiments/README.md`

The existing README is correct for the v0.9.1-era one-track framing. The cumulative design has expanded substantially. Key sections to update:

- The "What is Ouroboros Omega?" introduction picks up the Crypsinous + Chronos + Minotaur + Starstream stack and the no-backdoor stance.
- "How to read this repo" gains a pointer to the new RESEARCH-QUESTIONS.md and the Crypsinous/Chronos/Minotaur paper links.
- The existing ASCII diagram is replaced with a new layered diagram (Task 5).
- The status table reflects cumulative design state (Crypsinous-PQ in spec, mirror partnerchain in spec, Starstream upstream-tracked, etc.).
- The To-Do list is restructured by track (T1-T12) with explicit dependencies between tracks, with each item tagged with the spec section it implements.
- Honest disclosures: the ~10-20% silent-loss prediction, the no-backdoor cost.

- [ ] **Step 1: Read the current README**

```bash
cat /home/hoskinson/experiments/README.md | head -200
```

Note current section order, link patterns, voice register.

- [ ] **Step 2: Rewrite "What is Ouroboros Omega?" (5 paragraphs)**

Replace the existing paragraphs 12-20 in `README.md`. The new paragraphs cover, in order:

1. Why a clean-slate fork rather than in-place migration. Quantum threat timelines, NIST PQC standardisation. The mature version of the prior framing.
2. The Ω-Commitment as the bridge primitive. One-paragraph explanation of what gets committed and why holders can claim against it lazily.
3. The four-layer architecture: Ω-Commitment (T1) + Crypsinous-Chronos-Minotaur consensus (T2) + Starstream UTXO + zkVM (T3) + Filecoin-fork mirror partnerchain (T5). Each is a separate research-paper or open-source upstream.
4. The no-backdoor stance and what it forbids: no master keys, no court override, no escrow keys, no designated viewers. Three-layer constitutional binding from spec §13.1 (guardrails script + circuit invariants + social fork pre-commitment) makes the constraint mechanically enforced. Honest tension: 10-20% silent loss is the cost.
5. Cross-references: ARCHITECTURE.md is the deep-dive, GOALS.md is the program-level goal map, RESEARCH-QUESTIONS.md is the 10 open issues each treated at depth, the wiki holds the design spec and decision log.

- [ ] **Step 3: Rewrite "How to read this repo" (4 paragraphs)**

Same skeleton as current; add:
- Pointer to `cardano-wiki/docs/superpowers/specs/2026-05-03-omega-archive-anchored-claims-design.md` as the canonical design spec
- Pointer to `RESEARCH-QUESTIONS.md` for the open issues
- Paper links: Crypsinous, Chronos, Minotaur, Starstream, Filecoin
- Updated wiki page list

- [ ] **Step 4: Replace the ASCII diagram**

The existing diagram covers PRE-FORK CONSTRUCTION → GENESIS PUBLICATION → POST-FORK CLAIM. The new diagram has four lanes, drawn per Task 5. Insert the new diagram after the "How to read this repo" section.

- [ ] **Step 5: Rewrite the "Status as of 2026-05-03" table + commentary**

The table picks up the cumulative spec items:

```markdown
| Layer | State |
|---|---|
| T1 — Ω-Commitment construction (5 of 7 sub-trees, synthetic) | Shipped v0.9.1 |
| T1 — Headless mainnet ingestion | v1.0 in progress (omega-utxo-snapshot LSQ binary built) |
| T1 — Chain-follower for header + tx-index sub-trees | v1.1 planned |
| T1 — Chunked anchoring + embedded parser tarball | v1.2 planned |
| T1 — Mass-MPC ceremony tooling | v2.0 planned |
| T1 — Lean 4 reference impl for cross-impl reproducibility | v2.1 planned |
| T2 — PQ Crypsinous + Chronos + Minotaur consensus | Spec drafted, no implementation |
| T3 — Starstream UTXO + zkVM | Upstream LFDT-Nightstream tracking, no Omega-side integration |
| T5 — Mirror partnerchain (Filecoin fork) | Spec drafted, no implementation |
| T6 — Plonky3 verifier circuit | Not started, gated on T1 v1.1 stable commitment |
| T7 — Bridge protocol + claim semantics + guardrails script | Spec drafted |
| T9 — Whitepaper + formal spec | T1 design spec only |
```

Commentary paragraphs: 3-4 paragraphs covering the recent design decisions in plain prose. The current "the work that landed in the last 72 hours rewrote my mental model" framing is correct in voice; extend it for the consensus stack and mirror partnerchain decisions.

- [ ] **Step 6: Rewrite the "To do" list**

Restructure by track with explicit dependencies. Each item gets a one-paragraph description and a link to the wiki spec section it implements.

```markdown
## To do

Each item is tagged with the spec section it implements (`§N`) and the wiki page or plan file that covers it.

### T1 — Ω-Commitment tooling (this repo)

[Plan: cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md]

#### T1.v1.0 — finish real-mainnet ingestion for the 5 LedgerState-derivable sub-trees

(unchanged — same items as the existing list)

#### T1.v1.1 — chain-follower for header + tx-index sub-trees

(unchanged)

#### T1.v1.2 — chunked anchoring + embedded parser tarball  (NEW, §6.2 + §6.1)

The 218 GB Cardano-era snapshot is partitioned into ~3,400 ~64 MiB Merkle chunks. Each chunk has a Merkle path back to the bundle root. The chain commits each chunk-root at protocol epoch boundaries. The genesis pre-image also gains a content-addressed tarball of the CDDL spec + reference Rust + Haskell parser source, defending against year-50 encoding-format obsolescence. About a few hundred LOC of engineering work, mostly mechanical Merkle-of-Merkles bookkeeping. Done after v1.1 because the chain-follower has to land first.

#### T1.v2.0 — mass-MPC ceremony tooling  (NEW, §3)

Adapts the Zcash Sapling and Ethereum KZG ceremony codebases to Omega's specific ceremony shape. Participants are any Cardano stake holder; soundness requires only one honest re-deriver from block 0. Transcript livestreamed and OpenTimestamp-anchored to Bitcoin within the same epoch. Estimated 4-6 months of engineering and ceremony preparation; the ceremony itself runs once and then produces the genesis commitment.

#### T1.v2.1 — Lean 4 reference implementation  (NEW, §3.1)

The cross-implementation reproducibility check that gates the genesis ceremony requires bit-exact agreement between two independent codebases. The Rust workspace at `omega-commitment/` is the first impl; Lean 4 (extracting to runnable code) is the planned second. Provides both a differential check at ceremony time and a machine-checked correctness proof for the leaf-encoding, tree-construction, and bundle-aggregation logic.

### T2 — Consensus (PQ Crypsinous + Chronos + Minotaur)  [NEW track scope]

[Spec: §7 of the design spec]

Three Ouroboros papers compose: Crypsinous gives privacy-preserving PoS, Chronos gives permissionless clock synchronisation, Minotaur gives multi-resource consensus combining stake with the §6.5 mirror partnerchain's storage. All three need consistent PQ-primitive substitution (no curve VRF, no BLS aggregation, no Groth16). The hash-based VRF construction is research-paper-shaped open work (RESEARCH-QUESTIONS Q1).

### T3 — Smart-contract VM (Starstream)  [NEW track scope]

[Spec: §14 of the design spec; upstream: github.com/LFDT-Nightstream/Starstream]

Adopt LFDT-Nightstream/Starstream as Omega's native UTXO + zkVM layer. Goldilocks field + Poseidon2 hash align exactly with §3's mandated in-circuit primitives. Coroutines provide multi-step / atomic-bundle / time-locked claim primitives natively. Folding scheme means multiple claims by the same holder fold into a single recursive proof. Open issue: Plutus → Starstream translation pipeline.

### T4 — Network stack

(unchanged from prior plan; PQ-handshake variants of the Ouroboros networking miniprotocols)

### T5 — Storage layer + mirror partnerchain  [NEW track scope]

[Spec: §6.5 of the design spec]

Forks Filecoin under the Cardano partnerchains SDK. Replaces all curve crypto with PQ. Storage providers earn double revenue: Filecoin retrieval fees plus Omega-side block rewards via the partnerchain coupling. Provides the storage-resource input to Minotaur consensus per §7.2. ~6-12 months engineering.

### T6 — ZK verifier circuit

[Spec: §4 of the design spec]

The Plonky3 STARK that verifies claim transactions on Omega's ledger. PLUME nullifier, recipient-binding-inside-the-SNARK, item-count check against zero-padding, Starstream UTXO output emission. Verifier-circuit-ID is a rotatable protocol parameter per §13.4. Gated on T1.v1.1.

### T7 — Bridge protocol + guardrails script

[Spec: §4.3 + §13.1 of the design spec]

Per-claim-type assignment rules (`claim_utxo` forward-only allowed; `claim_governance` / `claim_script` / `claim_token_policy` banned without re-attestation). The guardrails script that mechanically rejects backdoor-shaped parameter updates. Both shipped together because they form one coherent governance-and-claim-semantics layer.

### T8 — Tooling + CLI

(largely as before; expanded by §9 wallet requirements: reproducible builds, multi-party signed releases, hardware-wallet-displayed claim payload, plurality of construction tools)

### T9 — Documentation + spec

The design spec at `cardano-wiki/docs/superpowers/specs/2026-05-03-omega-archive-anchored-claims-design.md` is the current reference. The next deliverable is a whitepaper that frames the program for the broader cryptographic and Cardano-ecosystem audiences.

### T10 — Audits + formal verification

(as before; gated on T1 v1.1 stable commitment + T2 spec frozen)

### T11 — Test-network operations

(as before; gated on T2 reference implementation)

### T12 — Mainnet operations + archival endowment funding

(as before; expanded by §6.3 archival bounty mechanics — perpetual treasury allocation funding storage-proof challenges)

### Cross-cutting work

#### Reproducibility-grade second implementation

(unchanged)

#### Mithril verification and the snapshot supply chain

Per §3.1: at least one ceremony participant MUST replay from Cardano block 0 using only the Mithril-multi-sig-certified immutable segment. The Mithril ledger-state ancillary file is treated as untrusted input. Cross-impl reproducibility requires bit-exact agreement on intermediate per-tree roots, not just bundle root.
```

- [ ] **Step 7: Verify all internal links resolve**

```bash
cd /home/hoskinson/experiments
grep -oE '\[[^]]+\]\(\.[^)]+\)' README.md | while read link; do
  path=$(echo "$link" | sed -E 's/.*\(\.\/?(.*)\)/\1/' | cut -d'#' -f1)
  test -e "$path" && echo "OK: $path" || echo "MISSING: $path"
done
```

Every relative link resolves to a file that exists.

---

## Task 2: Overhaul `ARCHITECTURE.md`

**Files:**
- Modify: `/home/hoskinson/experiments/ARCHITECTURE.md`

The existing ARCHITECTURE.md was correct as of the v0.9.1 era. New material to integrate:

- Update §1 cryptographic primitives section to add Goldilocks field, Poseidon2 mention, the §1 PQ-stack table from the design spec.
- New section: "Consensus stack: Crypsinous + Chronos + Minotaur" (3-4 paragraphs each).
- New section: "Starstream as the native UTXO + zkVM layer" (4-5 paragraphs).
- New section: "Mirror partnerchain (forked Filecoin)" (4-5 paragraphs).
- Update the "Lazy / pull-based resurrection" section with the per-claim-type assignment rules from spec §4.3.
- Update the "Cryptographic primitives" section with the no-curve-operations stance and the Mt Gox / 10-20% silent-loss disclosure.
- Update the "Tracks beyond commitment-tooling" section to reflect the new T2 scope (Crypsinous + Chronos + Minotaur), T3 scope (Starstream), and T5 scope (mirror partnerchain) — these were all previously "spec drafting" or "not started" and now have concrete upstream dependencies.

- [ ] **Step 1: Read current ARCHITECTURE.md**

```bash
cat /home/hoskinson/experiments/ARCHITECTURE.md | head -200
```

- [ ] **Step 2: Add new section after "The dual-track bundle root"**

Insert between existing §3 and §4. Section title: "Consensus stack: Crypsinous + Chronos + Minotaur, all post-quantum."

Three sub-sections of 3-4 paragraphs each:

(a) **Crypsinous** (eprint 2018/1132). Privacy-preserving PoS. Shielded VRF, shielded stake, shielded rewards, encrypted mempool. Original was curve-based; the PQ replacement uses Plonky3 STARKs in place of Groth16, hash-based VRF, hash-based threshold instead of BLS, Poseidon2 commitments inside circuits. Three paragraphs.

(b) **Chronos** (eprint 2019/838). Permissionless PoS-based global clock synchronisation. Removes external NTP-style dependency. The threshold-encryption committee from Crypsinous is the same committee Chronos pins epoch boundaries against. Verbatim quote from the abstract: "we obtain a permissionless PoS implementation of a global clock that may be used by higher level protocols that need access to global time." Three paragraphs.

(c) **Minotaur** (eprint 2022/104). Multi-resource consensus combining PoS with PoSpaceTime. Optimally-fungible security: `ω·β_w + (1−ω)·β_s < 1/2`. Honest majority required across the union, not in any single resource. The mirror partnerchain's storage providers are the PoSpaceTime input. Capturing Omega consensus now requires capturing both stake AND a meaningful fraction of the storage market. Three paragraphs.

(d) **The composition theorem**. All three papers descend from Ouroboros, share UC framework, compose without new soundness proofs. Engineering work is consistent PQ-primitive substitution, not new theorems. One paragraph.

- [ ] **Step 3: Add new section after the consensus stack**

Section title: "Starstream as the native UTXO + zkVM layer."

Five paragraphs:
1. Starstream is the LFDT-Nightstream UTXO-based zkVM. UTXO-based with coroutines as the core primitive. Native folding scheme. Compiles to WebAssembly. Off-chain execution sealed in succinct proofs.
2. Why it is load-bearing for Omega rather than swap-out. Goldilocks field + Poseidon2 align exactly with §1 PQ-stack mandates. The UTXO model preserves EUTXO mental continuity. Coroutines provide multi-step claim primitives natively.
3. What Starstream changes about claims. The verifier circuit emits a Starstream UTxO as its public output. claim_script becomes "submit a Starstream coroutine that produces the equivalent script-hash" rather than negotiating a foundation arbitrator. Multi-claim atomic bundles fold into a single recursive proof.
4. What Starstream does not solve. The hash-based VRF, the lattice-vs-hash signature decision, the mass-MPC ceremony — all orthogonal. Starstream is the post-claim execution layer.
5. Upstream maturity. LFDT-Nightstream/Starstream is in active design; compiler + interpreter + WebAssembly target shipping; IVC / MCC / lookups marked TODO upstream. Track T3 depends on these landing. Tracking via upstream rather than re-implementing.

- [ ] **Step 4: Add new section after Starstream**

Section title: "Mirror partnerchain (forked Filecoin)."

Five paragraphs:
1. The §6.3 storage-proof bounty alone funds replication; it does not provide a market for retrieval. A holder claiming in 2046 needs not just "the data exists somewhere" but "the data is fetchable from someone right now at predictable cost." For that we run a separate partnerchain — a fork of Filecoin — under the Cardano partnerchain model.
2. Forking Filecoin, not adopting it as-is. All curve crypto replaced with the §1 PQ stack. Filecoin's storage proofs (PoRep, Window-PoSt) are already hash-and-Merkle-based and port mechanically to Blake2b/SHA3/Poseidon2. The economic model survives unchanged.
3. Partnerchain coupling. Storage providers earn double revenue: Filecoin retrieval fees plus Omega-side block rewards via the partnerchain SDK. The mirror chain's consensus is itself Minotaur-shaped, with PoSpaceTime as the dominant resource.
4. The mirror partnerchain is OPTIONAL infrastructure. Omega's correctness does not depend on it. Holders who keep their own data still claim directly. The mirror is one of many possible providers of those proofs, not a privileged operator.
5. What the mirror partnerchain is NOT. Not a privileged operator. Not a censorship surface. Not a single point of failure. Not a regulator-friendly disclosure mechanism.

- [ ] **Step 5: Update existing sections for cumulative changes**

In §1 "Cryptographic primitives": add Goldilocks field mention, add Poseidon2 as the in-circuit hash, add reference to the §1 PQ-stack table in the design spec.

In §4 "Lazy / pull-based resurrection": add the per-claim-type assignment rules from §4.3 (forward-only for `claim_utxo`; banned for governance / script / token-policy).

In "Tracks beyond commitment-tooling": rewrite the per-track summaries to reflect:
- T2 = Crypsinous + Chronos + Minotaur stack (link to all three papers)
- T3 = Starstream (link to LFDT-Nightstream)
- T5 = Mirror partnerchain (link to Filecoin)
- T6 = ZK verifier (no change in scope; clarify it is the Plonky3 STARK)
- T7 = Bridge + guardrails script

- [ ] **Step 6: Verify section numbering + internal links**

```bash
grep -nE "^## |^### " /home/hoskinson/experiments/ARCHITECTURE.md
```

No gaps. Internal references like "see §N" point at sections that exist.

---

## Task 3: Overhaul `GOALS.md`

**Files:**
- Modify: `/home/hoskinson/experiments/GOALS.md`

- [ ] **Step 1: Read current GOALS.md**

- [ ] **Step 2: Update the "Tracks" table**

Change the status column for T2, T3, T5 from "Spec drafting" / "Not started" to "Spec drafted, upstream tracked" with a footnote linking to the design spec section. T3 now points at the Starstream upstream. T5 now exists as a track scope (was previously "storage / state management" generic).

- [ ] **Step 3: Update the per-track scope rows**

T2: "PQ Crypsinous + Chronos + Minotaur consensus stack" replacing "PQ Praos design + reference implementation."

T3: "Starstream UTXO + zkVM (LFDT-Nightstream upstream + PQ-Crypsinous shielding hooks)" replacing "Plutus-equivalent, plonky3-native execution model."

T5: "Mirror partnerchain (forked Filecoin under Cardano partnerchains SDK) + PoSpaceTime input to Minotaur consensus" replacing "UTxO storage + block storage adapted to plonky3 friendliness."

- [ ] **Step 4: Add a new "T1 sub-goals" section row for v1.2**

Insert between v1.1 and v2.0:

> ### v1.2 — chunked anchoring + embedded parser tarball
>
> Adds the §6.2 + §6.1 final-bullet items: split the snapshot into ~3,400 Merkle chunks committed at epoch boundaries; embed a content-addressed tarball of the CDDL + reference parser source into the genesis pre-image. About a few hundred LOC of engineering. Done after v1.1 because the chain-follower must land first.

- [ ] **Step 5: Update "Why" section if needed**

The "Why" section is voice-correct as-is; verify the timeline numbers (NIST PQC standardisation 2024, operational target 2030-2035) still match.

- [ ] **Step 6: Update "Non-goals"**

Add: "Reproducing Cardano's full state on Omega's chain by default. The chain-state is committed via the Ω-Commitment; replication is voluntary via the §6.5 mirror partnerchain or holder backups. The chain itself does not carry the data."

---

## Task 4: Author `RESEARCH-QUESTIONS.md`

**Files:**
- Create: `/home/hoskinson/experiments/RESEARCH-QUESTIONS.md`

This is the new artefact. Ten open issues from the design spec §15, each given a five-paragraph treatment. Length: approximately 15-20 pages of prose total.

The ten questions, in order:

1. **Hash-based VRF construction.** (X-VRF Buser et al. broken FC 2024; Praos-equivalent uniqueness reduction needed)
2. **Lattice-vs-hash signature decision.** (ML-DSA-65 / FN-DSA-512 vs SLH-DSA-only)
3. **PQ threshold-encryption committee composition.** (Per-epoch stake-weighted; specific PQ threshold scheme undecided)
4. **Claim-window length.** (5/7/10/20 years; trade-offs)
5. **Guardrails-script entrenchment depth.** (Forbidden-entirely vs higher-quorum)
6. **Plutus → Starstream translation.** (Source-of-truth semantics; automated vs holder-submitted)
7. **Starstream upstream maturity tracking.** (IVC/MCC/lookups TODO upstream)
8. **Filecoin PQ-port scope.** (Which Filecoin actor types port first; protocol-spec vs lotus-implementation level)
9. **Minotaur weighting parameter ω.** (Initial value; rotation policy; constitutional constraints)
10. **Mirror partnerchain economic model.** (Per-retrieval price floor; sustainability under declining ωADA)

For each, the five-paragraph structure:

- **Paragraph 1: What the open question is.** State the question precisely. Cite the spec section (§N) where it is identified. Cite the load-bearing source paper or open-source upstream. Concrete framing of "what would 'closed' look like."
- **Paragraph 2: Why it is open.** What prior work does or does not exist. What was tried and broke (e.g., X-VRF break for Q1). What is missing from the published literature or upstream codebase.
- **Paragraph 3: The decision space.** Concrete options. For each option, what it costs and what it gets you. Where possible, reference what other chains have done (Algorand on Falcon, Mina on transparent setup, Privacy Pools on association sets, etc.).
- **Paragraph 4: What it gates.** Which downstream tracks or v-bumps depend on this question being answered. What happens to the rest of the design if it is left open. Whether it is genesis-blocking or v∞-blocking.
- **Paragraph 5: How it gets resolved.** Concrete next steps. Whether the resolution is "adopt published paper X," "commission a specific research deliverable," "pick one option via governance," or "wait for upstream maturity." A timeline if estimable. The decision-maker (protocol owner, ceremony participants, CIP-1694 vote, upstream community).

- [ ] **Step 1: Author Q1 (Hash-based VRF)**

Five paragraphs covering:
- Para 1: Praos's adaptive-security theorem assumes a VRF with uniqueness and unpredictability under malicious key generation. The PQ replacement must hold these properties without reducing to "no collisions in H." X-VRF was the leading candidate; Bodaghi-Safavi-Naini at Financial Crypto 2024 published a uniqueness break.
- Para 2: Why the Praos theorem doesn't transfer for free. The original proof leans on DDH-style assumptions in the curve VRF; replacing with a hash-based construction means re-proving the security claim. The general-purpose hash-based VRF literature (Goldberg-Naor-Reyzin, etc.) was not designed for adaptive-security blockchain consensus.
- Para 3: Three options. (a) leanXMSS-style XMSS+SNARK verifier with a custom uniqueness proof. (b) Poseidon2-based VRF inside a STARK with a published security analysis. (c) Skip a true PQ VRF in v1, use FN-DSA-randomness-beacon + verifiable-delay-function in the Algorand-style fallback.
- Para 4: This question gates Crypsinous-PQ, Chronos-PQ, and Minotaur-PQ all three. Without it, the consensus stack has no clean PQ implementation path. Genesis-blocking.
- Para 5: Resolve by either pinning a published paper (research community deliverable) or commissioning one (paid academic engagement). Estimated 6-12 months to publication-grade. The decision-maker is the protocol-design lead in coordination with the research community.

- [ ] **Step 2: Author Q2 through Q10**

Same five-paragraph structure for each. Maintain consistent voice register; vary sentence rhythm; avoid the AI tells per the humanizer rules.

For Q4 (claim-window length): cite Mt Gox 10-year creditor wait as empirical anchor. Trade-off: shorter window = quantum-pre-fork-Ed25519 safety; longer = vault-holder forgiveness.

For Q5 (guardrails entrenchment): the binary is forbidden-entirely vs higher-than-supermajority quorum. The latter preserves a theoretical escape hatch but creates a target. The former is cleaner but assumes any future need to update the guardrails routes through chain replacement.

For Q6 (Plutus → Starstream): the canonical reference is the RuntimeVerification K-spec for Plutus Core. The decision points are which spec is source-of-truth, whether translation is automated or holder-submitted, whether script-hash equivalence is verified by the protocol or attested via dApp re-deployment.

For Q9 (Minotaur ω): explicitly address whether governance can push ω = 1 and disable storage-resource consensus. Per §13.1 guardrails script analysis, this is constrained by what counts as a "backdoor-shaped" parameter update — but ω specifically is a tuning knob, not a backdoor. Clarify the line.

- [ ] **Step 3: Add a header and a wrap-up**

The file gets a 2-paragraph header explaining the document's purpose and the relationship to the design spec. A wrap-up section near the end captures the meta-pattern: which questions are research-paper-shaped, which are governance-shaped, which are engineering-shaped.

- [ ] **Step 4: Verify length + voice register**

```bash
wc -w /home/hoskinson/experiments/RESEARCH-QUESTIONS.md
```

Target: 5,000-8,000 words. Five paragraphs of about 100-150 words each, ten questions, plus header and wrap-up.

---

## Task 5: Redraw the README architecture diagram

**Files:**
- Modify: `/home/hoskinson/experiments/README.md` (the diagram block)

Use the `ascii-visualizer` skill conventions. Box-drawing characters: `┌─┐ ├─┤ └─┘ │ ▼ ◄`.

The new diagram has four lanes (was three):

```
LANE 1 — PRE-FORK CONSTRUCTION
LANE 2 — GENESIS PUBLICATION (mass-MPC ceremony)
LANE 3 — POST-FORK CLAIM (claim verifier + Starstream output)
LANE 4 — CONSENSUS + ARCHIVE (Crypsinous-Chronos-Minotaur + mirror partnerchain)
```

- [ ] **Step 1: Sketch the four-lane diagram**

Draft on paper / scratchpad. Each lane labeled clearly. Arrows show data flow between lanes. Mark trust boundaries with `◄── trust boundary` annotations.

- [ ] **Step 2: Render in ASCII**

Use box-drawing characters consistently. Width ≤ 96 columns so the diagram renders correctly in GitHub markdown view. Verify by:

```bash
awk '/^```$/{p=!p; next} p{print length}' /home/hoskinson/experiments/README.md | sort -rn | head -1
```

(Longest line in any code block must be ≤96.)

- [ ] **Step 3: Add inline annotations**

Each box has a 2-3 word label inside; the prose around the diagram explains what each box does. Trust boundaries are explicitly labelled (the Mithril certificate, the genesis ceremony attestor signatures, the plonky3 verifier circuit ID).

- [ ] **Step 4: Replace the existing diagram in README.md**

`Edit` tool: find the existing diagram block (delimited by ```` ``` ```` fences), replace with the new four-lane diagram.

---

## Task 6: Add a "consensus stack composition" diagram

**Files:**
- Modify: `/home/hoskinson/experiments/ARCHITECTURE.md`

A second diagram in ARCHITECTURE.md shows how Crypsinous + Chronos + Minotaur compose. Three boxes plus arrows showing what each contributes:

```
   Crypsinous           Chronos            Minotaur
   (privacy)            (time)             (multi-resource)
        │                   │                    │
   shielded VRF        permissionless      stake + storage
   shielded stake      PoS clock           + future resources
   shielded rewards    sub-protocol        ω-weighted security
        │                   │                    │
        └───────────────────┼────────────────────┘
                            ▼
                  Composite Ouroboros-Omega
                  consensus protocol, all PQ
```

- [ ] **Step 1: Sketch + render**

Same conventions as Task 5. Width ≤96 cols. Insert after the new "Consensus stack" prose section.

---

## Task 7: Humanizer pass on all new prose

**Files:**
- Modify: `/home/hoskinson/experiments/{README.md, ARCHITECTURE.md, GOALS.md, RESEARCH-QUESTIONS.md}`

- [ ] **Step 1: Read humanizer skill rules**

```bash
cat /home/hoskinson/.claude/skills/humanizer/SKILL.md
```

- [ ] **Step 2: Pattern-search each file**

```bash
cd /home/hoskinson/experiments
for f in README.md ARCHITECTURE.md GOALS.md RESEARCH-QUESTIONS.md; do
  echo "=== $f ==="
  grep -nE "—| stands as | serves as |delve|underscore|showcase|vibrant|pivotal|tapestry|fostering|intricate|highlighting,|emphasizing,|reflecting,|underscoring,|in order to|at this point in time|in the event that|It is important to note that|future is bright|future looks bright|exciting times" $f | head -20
done
```

For each match, evaluate whether the rule applies (sometimes "key" as a noun is fine, sometimes em dash inside a code block doesn't count) and rewrite if it does.

- [ ] **Step 3: Audit-pass per humanizer skill**

Per the skill's "final anti-AI pass" guidance: ask "what makes the below so obviously AI generated?" Identify remaining tells. Revise.

- [ ] **Step 4: Re-run pattern search to confirm clean**

```bash
cd /home/hoskinson/experiments
for f in README.md ARCHITECTURE.md GOALS.md RESEARCH-QUESTIONS.md; do
  echo "=== $f ==="
  count=$(grep -cE "—| stands as | serves as |delve|underscore|showcase|vibrant|pivotal|tapestry|fostering|intricate|in order to" $f)
  echo "AI-tell hits: $count"
done
```

Expected: low single digits per file (some are unavoidable / context-correct).

---

## Task 8: Commit + push

**Files:** all modified files

- [ ] **Step 1: Verify all four cargo checks still pass**

```bash
cd /home/hoskinson/experiments/omega-commitment
. ~/.cargo/env
cargo clean
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

(Doc-only changes should not affect tests, but verify because we updated paths in places.)

- [ ] **Step 2: Verify no Claude attribution in pending commits**

The commit-msg hook at `.git/hooks/commit-msg` enforces this; the verification is the hook's existence + recent test (already done in prior batches).

- [ ] **Step 3: Append wiki log entry**

```markdown
## [2026-05-03] overhaul | comprehensive README + ARCHITECTURE + GOALS + RESEARCH-QUESTIONS rewrite
- Integrated cumulative spec changes since prior version: Crypsinous + Chronos + Minotaur consensus stack, Starstream as native UTXO + zkVM, mass-MPC genesis ceremony, Filecoin-fork mirror partnerchain, three-layer constitutional binding, chunked anchoring, per-claim-type assignment policies
- New RESEARCH-QUESTIONS.md with 10 open issues each treated at 5-paragraph depth
- Two new diagrams: 4-lane README architecture, consensus-stack composition in ARCHITECTURE
- All prose humanizer-audited; Mt Gox 10-year creditor wait is the empirical anchor for the claim-window question
- All paper + repo links verified resolvable
```

- [ ] **Step 4: Commit**

```bash
cd /home/hoskinson/experiments
git add README.md ARCHITECTURE.md GOALS.md RESEARCH-QUESTIONS.md cardano-wiki/wiki/log.md
git commit -m "$(cat <<'COMMIT_MSG'
docs: comprehensive overhaul integrating Crypsinous+Chronos+Minotaur, Starstream, mirror partnerchain

Updates README.md, ARCHITECTURE.md, GOALS.md, and adds RESEARCH-QUESTIONS.md
covering the cumulative design changes since the prior version.

Cumulative additions:
- §1 No-backdoor stance plus 3-layer constitutional binding (guardrails
  script + circuit invariants + social fork pre-commitment)
- §7 Three-paper consensus stack: Crypsinous (privacy, eprint
  2018/1132) + Chronos (time, eprint 2019/838) + Minotaur (multi-
  resource, eprint 2022/104), all PQ
- §14 Starstream as native UTXO + zkVM layer (LFDT-Nightstream upstream)
- §6.5 Forked Filecoin mirror partnerchain under Cardano partnerchains
  SDK; couples to Minotaur as PoSpaceTime resource input to consensus
- §6.2 Chunked anchoring + §6.1 embedded parser tarball
- §4.3 Per-claim-type assignment policy split (claim_utxo forward-only;
  claim_governance / claim_script / claim_token_policy banned without
  re-attestation)
- §3 Mass-MPC genesis ceremony replaces 7-of-12 threshold-signed
  publication

New RESEARCH-QUESTIONS.md treats 10 open issues each at 5-paragraph
depth: hash-based VRF, lattice-vs-hash signatures, PQ threshold-
encryption committee, claim-window length, guardrails-script
entrenchment, Plutus->Starstream translation, Starstream upstream
maturity, Filecoin PQ-port scope, Minotaur omega, mirror partnerchain
economics.

Two new diagrams: 4-lane README architecture (pre-fork construction,
genesis publication, post-fork claim, consensus+archive); consensus
stack composition in ARCHITECTURE showing how Crypsinous+Chronos+
Minotaur compose without new soundness proofs.

All prose passes the humanizer skill audit. All paper and repo links
verified resolvable. cargo test/fmt/clippy clean (doc-only changes).
COMMIT_MSG
)"
```

- [ ] **Step 5: Push**

```bash
git push origin main
```

- [ ] **Step 6: Verify on GitHub**

```bash
gh repo view CharlesHoskinson/experiments
gh api repos/CharlesHoskinson/experiments/commits --jq '.[0] | "\(.sha[0:8]) \(.commit.author.name) \(.commit.message | split("\n") | .[0])"'
```

Latest commit's first-line message starts with `docs: comprehensive overhaul`. Author is `charles hoskinson`.

---

## Self-review

**Spec coverage.** Each cumulative spec change has a corresponding update task: §1 no-backdoor and §13 governance binding land in README + ARCHITECTURE intro paragraphs (Task 1 step 2, Task 2 step 5). §3 mass-MPC ceremony lands in ARCHITECTURE consensus section (Task 2 step 2). §4.3 per-claim-type policy lands in ARCHITECTURE lazy-resurrection section (Task 2 step 5). §6.2 chunked anchoring lands in README To-Do as v1.2 task (Task 1 step 6). §6.5 mirror partnerchain lands in ARCHITECTURE new section (Task 2 step 4) and GOALS T5 row (Task 3 step 3). §7 consensus stack lands in ARCHITECTURE new section (Task 2 step 2) and consensus diagram (Task 6). §14 Starstream lands in ARCHITECTURE new section (Task 2 step 3) and GOALS T3 row (Task 3 step 3). §15 ten open issues each get 5-paragraph treatment in RESEARCH-QUESTIONS.md (Task 4).

**Placeholder scan.** All steps have explicit prose, code blocks, or commands. No "TBD," no "fill in details." The five-paragraph structure for the open questions specifies what each paragraph covers.

**Type / name consistency.** All papers cited consistently: "Crypsinous (eprint 2018/1132)," "Chronos (eprint 2019/838)," "Minotaur (eprint 2022/104)." All section references use `§N` notation. All file paths absolute or repo-relative consistently.

**Humanizer integration.** Task 7 is a standalone audit pass. Step 2 includes a concrete grep-pattern catalogue of common AI tells. The humanizer rule reading is mandatory before composition (mentioned in pre-flight + Task 7 step 1).

---

## Execution

Auto mode is on; the user requested the plan with the expectation of execution. Recommended approach: superpowers:subagent-driven-development with one sub-agent per task (8 sub-agents total). Tasks 1-3 are independent and can run in parallel; Task 4 (RESEARCH-QUESTIONS.md) is the largest single deliverable and runs alone; Tasks 5-6 (diagrams) are short and run in parallel after Tasks 1-2; Task 7 (humanizer) runs after all prose work; Task 8 (commit + push) runs last.

Inline execution is also viable for the smaller tasks (3, 5, 6, 8); the long-prose Task 4 benefits from a dedicated sub-agent.
