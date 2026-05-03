# Synthesis: Cross-Cutting Themes

_Written 2026-05-03 after parallel six-agent audit. Domain pages: `01-top-level-docs`, `02-prior-audit`, `03-cardano-wiki`, `04-skills-tooling`, `05-ci-operational`, `06-omega-commitment`._

## What this repo actually is

`CharlesHoskinson/experiments` is the working space for **track T1** of a twelve-track program called **Ouroboros Omega** — a clean-slate post-quantum redesign of Cardano with cryptographic continuity to every prior era. Two artifacts are co-located here: a Rust workspace producing the **Ω-Commitment** (a single hash committing to seven sub-trees of pre-fork Cardano state) and a markdown research wiki feeding the design.

The repo is the **gating dependency** for T6 (verifier circuit), T7 (bridge), T8 (tooling). T1 is the only critical-path code in-tree today; everything else is research.

## Cross-cutting strengths

1. **Tight wiki ↔ implementation coupling.** `cardano-wiki/wiki/log.md` records every workspace decision and every audit pass with file:line citations into the design. Knowledge stays in one place; provenance survives.
2. **Audit-driven hardening worked.** The 2026-05-02 ten-agent Codex audit produced 43 findings; 42 closed across five batches (`Batch 1-5` in `audit/RESOLUTION.md`) over ~24 hours, raising test count 248 → 282 with no regressions. The single deferred P1 (`A2/F001`, real-mainnet UTxO CBOR decoder) is tracked at the call site.
3. **Cryptographic foundations look right.** v1 domain separation (`omega:v1:leaf`, `omega:v1:node`) bound into preimages; padding leaves use a sentinel index so verifiers reject padding-leaf inclusion proofs; duplicate raw payloads rejected at the tree builder; trailing CBOR bytes rejected at all five ingest paths.
4. **Documentation discipline.** Four root docs at "decreasing levels of abstraction" (README → ARCHITECTURE → GOALS → RESEARCH-QUESTIONS) cross-reference cleanly; track tables align; threat-model citations consistent (Mt Gox, Steem-to-Hive, dust long-tail).

## Cross-cutting risks

1. **Real mainnet ingestion is the critical-path gap.** v0.9.1 ships only synthetic fixtures. The `cardano-cli --whole-utxo` Word16-VLE bug (`IntersectMBO/cardano-cli#1350`) forced a 2026-05-03 architectural pivot to a two-stream model: `dump_ledger_state.sh` for stake/governance + `omega-utxo-snapshot` (pallas-network LSQ) for UTXO. The decoder for the LSQ response is still a TODO at `omega-utxo-snapshot/src/main.rs:202`. T1 cannot ship v1.0 without it.
2. **SHA3 bundle root is drift-detection only.** Both roots aggregate over the same Blake2b leaf hashes; the SHA3 root does not independently protect against a Blake2b break. Documented (`bundle.rs:7-11`, `ARCHITECTURE.md:9`); a truly-independent SHA3 tree is deferred to v2.0. **Operators must understand this is not a break-hedge.**
3. **PQ-VRF gap blocks T2 entirely.** RESEARCH-QUESTIONS.md Q1: no published post-quantum VRF construction meets Praos uniqueness. Q1 gates Crypsinous, Chronos, and Minotaur. 6-12 month research window. T1 work continues independently, but the consensus track stalls until this lands.
4. **CI is minimal.** Single workflow runs fmt/clippy/test on push/PR. No `cargo-audit`, no proptest invocation in CI, no benchmark baselines, no fuzz, no Kani, no SBOM, no release automation. The skills manifest installs all the verification tooling locally (`cargo-fuzz`, `cargo-mutants`, `kani-verifier`, `cargo-nextest`); none of it runs in CI.
5. **Skill installer fragility on Windows.** Bash-only installer; CRLF line endings broke initial run on this machine. Plugin activation gap (`/plugin marketplace add` cannot be auto-run) means users may miss the two superpowers/wiki-skills plugins. Upstream skills are pinned to `--depth 1` HEAD with no commit pinning.
6. **Schema drift in cardano-wiki.** SCHEMA.md defines closed-list vocabularies for log operations and index categories; both have grown beyond the schema's lists. Prior audit A8/F002 raised this; pages now use a broader vocabulary; schema not yet rewritten.
7. **External binaries unverified.** `download_snapshot.sh` and the headless-node setup install `cardano-node` / `mithril-client` without checksum or signature verification. Marked debug-only by prior audit (A10/F003-F004); no production-grade path documented.

## What's load-bearing

| Artifact | Why it matters |
|---|---|
| `core::tree::leaf_hash_v1` / `node_hash_v1` | Every per-sub-tree root and the bundle root depend on this domain-separated construction. A bug here invalidates all commitments. |
| `omega-utxo-snapshot` (incl. unfinished decoder) | The only path to mainnet UTXO ingestion; v1.0 release blocker. |
| `cardano-wiki/wiki/log.md` | Source-of-truth for every decision; load-bearing for "why is this thing this way." |
| `audit/RESOLUTION.md` | Captures audit closure trail; auditors and reviewers need this to verify findings actually closed. |
| `skills/local/plonky3-friendly-rust/SKILL.md` | Encodes the patterns T6 verifier-circuit work depends on; only repo-vendored skill. |

## Recommended next moves

This audit is observation-only; recommendations are illustrative not prescriptive.

1. **Land A2/F001** — typed `GetUTxOWhole` decoder unblocks v1.0.
2. **Add `cargo-audit` to CI** — supply-chain hygiene; one job, low cost.
3. **Fix SCHEMA.md drift** — either widen the closed lists in `cardano-wiki/wiki/SCHEMA.md` or normalize logged operations back to schema vocabulary.
4. **Add a `--commit-pin` mode to skills installer** — record installed upstream commit SHAs to `~/.claude/skills/.installed.toml` for reproducibility.
5. **Promote one-or-two A4/F003 fixtures into v0.x** — Byron + pointer addresses would catch the address-encoding regression class without waiting for v1.1.
6. **Document Cargo.lock policy** — current state ambiguous; either commit `Cargo.lock` (binary workspace) or document why it is excluded.

## Audit boundary

This audit is observation-only. No code, docs, or wiki pages were modified. All findings are written to `audit/wiki/` to coexist with the prior audit's `audit/findings/` and `audit/SUMMARY.md`. The prior audit closed 42/43 P0/P1/P2/P3 findings; this pass adds wiki-style cross-domain analysis on top of that closed surface.
