# Instructions: pre-push audit by a 10-agent Codex workforce

This document is the protocol for a deep pre-publication audit of this repository, run by GPT-5.5 / Codex with a workforce of ten specialised sub-agents. The repository combines a Rust workspace ([`omega-commitment/`](./omega-commitment/)) and an LLM-maintained research wiki ([`cardano-wiki/`](./cardano-wiki/)) that together implement track T1 (commitment tooling) of the Ouroboros Omega program. Before this work goes public on a wider scale, it gets a systematic adversarial review.

The audit produces a tree of findings under `audit/`, ranked by severity, with file-and-line citations and proposed fixes. The repository owner reads only the consolidated summary first and drills down into per-agent reports as needed.

## Why ten, why specialised

A single generalist auditor smears attention thinly across a heterogeneous artefact and misses the deep failures. The repository contains code that is cryptographic, code that is Cardano-semantics-sensitive, code that is Rust-idiomatic, prose documentation that must match the code, and a wiki whose internal coherence is its whole value. Each of these surfaces rewards a reviewer who has loaded only that surface into context.

Three to seven agents is the canonical sweet spot. Above seven, coordination overhead dominates unless the orchestration is hierarchical. This audit uses ten agents in three lanes (code, docs, cross-cutting) with Codex itself as the coordinating lead. The lead dispatches each agent with a tight scope, collects per-agent reports as discrete files, then synthesises a triaged summary. No agent sees another agent's output during its own pass.

## Agent roster

### Code lane (6 agents)

**A1 — Cryptographic correctness.** Reviews the Merkle tree construction, hash domain separation, leaf preimage canonicalisation, dual-hash placement at the bundle layer, and the seven sub-tree root computations. Reads [`omega-commitment/crates/omega-commitment-core/src/`](./omega-commitment/crates/omega-commitment-core/src/) end to end, with focused passes on `tree.rs`, `bundle.rs`, and every `*_leaf.rs`. Specifically must check: (a) is each leaf bound to `(sub_tree_id, leaf_index, payload)` before hashing or is it raw payload, (b) do leaf hashes and internal-node hashes use distinct domain tags, (c) is zero-padding handled in a way that an attacker cannot spoof an empty position, (d) does the dual-hash at the bundle layer actually defend against a single-hash break or is it cosmetic.

**A2 — CBOR and serialisation strictness.** Reviews every CBOR codec path: the encoder used to produce leaf preimages, the decoder used to ingest mainnet snapshots, and the helpers in [`crates/omega-commitment-ingest/src/cbor.rs`](./omega-commitment/crates/omega-commitment-ingest/src/cbor.rs). Specifically must check: (a) do all decoders reject trailing bytes via `cbor::expect_end`, (b) is the encoder canonical (deterministic length encoding, sorted map keys), (c) does the parser handle the variable-length-integer cases for pointer addresses and TxIx without truncation, (d) does the asset-bundle parser preserve non-UTF8 asset names byte-for-byte rather than going through a JSON intermediate that would mangle them.

**A3 — Cardano semantics fidelity.** Reviews the per-sub-tree mainnet parsers against the actual Cardano specifications. Reference materials: CDDL in `IntersectMBO/cardano-ledger`, CIP-19 (addresses), CIP-32 (inline datums), CIP-33 (reference scripts), CIP-1694 (governance), the Conway formal ledger spec. Specifically must check: (a) are Byron bootstrap addresses, pointer addresses, and base/enterprise/stake address variants all handled, (b) do UTXO leaves carry `datum_option` (inline data, not just hash) and `script_ref`, (c) does the stake leaf encode the Conway DRep sum type including AlwaysAbstain and AlwaysNoConfidence, (d) is the snapshot epoch boundary pinned with respect to the Mark/Set/Go rotation, (e) are AccountState (treasury, reserves, deposits, fee pot) committed somewhere or silently dropped.

**A4 — Test design and golden-vector quality.** Reviews [`omega-commitment/crates/*/tests/`](./omega-commitment/) and the three layers of golden vectors (per-leaf, per-sub-tree-root, bundle). Specifically must check: (a) what edge cases the test suite does not cover (empty trees, single-leaf trees, maximum-depth trees, malformed CBOR, non-UTF8 asset names, Byron addresses, pointer addresses, inline datums, reference scripts, AlwaysAbstain DReps), (b) whether property tests exist for the obvious algebraic properties (Merkle root determinism under permutation, leaf-encoding round-trip, hash domain separation), (c) whether the golden vectors lock the right things at the right granularity, (d) is there any path where a test asserts on `_` or `Ok(_)` and silently ignores a return value.

