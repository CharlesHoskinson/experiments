---
date: 2026-05-03
kind: design-doc
topic: README overhaul (post-audit, transaction-flow, what-is-being-proved)
status: drafted
---

# README overhaul — design doc

## Context

The repo's `README.md` (329 lines) and `ARCHITECTURE.md` (183 lines) were last comprehensively rewritten 2026-05-03 for the four-layer architecture. A six-agent fresh-pass audit (`audit/wiki/`) identified three gaps the README does not surface:

1. **No explicit transaction-flow model** — readers cannot trace a single UTxO from Cardano-side spend → snapshot → genesis publication → Omega-side `claim_utxo` → resurrected Starstream UTxO.
2. **No statement of what the Plonky3 verifier proves** — the verifier circuit is referenced repeatedly (T6, lazy resurrection) but its constraint set is never enumerated.
3. **Operational gaps invisible** — CI is minimal (fmt/clippy/test only); verification tooling installed locally but unused; skill installer fragile on Windows; SCHEMA.md drift in cardano-wiki. The audit raised these but the README does not.

The user also asked for design lessons learned from the audit and from the Scrapling-verified citation pass to be reflected.

## Decisions

### Scope
- Rewrite `README.md` end-to-end. Preserve the four-layer architectural narrative (it was correct); add three new sections; tighten everything else.
- Do not rewrite `ARCHITECTURE.md`. Its content is correct and detailed; the README links to it for depth.

### New sections (in order)
1. **Design lessons learned (2026-05-03)** — bullet list of post-audit reframings, drawing on `audit/wiki/00-synthesis.md`. Five entries: SHA3 reframing, domain-tag mechanics, two-stream pipeline pivot, audit closure trail, six-agent fresh-pass.
2. **Transaction flow: Cardano UTxO → Omega Starstream UTxO** — five-phase ASCII diagram (A pre-fork → B snapshot → C genesis → D claim → E steady state). Each phase says what's being computed and what trust boundary applies.
3. **What the Plonky3 verifier proves** — eight numbered constraints (C1-C8) the on-chain circuit checks for `claim_utxo`. Each line is one constraint. This is the canonical answer to "what stops a forged claim?"
4. **Operational gaps** — short subsection inside Status. Three items: CI minimal, skill installer fragility, schema drift. Each links to the relevant `audit/wiki/` page.

### Sections preserved (with edits)
- Two-artifact table at top (`omega-commitment/`, `cardano-wiki/`) — unchanged structurally.
- "What is Ouroboros Omega?" — kept; tightened from 4 dense paragraphs to 3.
- Four-layer post-quantum stack — kept; small tightening.
- No-backdoor stance — kept; tightened.
- 4-lane architecture diagram — kept; status box updated.
- Three trust boundaries — kept.
- Cryptographic flags (dual-hash, domain sep, nullifier) — kept but reorganized so SHA3-is-drift-detection appears earlier and bolder.
- Status as of 2026-05-03 — kept; one row updated for v1.0 Task 4 progress.
- Tracks T1-T12 — kept; moderate tightening.
- To-do (v1.0 / v1.1 / cross-cutting) — kept; moderate tightening.
- License — unchanged.

### Reference verification (Scrapling)
Reference URLs verified by `audit/wiki/fetch_refs.py` (Scrapling): all 12 cited URLs resolve 200 except `csrc.nist.gov/pubs/fips/206/ipd` (404 — FN-DSA draft URL has moved; footnote already says "forthcoming" so no action needed). PR #1350 title clarifies the fix is via a Cabal source-replace-package against `cardano-ledger`, not directly in `cardano-cli` — worth one sentence in the v1.0-pivot paragraph.

### What is NOT changing
- ARCHITECTURE.md, GOALS.md, RESEARCH-QUESTIONS.md — content is correct and load-bearing; not in scope.
- The Rust code, tests, and crate structure — observation-only audit; no code changes.
- The `cardano-wiki/` knowledge base — observation-only audit; no edits.
- The skills installer or skill set — already handled in the previous turn.

## Architecture diagrams

Two diagrams will live in the README, both ASCII (one continues to be the existing 4-lane stack diagram, the other is the new transaction-flow diagram). No external image assets.

The transaction-flow diagram is structured as five horizontal phases (A through E), each with one boxed cluster of state, computations, and trust-boundary annotations. The diagram explicitly labels:
- Where each piece of state lives (Cardano-side / off-chain / Omega-side).
- What the holder controls vs what the protocol controls.
- Which step burns the nullifier (Phase D).
- Which artifacts the genesis block pins (Phase C).

## Implementation notes

- Single overwrite of `README.md`. No phased rollout, no feature-flagging, no doc-staging directory.
- Apply `humanizer` skill as a final polish pass for AI-tells.
- Architecture diagrams use the same ASCII conventions already in the existing 4-lane diagram (box-drawing chars, vertical pipes, three-dash arrows).
- Cite `audit/wiki/00-synthesis.md` once in the new "Design lessons" section as the source of the synthesis.
- Verified-citation footnote covers FIPS-204, FIPS-205, FIPS-206; add a one-line "verified 2026-05-03 via Scrapling" mention in the citation footnote.

## Acceptance

- README opens with the same two-artifact table.
- New "Design lessons learned" section appears after "What is Ouroboros Omega?" and before the four-layer stack.
- New "Transaction flow" section appears after the four-layer stack and before the existing 4-lane architecture diagram.
- New "What the Plonky3 verifier proves" section appears immediately after the transaction flow.
- New "Operational gaps" subsection appears inside Status as of 2026-05-03.
- All existing internal links still resolve.
- All external citations verified by Scrapling pass.
- Length: ~500 lines (vs prior 329). The new material lands cleanly at +50%, not +200%.
