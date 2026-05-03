# Program goals — Ouroboros Omega

This document is the single-source-of-truth for what the program is trying to accomplish, organized by horizon. Companion documents: [`README.md`](./README.md) frames the program for first-time readers, [`ARCHITECTURE.md`](./ARCHITECTURE.md) is the deep-dive on the Ω-Commitment construction and the v1.0 ingestion pipeline.

## Why

Cardano shipped in 2017 against an elliptic-curve cryptographic stack that was state-of-the-art at the time and is now on a deadline. Ed25519 for ordinary signatures. Praos VRF for slot-leader election, built over Curve25519. KES for forging keys, built over the same curve family. BLS12-381 underneath Mithril certificates. Each primitive was chosen for excellent reasons given the engineering and threat-model context of the late 2010s. Each is definitively breakable by a sufficiently large quantum computer, and the deadline for "sufficiently large" is no longer a research question with squishy answers in the 2050s.

NIST finalized its first batch of post-quantum standards in 2024. Operational target dates inside national-security agencies for finishing PQ migrations now sit between 2030 and 2035. Industry timelines are slower but tracking the same arc. The standard reasoning ("well, even if a CRQC arrives in 2035, we will have warning, we can migrate then") is wrong for two reasons. First, the migration window for a chain with persistent state is the entire history of the chain, not just the moment the CRQC appears: any signature ever produced over the lifetime of Cardano remains forgeable retroactively if the curve breaks. Second, migration takes years even when nobody is shooting at you.

The question is not whether Cardano needs to migrate. It is what the migration looks like. Two honest answers: in-place migration (layer hash-based or lattice-based primitives over the existing stack and manage the compatibility tax forever) or clean-slate fork (build the new chain you would have built in 2017 if you had known what you know now, with a one-way bridge so existing holders are not stranded). I have come around to the second answer over the last six months. The argument is not that incremental migration is impossible. It is that incremental migration produces a worse final design and never quite finishes, because every transition window between primitives demands its own coordination ritual and accumulates technical debt the new design pays forever.

The trickiest part of any clean-slate fork is the existing state. Cardano has roughly ten million UTxOs, 2.5 million stake credentials, 2,940 stake pools, a thousand DReps, and eight years of block history. None of that should disappear when the new chain starts; people built on it. None of it should sit in Omega's genesis ledger either, pre-loaded and ready to be re-validated by Omega's nodes. That would force every Omega validator to carry the entire historical weight of Cardano forever, which is the burden the redesign was meant to shed. Commit to the old state cryptographically, then resurrect it lazily as holders bother to claim it.

The Ω-Commitment is the cryptographic commitment that makes lazy resurrection work. The work in this repository is the tooling that builds it from real Cardano mainnet data and produces regression-tested per-sub-tree roots and a dual-hash bundle root. Track T1 of twelve. The other eleven tracks are downstream or parallel: consensus, smart-contract VM, network stack, storage, on-chain verifier circuit, bridge protocol, wallet tooling, formal spec, audits, testnet operations, and mainnet launch. T1 is the smallest track by line count and the most consequential by gating effect: nothing else can lock down its design until the commitment format is stable.

## What

A new chain ("Omega") with these design properties.

All primitives post-quantum. No curve signatures, no curve VRF, no curve mining, no BLS multisigs. The signature scheme is hash-based, probably SPHINCS+ or its successor in the next standardization round. The VRF is a hash-based construction proven inside a STARK rather than a curve-based VRF that would require a curve gadget. The forging-key scheme is hash-based and rotates more aggressively than KES does, because hash signatures have larger keys and the operational cost of rotation is small. Mithril, if Omega adopts something analogous, gets a hash-based threshold scheme rather than BLS aggregation.

