# Ouroboros Omega — Program Roadmap

> **Note:** This is a *program-level roadmap*, not a TDD implementation plan. It decomposes the multi-year Omega program into independently-shippable tracks, each of which gets its own TDD plan in this directory. The first concrete TDD plan is `2026-05-01-omega-utxo-commitment-plan.md`.

**Goal:** Sequence and parallelize the work needed to deliver Ouroboros Omega per the design spec at `docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md`.

**Architecture:** Twelve parallel/sequential tracks across research, engineering, governance, and operations. Each track produces one or more independently testable deliverables.

**Tech Stack:** Rust (primary), Haskell (legacy interop with Cardano node), Plonky3, Cardano testnets, GitHub Actions, IPFS/Arweave for archives.

---

## Nine locked decisions (carried forward from the spec)

1. Goal ranking: **B > C > A** (velocity > storage > wallet thinness)
2. Migration: **lazy / pull-based** ZK resurrection
3. Provable scope: **everything** (UTXOs, tokens, history, scripts, stake, governance)
4. Trust: **belt-and-braces** (Mithril-PQ + recursive STARK + CIP-1694)
5. Sunset: **D → C → A** (governance-gated)
6. Crypto: **post-quantum throughout, no exceptions**
7. Sigs: **PQ-only from day one** (no dual-sig)
8. ZK system: **Plonky3** (FRI / hash-only / recursion-friendly)
9. **Dual-hash:** selective — bundle root is a `(blake2b, sha3)` tuple; per-leaf and per-sub-tree are Blake2b-only. Decision doc: `docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md`. **Unblocks track T2 (Plonky3 claim circuits).**

---

## Twelve tracks

Each track is independently chartered, has its own success criteria, and produces one or more concrete deliverables. Some tracks are sequential (T1 must precede T2); most are parallelizable.

### T1 — Ω-Commitment Tooling *(start here)*
**What:** A Rust workspace that reads Cardano ledger state at a given slot and computes the seven sub-tree commitments. Produces `omega-commitment-core` (lib) + `omega-commitment-cli` (binary) + test fixtures against mainnet.
**Why first:** Has zero research dependencies. Can be tested against current mainnet today. Immediately useful as a public-good tool (auditing, light-client research, etc.). Delivers working software in 2-4 weeks.
**Deps:** none.
**Output:** `omega-commitment` repo. Reference Merkle/Verkle tree library. JSON-serialized commitment bundles.
**TDD plan:** `2026-05-01-omega-utxo-commitment-plan.md` (first sub-tree only — UTXO set; subsequent sub-trees in follow-on plans).

### T2 — Plonky3 Circuit Library
**What:** Reusable Plonky3 gadgets for the six claim types: UTXO inclusion, token policy lookup, tx history witness, script provenance, stake state proof, governance state proof. Plus the *recursive aggregation* circuit that proves "this batch of claims is valid against the same Ω-Commitment."
**Why early:** Circuits are the long-pole engineering item. Even mock claim circuits unblock node integration work.
**Deps:** T1 (consumer of commitment format).
**Output:** `omega-circuits` repo. One Rust crate per claim type + a recursion crate.
**TDD plan:** TBD per claim type — start with `omega-utxo-claim-circuit`.

### T3 — Mithril-PQ Research → Reference Implementation
**What:** Replace BLS12-381 STM with a hash-based STM over SLH-DSA + VRF eligibility. Begins as a **research track** (formal model, security proof, peer review) before engineering. Eventual deliverable: a `mithril-pq` crate compatible with the existing `mithril` repo's interfaces.
**Why parallel:** Highest research risk in the program. Must shadow-run on mainnet ≥1 year before fork.
**Deps:** none for research; T2 once integration begins.
**Output:** Paper(s), `mithril-pq` crate, mainnet shadow-attestation network.
**TDD plan:** TBD — too research-loaded to TDD-plan upfront. First TDD plan: `mithril-pq-pure-function-stm-prototype-plan.md` for the core algebraic primitives.

