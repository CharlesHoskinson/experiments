# Wiki Schema

## Identity
- **Path:** /home/hoskinson/cardano-wiki
- **Domain:** How Cardano works — protocol, scaling, governance, ecosystem, and active research directions. Built to support brainstorming radical ideas on top of Cardano.
- **Source types:** Cardano docs, IOG/IOHK research papers, CIPs, GitHub repositories, Intersect MBO publications, Cardano Forum threads, blog posts
- **Created:** 2026-05-01
- **Schema version:** 2

## Page Frontmatter
Every wiki page in `wiki/pages/` must start with:

---
title: <page title>
slug: <filename-without-md>
tags: [tag1, tag2]
sources: [source-slug1, source-slug2]
confidence: high | medium | low | speculative
provenance:
  - source-slug -> claim or fact derived from this source
created: YYYY-MM-DD
updated: YYYY-MM-DD
aliases: [<alternate names>]
cssclass: wiki-page
---

### Confidence Levels
- **high** — corroborated by 2+ independent sources
- **medium** — single reliable source, no contradictions
- **low** — single source, unverified or potentially outdated
- **speculative** — inference or synthesis, no direct source statement

### Provenance Format
Each entry in `provenance` is: `source-slug -> one-line description of what this source contributed to the page`. This creates an auditable chain from every claim back to its origin.

## Search Index
- File: `wiki/.search-index.md`
- Format: one line per page: `slug | title | tags | first-sentence-summary`
- Rebuilt by: `scripts/build-search-index.sh`
- Used by: wiki-search skill for fast lookup

## Cross-References
Use `[[slug]]` where slug = filename without `.md`.

## Log Entry Format
```
## [YYYY-MM-DD] <operation> | <title>
```
Operations: init, ingest, query, update, lint, search, merge, export, research

## Index Categories
- Protocol & Consensus
- Smart Contracts & Tooling
- Scaling & Layer 2
- Governance & Voltaire
- Ecosystem & Organizations
- Research & Roadmap
- Repositories
- Maintenance

## Obsidian Integration
- Obsidian compatibility: enabled
- aliases field: alternate page names for Obsidian search
- cssclass: wiki-page (for custom CSS styling)
- Dataview queries can use: tags, confidence, sources, updated, created
- Graph view: [[slug]] links render as graph edges

## Conventions
- raw/ is immutable — skills never modify it
- log.md is append-only — never rewritten, only appended
- index.md is updated on every operation that adds or changes pages
- All pages live flat in wiki/pages/ — no subdirectories
- overview.md reflects the current synthesis across all sources
- .search-index.md is a generated artifact — rebuilt by lint or on-demand
- Slugs are lowercase, hyphen-separated, no special characters
