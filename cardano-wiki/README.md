# cardano-wiki

> **Parent frame:** this is a subdir of [`../`](../) (the experiments repo). The program-level README is at [`../README.md`](../README.md), the architecture deep-dive at [`../ARCHITECTURE.md`](../ARCHITECTURE.md), and the program goals at [`../GOALS.md`](../GOALS.md).

An LLM-maintained research wiki on Cardano. Two functions:

1. **Domain reference** — pages on Ouroboros consensus, EUTXO, Plutus, Hydra, Mithril, Leios, CIP-1694 governance, Voltaire, Plomin hard fork, Intersect MBO, repos, and key ecosystem orgs. These are evolving syntheses, not snapshots.

2. **Program scratch space for Ouroboros Omega (T1, the work in `../omega-commitment/`)** — design specs, implementation plans, codex audit briefings, decision log, and discovery pages produced as the work proceeds.

## How to read

- **Start here:** [`wiki/index.md`](wiki/index.md) — categorized table of contents
- **Living synthesis:** [`wiki/overview.md`](wiki/overview.md)
- **Decision log (append-only timeline):** [`wiki/log.md`](wiki/log.md)
- **Pages (flat namespace, hyphen-slugged):** [`wiki/pages/`](wiki/pages/)
- **Implementation plans:** [`docs/superpowers/plans/`](docs/superpowers/plans/)
- **Codex audit-handoff briefings:** [`docs/codex_briefings/`](docs/codex_briefings/)

## Most important pages right now

| Page | What it documents |
|---|---|
| [`wiki/pages/spec-ouroboros-omega.md`](wiki/pages/spec-ouroboros-omega.md) | The Omega program design spec |
| [`wiki/pages/ledger-state-json-layout.md`](wiki/pages/ledger-state-json-layout.md) | Verified JSON paths for stake + governance ingestion |
| [`wiki/pages/lsq-getutxowhole-pipeline.md`](wiki/pages/lsq-getutxowhole-pipeline.md) | Why the cardano-cli `--whole-utxo` path doesn't work + what we built instead |
| [`docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`](docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md) | The v1.0 plan (read the "REVISION 2026-05-03" section first) |
| [`docs/codex_briefings/2026-05-03-omega-codex-pipeline-update-brief.md`](docs/codex_briefings/2026-05-03-omega-codex-pipeline-update-brief.md) | Latest audit-handoff brief for cross-LLM review |

## Conventions

- **Frontmatter on every page** with `title`, `slug`, `tags`, `sources`, `confidence` (low / medium / high), `provenance`, `created`, `updated`. See [`SCHEMA.md`](SCHEMA.md).
- **Bidirectional links** via `[[slug]]` syntax — added by the wiki-ingest workflow whenever a new page is written.
- **No subdirectories** under `wiki/pages/` — flat namespace, slug-uniqueness enforces clarity.
- **Source provenance** — each page lists what external sources informed it; raw source files were pruned from this snapshot.

## License

Apache-2.0 unless individual source files indicate otherwise.