Plonky3-friendly state model. Every per-block state transition expressible as STARK constraints without specialized cryptographic gadgets. Two motivations: cheap off-chain verification (light clients, rollups, exchanges that want to verify settlement without a full node), and a cheap on-chain verifier for bridge claims. The cost is that some abstractions Cardano's ledger uses for free become explicit constraints the prover must satisfy. The CEK machine for Plutus, for example, is not plonky3-friendly and would need to be replaced with something like a JOLT-flavored RISC-style VM compiled to STARK constraints.

One-way bridge from Cardano. Each pre-fork holder can submit a `claim_*` transaction with a ZK Merkle proof against the published Ω-Commitment to resurrect UTxOs, stake position, governance role, native token policies, or scripts. The bridge is one-way by design. Omega and Cardano are independent chains after migration. No live cross-chain interoperability protocol on the roadmap. No ongoing two-way peg. A clean break with a transition window, not a permanent linkage.

Lazy state migration. Omega's genesis ledger is essentially empty: a small set of bootstrap parameters, the first-epoch validator keys, and the Ω-Commitment root. Everything else exists only as a Merkle leaf inside the commitment until somebody claims it. State is pulled forward by holders, not pushed forward by validators. The long tail of dust addresses in the Cardano UTxO set simply does not migrate, because the cost to claim them exceeds their value, and that is an acceptable outcome rather than a problem to solve.

Selective dual-track at the bundle layer. The Ω-Commitment is a `(blake2b_root, sha3_root)` tuple. If one hash function falls to a future preimage attack, the other still anchors the commitment, and the protocol can hard-fork to require Merkle paths against the other root without invalidating the snapshot. The dual-hash applies only at the bundle layer; per-leaf and per-sub-tree-root computations are Blake2b only. The cost of the second hash at the bundle layer is 32 extra bytes of output and one extra hash over a 224-byte concatenation, negligible against the value of the hedge.

## Tracks

| # | Track | Scope | Status |
|---|---|---|---|
| **T1** | **Commitment tooling** | **Build the Ω-Commitment from real mainnet data; produce regression-tested per-sub-tree roots and the dual-hash bundle root** | **In progress (this repo, v0.9.1)** |
| T2 | Consensus | PQ Praos design + reference implementation | Spec drafting |
| T3 | Smart-contract VM | Plutus-equivalent, plonky3-native execution model | Spec drafting |
| T4 | Network stack | Ouroboros networking miniprotocols, PQ-handshake variants | Not started |
| T5 | Storage / state management | UTxO storage + block storage adapted to plonky3 friendliness | Not started |
| T6 | ZK verifier | Plonky3 integration; on-chain verifier for claim proofs | Not started |
| T7 | Bridge protocol | Claim-transaction format, replay-attack resistance, fee model | Spec drafting |
| T8 | Tooling + cli | Wallet primitives, claim helpers, devtools | Not started |
| T9 | Documentation + spec | Whitepaper, formal protocol spec | T1 spec only |
| T10 | Audits + formal verification | Third-party audit, machine-checked proofs of critical invariants | Not started |
| T11 | Test-network operations | Devnet → testnet → public testnet | Not started |
| T12 | Mainnet operations | Genesis ceremony, key rollout, validator onboarding | Not started |

The track decomposition is the result of a brainstorming pass in early 2026. Twelve tracks rather than six big ones or twenty small ones is partly aesthetic and partly operational. Twelve maps reasonably onto the staffing pattern: most tracks need one or two strong leads plus a small implementation team, and twelve is the maximum number of parallel workstreams a single steering function can hold in its head. Six would have meant several tracks bundling unrelated work; twenty would have meant tracks too small to justify their own coordination overhead.

The dependency graph is not flat. T1 (this repo) gates T6 (verifier) and T7 (bridge), because both need a stable commitment format. T6 and T7 in turn gate T8 (tooling), because wallets need a verifier and a protocol to construct against. T2, T3, and T4 can proceed largely in parallel under the shared assumption of post-quantum primitives. T5 is mostly downstream of T2 and T3 settling. T9 through T12 depend on the technical tracks being design-locked.