**A5 — Rust idioms, safety, panics, error handling.** Reviews the workspace for Rust-grade quality. Specifically must check: (a) any `unsafe` blocks (none expected, flag if found), (b) every `unwrap`/`expect`/`panic!` (each must be either obviously infallible with reasoning or a TODO), (c) error types per crate (one anyhow per binary, typed errors per library is the right pattern), (d) clippy-clean and rustfmt-clean status, (e) any `Cargo.toml` with version mismatches or unpinned major versions, (f) any `Cargo.lock` shipped that should not be (the workspace policy is "no lockfile, fresh resolution per consumer" — confirm).

**A6 — `omega-utxo-snapshot` LSQ correctness.** Reviews the new pallas-network LSQ binary at [`omega-commitment/crates/omega-utxo-snapshot/src/main.rs`](./omega-commitment/crates/omega-utxo-snapshot/src/main.rs). Specifically must check: (a) is the era index `6` actually Conway in pallas-network 0.30.2's `queries_v16` (cross-check against pallas's `protocols.rs` integration test), (b) is the `Acquire(None) → Query → Release → Done` handshake correctly ordered (release before done), (c) does the binary handle a node socket disconnect mid-query gracefully, (d) is the `AnyCbor` buffered-write strategy documented in the source comments and in `setup_headless_node.md`, (e) is there an obvious path to streaming if the buffered approach hits memory limits.

### Docs lane (3 agents)

**A7 — Top-level docs accuracy.** Reviews [`README.md`](./README.md), [`ARCHITECTURE.md`](./ARCHITECTURE.md), [`GOALS.md`](./GOALS.md), and the per-subdir READMEs. Specifically must check: (a) every numerical claim (entity counts, leaf-encoding sizes, file sizes, RAM ratios, signature sizes) against the code or the wiki page that pinned them, (b) every relative link resolves to a file that exists, (c) every code-fence command is runnable as shown (no missing flags, no obsolete subcommand syntax), (d) the ASCII architecture diagram in `README.md` matches the actual data flow in the code, (e) the To-Do list items in `README.md` match the v1.0/v1.1 plans.

**A8 — Wiki coherence.** Reviews [`cardano-wiki/wiki/`](./cardano-wiki/wiki/) for internal consistency. Specifically must check: (a) `[[slug]]` references resolve to existing pages, (b) every page has frontmatter with the required fields per `SCHEMA.md`, (c) `wiki/index.md` lists every page in `wiki/pages/`, (d) `wiki/log.md` entries do not contradict the page content they reference, (e) the `confidence` field on each page is justifiable from the cited provenance, (f) any pages that were superseded by later entries have a banner pointing forward.

**A9 — Plan and spec completeness vs code.** Reviews [`cardano-wiki/docs/superpowers/plans/`](./cardano-wiki/docs/superpowers/plans/) against the code that purports to implement them. Specifically must check: (a) every plan task marked DONE in the log has a corresponding code change visible in the workspace, (b) every plan task marked IN PROGRESS has explicit text describing the partial state, (c) any code that exists without a plan task is either pre-spec scaffolding (acceptable) or scope creep (flag), (d) the v1.0 plan's "REVISION 2026-05-03" supersedes the original plan body cleanly with no stale references, (e) the codex briefings under `docs/codex_briefings/` are mutually consistent and the older brief carries the supersession banner.

### Cross-cutting (1 agent)

**A10 — Operational, supply chain, secrets, licensing.** Reviews the whole tree from a release-readiness angle. Specifically must check: (a) `grep -RIn` across the repo for any private key material, API token, password, or credential pattern (none expected), (b) `LICENSE` matches the license declared in `Cargo.toml` (Apache-2.0), (c) every `Cargo.toml` dependency is pinned to a major.minor version and published to crates.io (no git deps, no path deps to outside the workspace), (d) any `*.cbor` or `*.json` fixture file under `crates/*/tests/fixtures/` is small (under 100 KB) and committed by intent, (e) what CI configuration exists (none expected at this stage; flag the absence and propose a minimal GitHub Actions workflow), (f) the `omega-commitment/var/snapshots/` directory is empty as expected, (g) no `.env`, `.envrc`, or shell-history file got copied in.

## Severity model

Each finding gets one of four severity tags.

**P0 blocker.** Must be fixed before publication. Examples: a leaf encoding that lets an attacker mint claims for non-existent state, a CBOR decoder that silently truncates, a test that asserts something false, a committed secret. P0 findings stop the push.

**P1 high.** Should be fixed before publication. Examples: missing domain separator in the Merkle leaf hash, a documented entity count that does not match the code, a `[[slug]]` link that 404s, a hardcoded path that does not exist. P1 findings stop the push absent a written justification.

**P2 medium.** Should be fixed in the next development cycle. Examples: a CBOR parser that handles the common case but not Byron edge cases, an unimplemented variant of an enum, missing property tests for an algebraic property, a confusing-but-correct prose passage. P2 findings ship as an open issue or TODO.