### T4 — Hash-VRF Specification → Reference Implementation
**What:** Replace Ed25519-VRF with hash-VRF (or lattice-VRF — picked in T4). Spec, security analysis, then `omega-vrf` crate.
**Why parallel:** Independent of all engineering tracks. Pure crypto research.
**Deps:** none.
**Output:** CIP draft, paper, `omega-vrf` crate.
**TDD plan:** TBD — primarily research; first TDD task is a benchmark suite.

### T5 — Recursive STARK Proof of Ouroboros Execution
**What:** Build a Plonky3 zkVM (or custom circuit) that proves "executed Ouroboros Praos from slot 0 to slot S, output state matches Ω-Commitment." Run on a **funded proving cluster** for the actual mainnet history.
**Why later:** Requires T1 (commitment format) and T2 (Plonky3 fluency). Most expensive single deliverable. Estimated $5-15M proving cost + 6-12 months wall time.
**Deps:** T1, T2.
**Output:** `omega-history-prover` repo + the recursive proof artifact for actual mainnet.
**TDD plan:** TBD. Start with a synthetic 1000-block test chain.

### T6 — Omega Node (`omega-node`)
**What:** Reference implementation of the new chain. Rust. Implements the new ledger, claim transaction processing, native Leios pipelining, encrypted mempool, account abstraction. Verifier integration consumes circuits from T2.
**Why parallel:** Can ship as testnet-only well before T3/T5 are mainnet-ready.
**Deps:** T2 (verifier circuits), T7 (script ISA), T4 (VRF).
**Output:** `omega-node` repo, `Ω-preview` testnet.
**TDD plan:** TBD — large; start with `omega-node-genesis-bootstrap-plan.md` (read Ω-Commitment, validate it, emit block 0).

### T7 — Script ISA Design + Compiler
**What:** Design and implement a STARK-friendly script VM to replace UPLC. Candidate options: Cairo, custom Plonky3-RISC, Valida-style. Includes a compiler from at least one high-level language (Aiken-Omega? Plonky3-Aiken?).
**Why parallel:** Pure design + research initially; engineering once design freezes.
**Deps:** T2 (proving system fluency).
**Output:** ISA spec, `omega-vm` crate, `omega-aiken` compiler.
**TDD plan:** TBD — first plan: `omega-vm-instruction-decoder-plan.md`.

### T8 — Hardware Wallet Coordination
**What:** Multi-vendor coordination (Ledger, Trezor, Tangem, Keystone, BitBox) for ML-DSA + SLH-DSA + Falcon support. Reference firmware for an open HW wallet. Test vectors.
**Why early:** 2-year coordination window; vendors plan firmware roadmaps far in advance.
**Deps:** none.
**Output:** Vendor MOUs, firmware reference, integration test suite.
**TDD plan:** TBD — first deliverable is non-code (vendor-engagement deck + technical brief).

### T9 — CIP Drafting Track
**What:** Write the CIPs the Cardano governance system must ratify. At minimum:
- CIP-Ω-1 — Ω-Commitment format spec
- CIP-Ω-2 — Mithril-PQ aggregation spec
- CIP-Ω-3 — PQ signature scheme on the new chain (ML-DSA + SLH-DSA + Falcon)
- CIP-Ω-4 — Hash-VRF spec
- CIP-Ω-5 — STARK-friendly script ISA
- CIP-Ω-6 — Six claim transaction types
- CIP-Ω-7 — Sunset trajectory (D → C → A) governance contract
- CIP-Ω-8 — Hard-fork action (genesis ratification)
**Why parallel:** Pure documentation work. CIPs need community review cycles measured in months.
**Deps:** intermittent — each CIP depends on its corresponding research/engineering track maturing.
**Output:** Eight CIP drafts in `cardano-foundation/CIPs`.
**TDD plan:** N/A (documentation track).

### T10 — Governance / Political Track
**What:** Coordinate Intersect, Cardano Foundation, IOG, Emurgo, DRep community, SPO community. Build consensus for the fork. Run informational governance actions to gauge support. Establish working groups for each technical track.
**Why parallel:** Cannot be skipped or compressed. Hardest track to manage.
**Deps:** T9 (need CIPs to discuss).
**Output:** Working group charters, info-action results, ratified CIP-Ω-7 (sunset), ratified CIP-Ω-8 (hard-fork action).
**TDD plan:** N/A.

