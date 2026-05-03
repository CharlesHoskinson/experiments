# cardano-wiki Audit

_Source: parallel agent pass, 2026-05-03. Files audited: `cardano-wiki/wiki/{index,overview,log,SCHEMA}.md`, `cardano-wiki/wiki/pages/*.md` (18 pages), `docs/superpowers/`, `docs/codex_briefings/`._

## Scope and taxonomy

Five interlocking domains across 18 pages, plus index/overview/log:

1. **Protocol & Consensus** (4): `ouroboros-consensus`, `eutxo-model`.
2. **Smart Contracts & Tooling** (1): `plutus-and-smart-contracts`.
3. **Scaling & Layer 2** (4): `hydra-scaling`, `mithril-certificates`, `leios-scaling`, `midnight-sidechain`.
4. **Governance & Voltaire** (4): `cip-1694-governance`, `plomin-hard-fork`, `voltaire-roadmap`, `intersect-mbo`.
5. **Ecosystem & Orgs** (3): `cardano-orgs`, `project-catalyst`, `cardano-repos`.
6. **Research & Roadmap** (1): `spec-ouroboros-omega`.
7. **Mainnet Ingestion (added 2026-05-03)** (2): `ledger-state-json-layout`, `lsq-getutxowhole-pipeline`.

## Organization and conventions

- `wiki/index.md` — category tree (7 sections); every page indexed.
- `wiki/log.md` — append-only event log, 33 entries (2026-05-01 → 2026-05-03). Operations include `plan`, `execute`, `spec`, `decision`, `artifact`, `codex-audit`, `infra`, `verify`, `discovery`, `resolve`, `audit-defer`.
- `wiki/overview.md` — synthesis across sources (updated 2026-05-01).
- Linking: `[[slug]]` style (SCHEMA.md:42-43); 52 link instances across 21 documents, no dangling refs.
- Frontmatter (SCHEMA.md:10-34): every page carries `slug`, `tags`, `sources`, `confidence`, `provenance` (source-slug → claim), `created`/`updated` dates, `aliases`, `cssclass: wiki-page`.
- Search index `wiki/.search-index.md` referenced in schema (auto-generated).

## Load-bearing vs background pages

**Core (load-bearing for design decisions):**
- `ouroboros-consensus` — Praos → Genesis → Chronos → Leios family.
- `eutxo-model` — why EUTXO differs from accounts; parallel validation; design patterns.
- `plutus-and-smart-contracts` — language families; off-chain libraries.
- `cip-1694-governance` — three voting bodies; seven action types; constitutional mechanism.
- `spec-ouroboros-omega` — nine locked architectural decisions for the new chain.

**Supporting context (encyclopedic):**
- `hydra-scaling`, `mithril-certificates`, `leios-scaling`, `midnight-sidechain`, `voltaire-roadmap`, `plomin-hard-fork`, `intersect-mbo`, `cardano-orgs`, `project-catalyst`, `cardano-repos`.

**Tightly coupled to the implementation workspace (added 2026-05-03):**
- `ledger-state-json-layout` — verified live counts: 1.47M accounts, 2,940 pools, 1,016 DReps, 15 gov proposals; RAM/file ratio 3.24x; parse wall 6.47s; peak VmHWM 6.46 GiB.
- `lsq-getutxowhole-pipeline` — root cause of `cardano-cli --whole-utxo` Word16-VLE bug (Address.hs:847 vs 348); PR `IntersectMBO/cardano-cli#1350`; `omega-utxo-snapshot` Rust binary as workaround.

## Coherence and source rigor

- **Cross-references**: bidirectional, e.g., `cip-1694-governance ↔ plomin-hard-fork ↔ intersect-mbo ↔ voltaire-roadmap`. `overview.md:32-42` forms a full-stack diagram.
- **Provenance**: 3-7 sources per page with explicit `source-slug → claim` mapping. `ouroboros-consensus.md:8-10` cites three sources to three distinct framing claims.
- **Confidence**: 13 pages `high`, 2 `medium` (e.g. lsq-getutxowhole-pipeline cites PR + wire-format verification), 1 `speculative` (spec-ouroboros-omega).
- **Evidence density**: ingestion pages give exact JSON paths, entity counts, RAM/file ratios, root-cause line numbers (Address.hs:847 vs 348).

## Wiki ↔ implementation relationship

The wiki is the **design knowledge base feeding the omega-commitment Rust workspace**. Concrete couplings:

- `spec-ouroboros-omega.md:19` links to `docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md` (306 lines, log.md:7).
- `ledger-state-json-layout.md:21` claims `stateBefore.esLState.delegationState.dstate.accounts: 1,474,666` — verified live 2026-05-03 by `omega-commitment-ingest/examples/probe_ledger_state_paths.rs` (log.md:250-268).
- `lsq-getutxowhole-pipeline.md:21-22` defines the v1.0 UTxO ingestion path: `omega-utxo-snapshot` binary + pallas-network 0.30.2 LSQ client; wire-format match verified (log.md:234-246).
- `wiki/log.md` documents every workspace decision and commit (e.g., v0.9.1 Codex audit fixes at log.md:214-222).
- Codex briefings (`docs/codex_briefings/2026-05-01-omega-codex-debug-brief.md`, log.md:180-186) serve as long-form synopses for autonomous agents.

## Notable observations

1. **Schema maturity** — `SCHEMA.md` (88 lines) explicitly documents identity, frontmatter, confidence levels, provenance format, search index, cross-references, log operations, index categories, Obsidian integration. Vocabulary marked non-exhaustive (lines 49, 63-71). _Note: prior audit A8/F002 flagged log/index using values outside SCHEMA's closed lists; pages now use a broader vocabulary, schema not yet rewritten._
2. **Mainnet coupling is recent and tight** — both ingestion pages created 2026-05-03 with measurements against epoch 628 mainnet (978 MB before decoder failure, 2.04 GiB after).
3. **Five-batch resolution trail** — log.md:282-324 documents the 2026-05-02 Codex audit and 2026-05-03 fix-through-resolve; 42/43 findings closed; A2/F001 deferred to v1.0 Task 4 with TODO at log.md:300.
4. **One archived page** — `archive-daedalus-setup.md` not in `index.md`. `git mv`'d from `omega-commitment/scripts/` (log.md:288) when the architecture revised away from Daedalus GUI to headless node (log.md:291).
5. **Open questions seeded in overview.md:60-74** — eight active 2026 research items; five open Cardano-design questions (governance differentiation, AI-native primitives, constitutional trade-offs, Hydra closed-party limit, Mithril under-deployment).
6. **Spec ↔ consensus coupling** — `spec-ouroboros-omega.md:47-54` cross-links all six parent topics; nine locked decisions trace back to log.md entries with file:line citations.
