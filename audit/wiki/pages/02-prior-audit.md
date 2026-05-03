# Prior Audit Cross-Reference

_Source: parallel agent pass, 2026-05-03. Files audited: `audit/SUMMARY.md`, `audit/RESOLUTION.md`, `audit/findings/A1–A10`, `audit-prompt.md`._

## Audit prompt and scope

The 10-agent pre-push audit (2026-05-03) was driven by `audit-prompt.md` and `instructions.md`. Lanes A1–A6 covered code; A7–A8 docs/wiki; A9 plan; A10 cross-cutting ops. Each sub-agent reviewed against protocol invariants without seeing peer outputs; synthesis produced a triage table. **43 total findings: 0 P0, 21 P1, 16 P2, 6 P3.**

## Headline findings by track

### A1 Cryptographic Correctness — 5 findings (4 P1)
- F001 (A1:41-42): leaf hashes do not bind `(sub_tree_id, canonical_index)` to preimage.
- F002 (A1:56-68): leaf and internal-node hashes both raw Blake2b-256, no distinct domain tag.
- F003 (A1:82-90): zero-padding uses literal all-zero hash; verifiers cannot distinguish empty slots without external counts.
- F004 (A1:126-137): SHA3 bundle root consumes the same Blake2b leaves — no defense against a Blake2b break at leaf layer.
- F005 (A1:165-174): duplicate semantic keys documented as data errors; builders never invoke validators.

### A2 CBOR Strictness — 2 findings (1 P1)
- F001 (A2:56-88): no real mainnet UTxO CBOR decoder; `omega-utxo-snapshot` writes raw `GetUTxOWhole` bytes; `ingest_utxos` only accepts synthetic fixture shape.
- F002 (A2:99-131): multi-asset maps accept duplicate / non-canonical keys.

### A3 Cardano Semantics — 5 findings (all P1)
- F001 (A3:22-32): UTXO leaves collapse address to fixed 32-byte hash; cannot represent Byron/bootstrap, pointer, base, enterprise, reward variants.
- F002 (A3:42-52): leaves omit CIP-32 inline datums and CIP-33 reference scripts.
- F003 (A3:60-71): stake leaves can't encode Conway DRep sum type.
- F004 (A3:82-87): UTXO and ledger-state acquired at "latest tip" with no shared epoch/Mark/Set/Go selection.
- F005 (A3:97-106): only treasury committed; reserves, deposits, fee-pot omitted from AccountState.

### A4 Test Design — 4 findings (1 P1)
- F001 (A4:25-36): hash domain separation not test-locked.
- F002 (A4:48-68): per-leaf golden vectors absent.
- F003 (A4:77-103): edge-case fixture corpus too narrow.
- F004 (A4:113-124): three test paths compute into `_` and only check no panic.

### A5 Rust Idioms — 6 findings (0 P1)
- F001-F002: bundle and ingest crates leak `anyhow::Result` in public APIs.
- F003 (A5:55-65): deps pinned major-only; `serde = "1"`, `clap = "4"`, `tokio = "1"` float.
- F004 (A5:72-82): `cargo fmt --check` fails on ingest example (long lines).
- F005-F006: probe panics on missing arg via `expect`; rustdoc claims paths "scaffolded" that are real.

### A6 LSQ Binary — 3 findings (1 P1)
- F001 (A6:22-54): full UTxO copied via `.to_vec()` before write; LSQ release delayed past write.
- F002 (A6:62-91): runbook omits `AnyCbor` buffering warning and 16+ GB RAM caveat.
- F003 (A6:99-122): `pallas-network 0.30.2` documented but not pinned (manifest uses floating `0.30`, no Cargo.lock).

### A7 Top-Level Docs — 6 findings (2 P1)
- F001 (A7:22-28): README claims `(sub_tree_id, leaf_index, payload_hash)` domain separation that code did not implement.
- F002 (A7:37-45): README/ARCHITECTURE described SHA3 bundle structure differently from bundle crate.
- F003-F004: missing chain-follower input arrows; pallas-vs-Koios decision matrix task lingers.
- F005-F006: stale test-count (26 vs actual 248); signature-size ranges cited without repo evidence.

### A8 Wiki Coherence — 3 findings (0 P1)
- F001-F003: ingestion pages missing required frontmatter; SCHEMA closed-list operations diverge from log usage; `/home/hoskinson/...` hardcoded in spec page.

### A9 Plan Completeness — 5 findings (4 P1)
- v1.0 plan still contains `cardano-cli ... --output-cbor` after the 2026-05-03 pivot deprecating it.
- Task 14 missed `omega-utxo-snapshot` in the version-bump crate list.
- Headless runbook claims `omega-ingest utxo --format mainnet` (no such flag).
- Deprecated Daedalus runbook keeps the obsolete CBOR dump.
- Older Codex briefing says four-crate workspace; manifest now five.

### A10 Operational — 4 findings (3 P1)
- F001: cargo deps unpinned at major level.
- F002: CI workflow nested under `omega-commitment/.github/workflows/` — GitHub Actions does not run it from there; no root-level workflow.
- F003-F004: snapshot helper bypasses `mithril-client` verification; runbook installs `cardano-node` and `mithril-client` without checksums or sig verification.

## Resolved

42 of 43 findings closed across five batches (RESOLUTION.md):

| Batch | Theme | Commit |
|---|---|---|
| 1 | Crypto soundness + dual-hash story | `bd6ac46` |
| 2 | Cardano semantic fidelity | `71bb5cc` |
| 3 | v1.0 pivot propagation | `5f777d1` |
| 4 | Release readiness + ops trust | `d09db8e` |
| 5 | Long tail + audit-trail | HEAD |

Each batch passed `cargo clean && cargo fmt --check && cargo clippy --workspace --all-targets && cargo test --workspace`. Test count: 248 → **282** (post-Batch-3, held through Batch-5 + per-leaf goldens).

## Outstanding

One deferred P1: **A2/F001** — real mainnet `GetUTxOWhole` decoder. Tracked as v1.0 Task 4 in `2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`. TODO marker in `omega-utxo-snapshot/src/main.rs` keeps it visible at the call site (RESOLUTION.md:20). Blocks full mainnet e2e but not v0.9.1 synthetic-path release.

## Tracks worth re-examining

1. **A1** — Verify all leaf/node hashes route through tagged helpers (`"omega:v1:leaf"`, `"omega:v1:node"`); witness verification uses same paths; new goldens actually pin tags in preimages.
2. **A3** — Confirm UTXO leaves carry variable-length address bytes (not 32-byte hash); DRep enum covers `None | KeyHash | ScriptHash | AlwaysAbstain | AlwaysNoConfidence`; fixtures cover all address variants; manifest includes Mark/Set/Go and epoch.
3. **A4** — `golden_per_leaf.rs` pins canonical encodings for all seven leaf types; empty-set / single-leaf / AlwaysAbstain cases pinned; full edge-case corpus tracked.
4. **A7+A9** — `setup_daedalus.md` and v1.0 Task 2 either historical or rewritten; runbook references current CLI surface; Task 14 includes `omega-utxo-snapshot`.
5. **A10** — All manifests use `~major.minor`; root `.github/workflows/ci.yml` exists and runs on push; `download_snapshot.sh` and runbook use `mithril-client` verification or mark debug-only; Cargo.lock policy documented.