**P3 low.** Nice to fix. Examples: stylistic Rust improvements, prose tightening, dead-link in a non-critical doc. P3 findings ship without comment unless trivially fixable.

## Confidence model

Each finding gets a confidence rating: **high** (the reviewer can cite the exact line and the exact spec/CIP/RFC clause it violates), **medium** (the reviewer can cite the line but the spec interpretation is debatable), **low** (the reviewer suspects a problem but cannot point at the smoking gun). Low-confidence findings still ship; the lead triages them differently.

## Per-agent output format

Each agent writes one markdown file at `audit/findings/<agent-id>-<short-title>.md`. The file follows this schema.

```markdown
---
agent: A<N>
lane: <code|docs|cross>
title: <short-title>
files-reviewed: [<list of file paths>]
findings-count: { p0: <n>, p1: <n>, p2: <n>, p3: <n> }
---

# Summary

<2-4 sentence overview of what the agent looked at and what it found>

# Findings

## F<NNN> — <one-line title>

- **Severity:** P<N>
- **Confidence:** <high|medium|low>
- **Location:** `<file>:<line range>`
- **Issue:** <1-3 sentences describing what is wrong>
- **Evidence:** <verbatim code snippet or doc passage; cite spec/CIP/RFC if applicable>
- **Suggested fix:** <concrete patch description; code block if a one-liner>
- **Verification:** <how the lead can verify the finding without re-reading the whole file>

## F<NNN> — ...
```

Findings are numbered F001, F002, ... within each agent's file. The numbering does not need to be unique across agents; the lead synthesises a global registry in the summary.

## Lead synthesis: `audit/SUMMARY.md`

Codex (the lead) produces one consolidated summary at `audit/SUMMARY.md`. The summary contains:

- **Triage table.** Every finding from every agent listed once, ranked by severity then confidence, with a one-line title and a link to the per-agent file. The table is sortable by severity at a glance.
- **Must-fix-before-push.** Every P0 plus every P1 with high confidence, listed in suggested-fix-order.
- **Fix-in-follow-up.** Every P1 with low/medium confidence plus every P2.
- **Acknowledge-and-ship.** Every P3 plus any P2 that the lead judges genuinely out of scope (with one-line justification).
- **Cross-cutting themes.** Patterns the lead noticed across multiple agent reports (for example, "three agents independently flagged missing domain separation in the Merkle leaf hash").
- **Open questions for the repository owner.** Decisions only the human can make, listed at the end.

The summary is at most 300 lines. The per-agent reports can be longer.

## Tooling and reading conventions

The lead and every sub-agent operates with read-only access to the repository (no commits, no edits). Findings are *proposals*; the human owner ingests them and decides what to apply.

For grep-style searches use ripgrep (`rg`) where possible. For Rust files prefer reading the whole module rather than cherry-picking lines: a 200-line module is cheap context and avoids missing inter-function invariants. For wiki pages read the page's frontmatter first, then the body, then any pages it `[[slug]]`-references.

When citing line numbers, use the form `path/to/file.rs:LL-LL` (range) or `path/to/file.rs:LL` (single line). For prose docs, use a `>` blockquote with the offending sentence verbatim.

## Ingestion plan (for the human owner)

After Codex finishes:

1. Read `audit/SUMMARY.md` end to end (target ~10 minutes).
2. Decide on each P0 and P1 finding individually: fix in-place now, defer with a written note, or contest. Document the decision in a final block at the bottom of `SUMMARY.md` titled "Owner triage 2026-05-03".
3. Apply the agreed fixes as separate commits, one per finding (this preserves the audit trail and lets the next reviewer diff cleanly).
4. For deferred items, open a corresponding entry in `cardano-wiki/wiki/log.md` under a new `[2026-05-DD] audit-defer | <topic>` heading so the deferral is recorded in the project log.
5. Run `cargo test --workspace`, `cargo fmt --check`, `cargo clippy -- -D warnings` once before committing the fixes; once after.
6. Commit the entire `audit/` directory as a single commit with subject `audit: 10-agent pre-push review`. The audit findings stay in the repo as historical artefact.
7. Push.

## What the audit is NOT

The audit is not a feature-completeness review. v1.0 is in progress; missing v1.0 features are expected and tracked in the To-Do section of `README.md`. The audit looks at what is already in the repo, not at what is planned but absent.

The audit is not a re-derivation of the program design. The architectural decisions (PQ-only primitives, dual-hash bundle, lazy resurrection, etc.) are out of scope. The audit checks that the implementation matches the design, not that the design is the right design.

The audit is not a correctness proof. Findings are proposals based on the reviewer's reading of the code; ground-truth verification (does the leaf encoding match the formal spec when run against a real mainnet snapshot) requires the cross-implementation reproducibility check that lives downstream of T1.
