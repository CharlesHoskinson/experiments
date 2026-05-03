---
agent: A8
lane: docs
title: wiki-coherence
files-reviewed: [cardano-wiki/SCHEMA.md, cardano-wiki/wiki/index.md, cardano-wiki/wiki/log.md, cardano-wiki/wiki/overview.md, cardano-wiki/wiki/pages/cardano-orgs.md, cardano-wiki/wiki/pages/cardano-repos.md, cardano-wiki/wiki/pages/cip-1694-governance.md, cardano-wiki/wiki/pages/eutxo-model.md, cardano-wiki/wiki/pages/hydra-scaling.md, cardano-wiki/wiki/pages/intersect-mbo.md, cardano-wiki/wiki/pages/ledger-state-json-layout.md, cardano-wiki/wiki/pages/leios-scaling.md, cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md, cardano-wiki/wiki/pages/midnight-sidechain.md, cardano-wiki/wiki/pages/mithril-certificates.md, cardano-wiki/wiki/pages/ouroboros-consensus.md, cardano-wiki/wiki/pages/plomin-hard-fork.md, cardano-wiki/wiki/pages/plutus-and-smart-contracts.md, cardano-wiki/wiki/pages/project-catalyst.md, cardano-wiki/wiki/pages/spec-ouroboros-omega.md, cardano-wiki/wiki/pages/voltaire-roadmap.md]
findings-count: { p0: 0, p1: 0, p2: 2, p3: 1 }
---

# Summary

All `[[slug]]` references I found resolve to existing `wiki/pages/` files, and `wiki/index.md` lists all 17 page files. I did not find a direct log-vs-page content contradiction or an obsolete page that needs a forward banner. The coherence gaps are schema drift: two newer ingestion pages do not satisfy required frontmatter/provenance shape, and the schema vocabulary no longer matches the current log/index.

# Findings

## F001 — Ingestion pages do not satisfy required frontmatter/provenance schema

- **Severity:** P2
- **Confidence:** high
- **Location:** `cardano-wiki/wiki/pages/ledger-state-json-layout.md:1-14`; `cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md:1-16`
- **Issue:** Both May 3 ingestion pages are under `wiki/pages/`, but their frontmatter ends without required `aliases` and `cssclass` fields. They also use structured `kind` provenance objects instead of the schema-required `source-slug -> one-line description` entries, so confidence cannot be mechanically justified from cited provenance in the same way as the rest of the wiki.
- **Evidence:**
  ```markdown
  Every wiki page in `wiki/pages/` must start with:
  ...
  aliases: [<alternate names>]
  cssclass: wiki-page
  ```
  ```markdown
  Each entry in `provenance` is: `source-slug -> one-line description of what this source contributed to the page`.
  ```
  ```yaml
  confidence: high
  provenance:
    - kind: live-measurement
      when: 2026-05-03
      artifact: /home/hoskinson/cardano/snapshots/ledger_state_20260502_235649.json (2.04 GiB)
      measured-by: omega-commitment-ingest/examples/probe_ledger_state_paths.rs
  created: 2026-05-03
  updated: 2026-05-03
  ---
  ```
  ```yaml
  confidence: medium
  provenance:
    - kind: in-tree-implementation
      when: 2026-05-03
      artifact: /home/hoskinson/omega-commitment/crates/omega-utxo-snapshot/
    - kind: live-discovery
      when: 2026-05-03
      artifact: failed cardano-cli --whole-utxo run at /home/hoskinson/cardano/logs/utxo_dump.log
  created: 2026-05-03
  updated: 2026-05-03
  ---
  ```
- **Suggested fix:** Add `aliases` and `cssclass: wiki-page` to both pages, and convert provenance to source-slug entries. For example, `ledger-state-json-layout` should cite separate slugs for the `cardano-cli` command and the live epoch-628 dump; `lsq-getutxowhole-pipeline` should cite slugs for pallas-network, cardano-cli, the PR, and the ledger source file rather than `kind` records.
- **Verification:** From repo root, `rg -n "^(aliases|cssclass):|  - .* -> |  - kind:" cardano-wiki/wiki/pages/ledger-state-json-layout.md cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md` should show `aliases`, `cssclass`, and `source -> claim` entries, with no `kind:` provenance entries.

## F002 — Schema operation and index-category vocabularies are stale