T1 is in implementation, T2 / T3 / T7 are in spec drafting, the rest are not started. T1 ships a stable commitment format with v1.1 (when the chain-follower lands and the seven-of-seven mainnet bundle root is captured), at which point T6 can begin in earnest. If T1's v1.1 lands within 2026, the program is on track for a 2028 testnet. Aggressive but plausible.

## T1 sub-goals

### v0.x.0 series — synthetic ingestion (DONE)

Build the commitment data structure end-to-end against hand-crafted CBOR fixtures. The goal at this stage was not to handle real mainnet data; it was to prove that the leaf-encoding to sub-tree-root to bundle-root pipeline is deterministic, regression-detectable, and computes per-sub-tree roots that change when and only when the input changes. v0.9.1 ships this with 248 tests and pinned golden vectors at three layers (per-leaf encoding, per-sub-tree root, bundle root tuple).

The v0.x.x series produced a Codex audit pass that found four issues, all closed in v0.9.1. The most consequential was a UTxO sub-tree bug where the v0.9.0 parser correctly parsed native token bundles then discarded them before populating the leaf, producing leaves that did not match the spec's "claim_utxo resurrects ADA + native tokens" contract. The fix was to thread the asset bundles through to the leaf and re-pin the affected golden vectors. This kind of regression is what the three-layer golden vector setup is designed to catch, and it caught itself once the audit prompted a closer look.

### v1.0 — real mainnet ingestion for 5 sub-trees (IN PROGRESS)

Replace the synthetic CBOR fixtures with real mainnet data via the two-stream pipeline: `omega-utxo-snapshot` for utxo, token-policy, and script sub-trees; `cardano-cli conway query ledger-state` JSON for stake and governance. Pin the resulting "5 real + 2 placeholder" bundle root tuple as a real-data golden vector at a chosen mainnet epoch boundary. The first artifact an external party could in principle reproduce against the same Cardano snapshot.

The v1.0 plan is in [`cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`](./cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md). The 2026-05-03 architecture revision at the top of that document supersedes the original single-CBOR-dump model with the two-stream pipeline. The full task list is also in [`README.md`](./README.md) under the "To do" section.

### v1.1 — chain-follower for the remaining 2 sub-trees

The header chain and transaction index sub-trees cannot be derived from a snapshot alone. They require walking every block from genesis (or from a Mithril-restored recovery point) to the chosen tip. The v1.1 plan is in [`cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.1-chain-follower-plan.md`](./cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.1-chain-follower-plan.md) and breaks the work into twelve tasks centered on a `pallas-network` chain-sync miniprotocol client that streams blocks, decodes per-block headers and transactions, writes NDJSON checkpoints, and postprocesses into the per-sub-tree input format.

The capstone of v1.1 is a complete seven-of-seven mainnet bundle root tuple captured at the same epoch boundary as v1.0's anchor. This replaces the v1.0 "5 real + 2 placeholder" intermediate and is the first artifact that captures the entire pre-fork Cardano state in the published commitment format. Everything downstream of T1 depends on this artifact existing.

### v2.0 — formal-verification-grade reproducibility

A second independent implementation of the Ω-Commitment construction (probably Haskell, possibly Lean) consumes the same mainnet snapshot as v1.1 and produces byte-identical roots. Cross-implementation agreement is the audit precondition for using the commitment in Omega's genesis block. A single implementation, no matter how well tested, has unobservable shared assumptions with itself; only an independent re-derivation provides the kind of evidence that justifies betting an entire chain on the result.

The candidate languages are Haskell (because most of cardano-ledger is already Haskell, so the second team can borrow its type infrastructure) and Lean (because Lean 4 is mature enough now to carry both an executable specification and machine-checked proofs of the invariants we care about). The choice is open. The work cannot start until v1.1 is done because the spec is not stable until then.

