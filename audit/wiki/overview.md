# Audit Wiki Overview

This wiki holds the output of a 2026-05-03 fresh-pass audit of the `experiments` repo. It complements the existing `audit/findings/` directory (the 2026-05-02 ten-agent Codex audit and its five-batch resolution trail) by adding cross-domain wiki-style analysis at a higher altitude.

## Method

Six parallel `Explore` agents were dispatched, each with a narrow scope and a fixed output schema:

| Agent | Scope |
|---|---|
| Top-level docs | `README.md`, `ARCHITECTURE.md`, `GOALS.md`, `RESEARCH-QUESTIONS.md`, `instructions.md`, `audit-prompt.md`, `LICENSE` |
| omega-commitment workspace | Five Rust crates, workspace `Cargo.toml`, tests |
| cardano-wiki | 18 pages + index/overview/log/SCHEMA |
| Prior audit | `audit/SUMMARY.md`, `audit/RESOLUTION.md`, `audit/findings/A1–A10` |
| Skills tooling | `skills/manifest.toml`, `install.sh`, `local/plonky3-friendly-rust` |
| CI / operational | `.github/workflows/`, `omega-commitment/scripts/`, toolchain + cargo config |

Each agent returned a single markdown wiki page with file:line citations. The synthesis page ([[00-synthesis]]) was written after all six returned.

## Key findings at a glance

- **Observation only.** No code or docs changed. All output is under `audit/wiki/`.
- **Prior audit is well-closed.** 42 of 43 findings closed across five batches; one P1 deferred (real mainnet UTxO CBOR decoder, tracked at the call site).
- **Critical-path gap.** v1.0 cannot ship until `omega-utxo-snapshot/src/main.rs:202` typed `GetUTxOWhole` decoder lands.
- **Documented but easy to miss.** SHA3 bundle root is **drift detection only**, not a Blake2b break-hedge. Operators must understand this.
- **Q1 (PQ-VRF) gates T2 entirely.** No published construction meets Praos uniqueness; 6-12 month research window.
- **CI is minimal.** Single workflow runs fmt/clippy/test. Verification tooling (cargo-fuzz, cargo-mutants, kani-verifier, cargo-nextest, proptest) installed locally; nothing runs in CI.

## Layout

```
audit/wiki/
├── index.md          this index
├── overview.md       this file
├── log.md            append-only audit-pass log
└── pages/
    ├── 00-synthesis.md        cross-cutting themes
    ├── 01-top-level-docs.md
    ├── 02-prior-audit.md
    ├── 03-cardano-wiki.md
    ├── 04-skills-tooling.md
    ├── 05-ci-operational.md
    └── 06-omega-commitment.md
```

## Boundary with the prior audit

| Artifact | Date | Source | Role |
|---|---|---|---|
| `audit/findings/A1–A10` | 2026-05-02 | 10-agent Codex audit | Granular findings table; closed 42/43 |
| `audit/SUMMARY.md` + `RESOLUTION.md` | 2026-05-03 | Synthesis + closure trail | What was found, what was fixed, what's deferred |
| `audit/wiki/` (this dir) | 2026-05-03 (later) | 6-agent fresh pass | Cross-domain narrative; observation only |

The two passes are complementary: the prior audit sliced **by lane** (crypto / CBOR / semantics / tests / idioms / LSQ / docs / wiki / plan / ops). This pass slices **by domain** (top-level docs / Rust / wiki / skills / CI / cross-reference) and steps back to look for cross-cutting themes.