- **Severity:** P2
- **Confidence:** high
- **Location:** `cardano-wiki/SCHEMA.md:45-59`; `cardano-wiki/wiki/log.md:13-263`; `cardano-wiki/wiki/index.md:29-31`
- **Issue:** `SCHEMA.md` defines a closed-looking log operation list and index category list, but the current log and index use many values outside those lists. The log/index content is internally useful, but a schema-based lint or downstream consumer would reject the current wiki unless the schema is updated.
- **Evidence:**
  ```markdown
  Operations: init, ingest, query, update, lint, search, merge, export, research
  ```
  ```markdown
  ## [2026-05-01] spec | ouroboros-omega
  ## [2026-05-01] plan + execute | omega-commitment v0.3.0 (track T1, sub-tree 3 of 7)
  ## [2026-05-03] verify | LedgerState JSON paths confirmed live + RAM budget measured for stake/gov ingestion
  ## [2026-05-03] discovery | v1.0 architecture revised — split UTxO from stake/governance; UTxO needs custom LSQ client
  ```
  ```markdown
  ## Index Categories
  - Protocol & Consensus
  - Smart Contracts & Tooling
  - Scaling & Layer 2
  - Governance & Voltaire
  - Ecosystem & Organizations
  - Research & Roadmap
  - Repositories
  - Maintenance
  ```
  ```markdown
  ### Mainnet Ingestion (omega-commitment v1.0)
  - [[ledger-state-json-layout]] — JSON paths + verified entity counts for stake/governance sub-trees _(ingested 2026-05-03)_
  - [[lsq-getutxowhole-pipeline]] — `omega-utxo-snapshot` binary + why cardano-cli `--whole-utxo` fails on mainnet _(ingested 2026-05-03)_
  ```
- **Suggested fix:** Because `log.md` is append-only, update `SCHEMA.md` to include the operations now in use (`spec`, `plan`, `execute`, `plan + execute`, `decision`, `artifact`, `codex-audit`, `infra`, `verify`, `discovery`) and add the `Mainnet Ingestion (omega-commitment v1.0)` category, or explicitly document that these lists are examples rather than allowed values.
- **Verification:** From repo root, compare `rg -n "^Operations:|^## \\[[0-9]{4}-[0-9]{2}-[0-9]{2}\\]" cardano-wiki/SCHEMA.md cardano-wiki/wiki/log.md` and `rg -n "^## Index Categories|^### " cardano-wiki/SCHEMA.md cardano-wiki/wiki/index.md`; every log operation and index heading should be represented or the schema should say the list is non-exhaustive.

## F003 — Omega spec page hardcodes the non-repo wiki path

- **Severity:** P3
- **Confidence:** high
- **Location:** `cardano-wiki/wiki/pages/spec-ouroboros-omega.md:19-31`; `cardano-wiki/SCHEMA.md:4`
- **Issue:** The Omega spec page and schema hardcode `/home/hoskinson/cardano-wiki`, while this audit checkout is rooted at `/home/hoskinson/experiments` and the log uses repo-relative `docs/superpowers/...` references for the same artifacts. This makes the page local-machine-specific and ambiguous when there are multiple wiki checkouts.
- **Evidence:**
  ```markdown
  - **Path:** /home/hoskinson/cardano-wiki
  ```
  ```markdown
  **Full design doc:** `/home/hoskinson/cardano-wiki/docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md`
  ```
  ```markdown
  9. **Dual-hash:** selective — bundle root is a `(blake2b, sha3)` tuple; per-leaf and per-sub-tree are Blake2b-only. Decision doc: `/home/hoskinson/cardano-wiki/docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md`
  ```
  The append-only log uses repo-relative references for the same class of artifacts:
  ```markdown
  - Spec: docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md
  - Decision: docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md
  ```
- **Suggested fix:** Replace hardcoded `/home/hoskinson/cardano-wiki/...` wiki references with repo-relative or wiki-root-relative paths such as `docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md`, and update the schema identity path to avoid pinning a user-specific checkout unless that is intentional.
- **Verification:** From repo root, `rg -n "/home/hoskinson/cardano-wiki" cardano-wiki/SCHEMA.md cardano-wiki/wiki/pages/spec-ouroboros-omega.md` should return no matches after the path normalization.
