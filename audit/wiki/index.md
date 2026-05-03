# Audit Wiki Index

_Generated 2026-05-03 from a parallel six-agent audit of the `experiments` repo. Scope: observation only — no code, docs, or wiki pages outside `audit/wiki/` were modified._

### Synthesis
- [[00-synthesis]] — Cross-cutting strengths, risks, what's load-bearing, recommended next moves

### Domain audits
- [[01-top-level-docs]] — README, ARCHITECTURE, GOALS, RESEARCH-QUESTIONS, instructions, audit-prompt, LICENSE
- [[06-omega-commitment]] — Five-crate Rust workspace; v0.9.1; 248 tests; primitives, encodings, test architecture, risk
- [[03-cardano-wiki]] — 18-page knowledge base; schema; load-bearing pages; wiki ↔ implementation coupling
- [[04-skills-tooling]] — `manifest.toml`, `install.sh`, vendored `plonky3-friendly-rust` skill
- [[05-ci-operational]] — `.github/workflows/ci.yml`, scripts, build config, operational gaps

### Cross-reference
- [[02-prior-audit]] — Summary of the 2026-05-02 ten-agent Codex audit (A1–A10): 43 findings, 42 closed, 1 deferred (`A2/F001`)

### Maintenance
- See [[log]] for ingestion log
