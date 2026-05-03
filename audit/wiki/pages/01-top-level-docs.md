# Top-Level Docs Audit

_Source: parallel agent pass, 2026-05-03. Files audited: `README.md`, `ARCHITECTURE.md`, `GOALS.md`, `RESEARCH-QUESTIONS.md`, `instructions.md`, `audit-prompt.md`, `LICENSE`._

## What this repo claims to be

Per README.md:2-3, a "working space for in-progress research and prototypes" where **Ouroboros Omega** ("a clean-slate post-quantum redesign of Cardano with cryptographic continuity to every prior era of the chain") lives as a twelve-track program. The repo itself contains T1 (commitment tooling) plus the supporting wiki. README.md:22 stresses: "This repo is the gating dependency for most of what comes next."

Two core artifacts: `omega-commitment/` (a Rust workspace producing the Ω-Commitment, v0.9.1 + Batch 1/2 audit fixes, 282 tests) and `cardano-wiki/` (LLM-maintained research wiki). README.md:10 frames the four root-level docs at "decreasing levels of abstraction."

## Stated architecture

A four-layer post-quantum stack (ARCHITECTURE.md:18):

1. **Layer One** — T1 commitment-tooling + on-chain verifier circuit (T6): Blake2b/SHA3 dual-hash over seven sub-trees of pre-fork state (UTxOs, headers, tx-index, token policies, scripts, stake, governance), published once in genesis as the Ω-Commitment.
2. **Layer Two** — Consensus: PQ-Crypsinous (shielded VRF, encrypted mempool) + PQ-Chronos (permissionless PoS clock) + PQ-Minotaur (multi-resource: stake + storage).
3. **Layer Three** — Smart contracts: LFDT-Nightstream/Starstream zkVM (UTXO-based, coroutines, native folding, Goldilocks + Poseidon2 in-circuit).
4. **Layer Four** — Optional infrastructure: Filecoin-fork mirror partnerchain under Cardano partnerchains SDK.

All primitives post-quantum; no curve operations anywhere (ARCHITECTURE.md:6). ARCHITECTURE.md:68-119 specifies the three consensus papers and their composition.

## Stated goals

GOALS.md:17-37 design properties:
- All primitives post-quantum (SLH-DSA for genesis ceremonies; lattice-vs-hash open for user signing — see Q2).
- Plonky3-friendly state model (every per-block transition expressible as STARK constraints).
- Composite Ouroboros consensus, all PQ.
- One-way bridge (`claim_*` transactions with ZK Merkle proofs; no two-way peg).
- Lazy state migration (genesis ledger essentially empty; dust addresses do not migrate).
- **No backdoors** — three layers of constitutional binding: CIP-1694 guardrails script + Plonky3 circuit invariants + wallet ecosystem social fork (Steem-to-Hive template).
- Mass-MPC genesis ceremony (Zcash-Sapling style).
- Selective dual-hash at bundle layer (Blake2b + SHA3, drift detection — not Blake2b-break hedge per ARCHITECTURE.md:9).
- Optional mirror partnerchain (Filecoin fork; Omega correctness independent of it).

T1 sub-goals: v0.x synthetic ingestion DONE (v0.9.1, 282 tests); v1.0 real mainnet for 5 sub-trees IN PROGRESS; v1.1 chain-follower for remaining 2 planned; v1.2 cumulative integration; v2.0 second implementation.

## Research questions / open work

RESEARCH-QUESTIONS.md, ten open issues:

1. **Hash-based VRF** — construction with Praos-equivalent uniqueness reduction, gates T2 consensus entirely.
2. **Lattice-vs-hash signature decision** — ML-DSA-65 vs FN-DSA-512 vs SLH-DSA for user signing.
3. **PQ threshold-encryption committee** — gates Crypsinous's encrypted mempool.
4. **Claim-window length** — 5/7/10/20 years; governance-shaped, genesis-blocking.
5. **Guardrails-script entrenchment** — forbidden-entirely or higher-quorum updatable.
6. **Plutus → Starstream translation** — automated compiler or holder-submitted + dApp attestation.
7. **Starstream upstream maturity** — type checker, IVC, MCC, lookups still TODO upstream.
8. **Filecoin PQ-port scope** — 6-12 months; forest-level vs lotus-level vs spec-level.
9. **Minotaur weighting parameter ω** — initial value, rotation policy.
10. **Mirror partnerchain economic model** — revenue split, price floor.

## Internal consistency

Strong:
- All four root docs trace dependencies to the same commitment primitive and canonical spec (`cardano-wiki/docs/superpowers/specs/2026-05-03-omega-archive-anchored-claims-design.md`).
- README/GOALS/ARCHITECTURE track tables align on track boundaries.
- RESEARCH-QUESTIONS.md:128-138 maps each Q to decision shape and timeline.
- ARCHITECTURE.md:156-182 mirrors README.md:48-54 on the v1.0 pipeline pivot.
- Threat model consistent: dust long-tail (GOALS.md:29, ARCHITECTURE.md:150), Mt Gox precedent (RESEARCH-QUESTIONS.md:45), Steem-to-Hive social fork (README.md:20, GOALS.md:31).
- Cross-refs resolve; LICENSE matches README.md:328 (Apache-2.0).

## Notable observations

1. **Load-bearing VRF gap** — Q1 gates all of T2; no published PQ-VRF meets Praos uniqueness. 6-12 month research window.
2. **Two-stream v1.0 pivot (2026-05-03)** — `cardano-cli 10.16` lacks `--output-cbor` for ledger-state dumps; `omega-utxo-snapshot` (pallas-network LSQ) is now critical path.
3. **Dual-hash reframing (2026-05-03)** — bundle-layer SHA3 documented as drift detection, not Blake2b-break hedge (ARCHITECTURE.md:9, audit finding A1/F004). True independent SHA3 tree deferred to v2.0.
4. **No-backdoor as identity** — three-layer constitutional binding; tension acknowledged at RESEARCH-QUESTIONS.md:59 (clean entrenchment vs. chain-replacement to fix legitimate bugs).
5. **T1 is the gate** — README.md:22, ARCHITECTURE.md:170: T1 gates T6/T7/T8. Nothing locks down until v1.1 (seven-of-seven real-data bundle root) ships.
6. **Twelve-track rationale** — GOALS.md:56-57: maximum parallel workstreams a single steering function can hold.
7. **Decision-log as source-of-truth** — README.md:28, GOALS.md:120 delegate "why is this thing this way" to `cardano-wiki/wiki/log.md`.
8. **Mainnet date absent** — GOALS.md:60 projects 2029 testnet conditional on v1.1 in 2026; no top-level doc names a mainnet date.