## Non-goals

Reproducing Cardano's full state on Omega. The Cardano chain state is the source-of-truth, and Omega only commits to it. Pre-loading Omega's genesis ledger with the entire Cardano state would defeat the redesign. Lazy resurrection is the design.

Verifying Cardano consensus from Omega. Omega trusts the snapshot. Snapshot correctness is established by Mithril certificates over the input data, plus the cross-implementation reproducibility check that gates the genesis ceremony. Re-running Cardano consensus inside Omega's verifier would be extraordinarily expensive (eight years of Praos history to re-derive) and an unnecessary trust assumption.

Live cross-chain interoperability. A one-way migration bridge, not a two-way bridge. After migration, Cardano and Omega are independent chains. No ongoing peg, no synchronized state machine, no shared liquidity protocol. Anyone who wants to operate across both chains operates two wallets. This is a feature: the two designs can evolve independently without coordination overhead, and the bridge protocol does not need to defend against rollback attacks on the source chain.

Backwards compatibility with Cardano addresses, scripts, or transaction formats on Omega. Omega has its own primitives. Address formats differ (different signature schemes, different hash function choices). Script formats differ (the Plutus CEK machine is not portable to a STARK-friendly VM). Transaction formats differ (different fee model, different witness format). The bridge is the only continuity layer. Within the bridge protocol, an Omega claim verifier knows how to interpret Cardano-era addresses, scripts, and credentials for the purpose of validating a claim, but everything that lives natively on Omega uses Omega's own primitive set.

Building Omega in this repository. The work in `omega-commitment/` is the tooling that produces the commitment that Omega's genesis block will pin. It is not the chain itself. The chain implementation lives in tracks T2 through T5, which are not started in code (only spec). This repository is small and stays small. Intentional: the commitment format is the most consequential artifact in the program, and concentrating on it without distraction is the right discipline at this stage.

## How decisions are recorded

All material design decisions are written into the wiki at [`cardano-wiki/wiki/log.md`](./cardano-wiki/wiki/log.md) (append-only, dated, one entry per decision or discovery) and promoted to pages under `cardano-wiki/wiki/pages/` when they need standalone reference. The log is the source-of-truth for "why is this thing the way it is" questions. If a decision is not in the log, it is not a decision; it is an unexamined default.

Implementation plans live under [`cardano-wiki/docs/superpowers/plans/`](./cardano-wiki/docs/superpowers/plans/). Each plan has a date in its filename and a "REVISION YYYY-MM-DD" section at the top whenever it has been substantively reworked. The active plans are the v1.0 real-mainnet ingestion plan (2026-05-01, with the 2026-05-03 revision) and the v1.1 chain-follower plan (2026-05-01, no revision yet). Plans are written in a format executable by a coding agent, but a human can read them.

Audit-handoff briefings live under [`cardano-wiki/docs/codex_briefings/`](./cardano-wiki/docs/codex_briefings/). Each is a self-contained document for an LLM (Codex, currently) to do an autonomous audit pass. The briefing format includes the project context, the current state of the code, the recent decisions that affect the audit, and the specific things the auditor should look at. The 2026-05-03 brief is current; the 2026-05-01 brief has a "PARTIALLY SUPERSEDED" banner pointing forward.

The decision log compounds. Reading the last three or four entries gives you the current state of play more efficiently than any other artifact in the repository. The log is also where mistakes get recorded honestly: the 2026-05-03 entry on the architecture revision flags the specific premature conclusions I made earlier in the day and corrects them. Recording the mistake is more useful than papering over it because the mistake itself is informative.

The wiki and plan documents are not separable from the code. Reading the code without the wiki leaves you guessing why decisions went the way they did. Reading the wiki without the code leaves you with a beautiful design that may or may not match what got implemented. Both together give you the program. That is why both are in this repository, and why both are linked from [`README.md`](./README.md).