### T11 — Migration Tooling for End Users
**What:** Reference open-source proving service (so users with cold wallets can claim without running a prover locally). Wallet integration kits (Lucid-Omega, MeshJS-Omega). Block explorer that queries both old and Omega chains.
**Why later:** Needs T2 (circuits) + T6 (node) + T1 (commitments) all functional first.
**Deps:** T1, T2, T6.
**Output:** `omega-prover-service`, `lucid-omega`, `omega-explorer`.
**TDD plan:** TBD — first plan: `omega-prover-service-utxo-claim-plan.md`.

### T12 — Operational / Treasury Track
**What:** Treasury proposal under Plomin governance to fund the program. Estimated $50–100M ADA program budget across 5 years. Allocate to working groups, proving cluster, audits, formal verification. Continuous reporting.
**Why parallel:** Funding must be in place before each track can scale.
**Deps:** T10.
**Output:** Treasury withdrawal action, quarterly reports, audited financials.
**TDD plan:** N/A.

---

## Sequencing diagram

```
   year:        Y0     Y1       Y2       Y3       Y4       Y5
   T1  ════════════════                                                (Ω-commitment tooling)
   T2          ═══════════════════                                     (claim circuits)
   T3   ═══════════════════════════════════════════                    (Mithril-PQ research → impl)
   T4   ════════════════════════                                       (hash-VRF)
   T5                  ═══════════════════════════                     (recursive STARK)
   T6                  ═══════════════════════════════                 (omega-node)
   T7          ═══════════════════════                                 (script ISA)
   T8   ════════════════════════════════════════                       (HW wallet coord)
   T9   ═══════════════════════════════════════                        (CIPs)
   T10  ═══════════════════════════════════════════════                (governance)
   T11                          ═══════════════════════                (migration tooling)
   T12  ═══════════════════════════════════════════════════════════    (treasury / ops)

   Mainnet activation:                                       ≈Y5 ↑
```

---

## Suggested first six concrete TDD plans (in priority order)

1. **`2026-05-01-omega-utxo-commitment-plan.md`** *(this directory, written today)* — Rust workspace + UTXO Merkle root tooling. Track T1.
2. **`2026-XX-XX-omega-block-header-accumulator-plan.md`** — block header sub-tree. Track T1.
3. **`2026-XX-XX-omega-utxo-claim-circuit-plan.md`** — first Plonky3 claim circuit. Track T2.
4. **`2026-XX-XX-mithril-pq-stm-primitives-plan.md`** — algebraic primitives. Track T3.
5. **`2026-XX-XX-omega-node-genesis-bootstrap-plan.md`** — node reads Ω-Commitment, emits block 0. Track T6.
6. **`2026-XX-XX-cip-omega-1-draft-plan.md`** — CIP drafting workflow. Track T9.

These six produce working, testable software (or in T9's case, a peer-reviewable spec draft) within ~3 months of program kickoff and unblock everything else.

---

## Risk register (program-level)

| Track | Risk | Mitigation |
|---|---|---|
| T3 | Mithril-PQ doesn't reach maturity | Mainnet shadow-test ≥1 year; fall back to dual-sig if PQ STM has unfixable flaws (with governance approval) |
| T5 | Recursive STARK proving cost overruns | Start with a synthetic chain; iterate on Plonky3 perf; only commit to full mainnet history once cost-per-epoch is stable |
| T7 | Script ISA wars (community can't agree) | Time-box selection to 12 months; default to Cairo if no consensus emerges |
| T8 | HW vendors won't ship PQ in time | Provide reference firmware; if necessary, fork Trezor's open-source firmware ourselves |
| T10 | Governance refuses fork | Sunset is graceful (D→C→A means old chain survives indefinitely if needed); fork is opt-in |
| T12 | Treasury can't fund | Right-size to community willingness; cut scope via additional CIP-1694 actions |

---

## How to read this document

- This is the **program roadmap**. It does not contain TDD-task-level steps.
- Each track gets one or more **TDD implementation plans** that *do* contain TDD-task-level steps. Those plans live alongside this one in `docs/superpowers/plans/`.
- **Start by executing `2026-05-01-omega-utxo-commitment-plan.md`** — it's the lowest-risk, highest-leverage first deliverable.
