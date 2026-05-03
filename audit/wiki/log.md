# Audit Wiki Log

Append-only. Format: `## [YYYY-MM-DD] <operation> | <title>`

---

## [2026-05-03] init | audit-wiki

- Created `audit/wiki/` to hold the output of a fresh-pass audit, complementing `audit/findings/` (2026-05-02 Codex audit) and `audit/SUMMARY.md` / `RESOLUTION.md` (closure trail).

## [2026-05-03] audit | six-agent parallel audit pass

- Method: six parallel `Explore` agents, each with narrow scope + fixed output schema.
- Scope split: top-level docs · omega-commitment workspace · cardano-wiki · prior audit · skills tooling · CI/operational.
- Output: six domain pages + one synthesis page under `audit/wiki/pages/`.
- Boundary: observation only — no code, docs, or wiki pages outside `audit/wiki/` were modified.
- Headline findings:
  - 42 of 43 prior-audit findings closed; one P1 deferred (`A2/F001`, real mainnet `GetUTxOWhole` decoder; TODO at `omega-utxo-snapshot/src/main.rs:202`).
  - v0.9.1 (282 tests) is consistent with the 2026-05-03 spec; deltas are additive (chunked anchoring, mass-MPC tooling, Lean reference impl, guardrails script).
  - Critical-path gap: typed CBOR decoder for `GetUTxOWhole` LSQ response unblocks v1.0.
  - Q1 (PQ-VRF) gates T2 entirely — 6–12 month research window.
  - CI runs only fmt/clippy/test; verification tooling installed locally but not exercised in CI.
  - SHA3 bundle root is drift-detection only, not a Blake2b break-hedge — documented in code and in `ARCHITECTURE.md:9`, but easy to misread.
  - `cardano-wiki/SCHEMA.md` closed-list vocabularies have drifted relative to actual log/index usage; prior audit A8/F002 raised this.
- See [[index]] for navigation.
