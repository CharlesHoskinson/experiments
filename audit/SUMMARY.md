# 10-Agent Pre-Push Audit Summary

All ten sub-agents reported. Total findings: P0=0, P1=21, P2=16, P3=6.

## Triage table

| Severity | Confidence | Agent | Finding | Title | Report |
|---|---|---:|---|---|---|
| P1 | high | A1 | F001 | Leaf hashes do not bind sub_tree_id or canonical leaf_index | [A1](findings/A1-cryptographic-correctness.md) |
| P1 | high | A1 | F002 | Leaf and internal-node hashes share the same untagged domain | [A1](findings/A1-cryptographic-correctness.md) |
| P1 | high | A1 | F003 | Zero padding can be proven as membership without item_count checks | [A1](findings/A1-cryptographic-correctness.md) |
| P1 | high | A1 | F004 | Bundle SHA3 track is cosmetic against a Blake2b leaf-hash break | [A1](findings/A1-cryptographic-correctness.md) |
| P1 | high | A2 | F001 | Mainnet UTxO CBOR has no decoder behind the snapshot producer | [A2](findings/A2-cbor-strictness.md) |
| P1 | high | A3 | F001 | UTXO leaves collapse Cardano addresses to a fixed 32-byte hash | [A3](findings/A3-cardano-semantics.md) |
| P1 | high | A3 | F002 | UTXO leaves do not commit inline datums or reference scripts | [A3](findings/A3-cardano-semantics.md) |
| P1 | high | A3 | F003 | Stake leaves cannot encode the Conway DRep sum type | [A3](findings/A3-cardano-semantics.md) |
| P1 | high | A3 | F004 | Snapshot sources are not pinned to one block/epoch or Mark/Set/Go choice | [A3](findings/A3-cardano-semantics.md) |
| P1 | high | A3 | F005 | AccountState pots are not fully committed | [A3](findings/A3-cardano-semantics.md) |
| P1 | high | A6 | F001 | Full UTxO response is copied before write and LSQ release is delayed | [A6](findings/A6-lsq-binary.md) |
| P1 | high | A7 | F001 | README claims Merkle domain separation that is absent from code | [A7](findings/A7-top-level-docs.md) |
| P1 | high | A7 | F002 | Top-level docs define the SHA3 bundle root differently from the bundle crate | [A7](findings/A7-top-level-docs.md) |
| P1 | high | A9 | F001 | v1.0 plan still contains executable --output-cbor Task 2 body | [A9](findings/A9-plan-completeness.md) |
| P1 | high | A9 | F002 | Task 14 omits omega-utxo-snapshot from the v1.0 version bump | [A9](findings/A9-plan-completeness.md) |
| P1 | high | A9 | F003 | Headless runbook claims a not-yet-existing omega-ingest --format mainnet path | [A9](findings/A9-plan-completeness.md) |
| P1 | high | A9 | F004 | Deprecated Daedalus runbook still instructs the obsolete CBOR dump | [A9](findings/A9-plan-completeness.md) |
| P1 | high | A10 | F001 | Cargo dependencies are not all pinned to major.minor | [A10](findings/A10-operational.md) |
| P1 | high | A10 | F002 | CI workflow is nested where GitHub Actions will not run it for this repo | [A10](findings/A10-operational.md) |
| P1 | high | A10 | F003 | Snapshot helper downloads/extracts Mithril snapshot without verification | [A10](findings/A10-operational.md) |
| P1 | medium | A4 | F001 | Hash-domain separation is not test-locked | [A4](findings/A4-test-design.md) |
| P2 | high | A4 | F002 | Golden vectors skip the per-leaf layer | [A4](findings/A4-test-design.md) |
| P2 | high | A4 | F003 | Ledger edge-case fixture corpus is too narrow | [A4](findings/A4-test-design.md) |
| P2 | high | A5 | F001 | Bundle library exposes anyhow in public APIs | [A5](findings/A5-rust-idioms.md) |
| P2 | high | A5 | F002 | Ingest library exports anyhow instead of typed parse errors | [A5](findings/A5-rust-idioms.md) |
| P2 | high | A5 | F003 | Several dependencies are major-only under a no-lockfile policy | [A5](findings/A5-rust-idioms.md) |
| P2 | high | A6 | F002 | Headless setup runbook omits AnyCbor memory and streaming warning | [A6](findings/A6-lsq-binary.md) |
| P2 | high | A6 | F003 | pallas-network 0.30.2 is documented but not reproducibly pinned | [A6](findings/A6-lsq-binary.md) |
| P2 | high | A7 | F003 | README diagram omits the chain-follower input for header and tx-index | [A7](findings/A7-top-level-docs.md) |
| P2 | high | A7 | F004 | README Task 13 does not match the v1.0 plan text | [A7](findings/A7-top-level-docs.md) |
| P2 | high | A7 | F005 | omega-commitment README has stale commands and test-count prose | [A7](findings/A7-top-level-docs.md) |
| P2 | high | A8 | F001 | Ingestion pages do not satisfy frontmatter/provenance schema | [A8](findings/A8-wiki-coherence.md) |
| P2 | high | A8 | F002 | Schema operation and index-category vocabularies are stale | [A8](findings/A8-wiki-coherence.md) |
| P2 | high | A9 | F005 | Codex briefings disagree on workspace shape after pipeline update | [A9](findings/A9-plan-completeness.md) |
| P2 | high | A10 | F004 | Headless runbook installs downloaded executables without checksum/signature verification | [A10](findings/A10-operational.md) |
| P2 | medium | A1 | F005 | Duplicate semantic keys are documented as errors but accepted by root builders | [A1](findings/A1-cryptographic-correctness.md) |
| P2 | medium | A2 | F002 | Multi-asset maps accept duplicate/non-canonical keys | [A2](findings/A2-cbor-strictness.md) |
| P3 | high | A4 | F004 | Some unit tests discard the computed value | [A4](findings/A4-test-design.md) |
| P3 | high | A5 | F004 | rustfmt check fails on the ingest example | [A5](findings/A5-rust-idioms.md) |
| P3 | high | A5 | F005 | Probe example panics on missing user input | [A5](findings/A5-rust-idioms.md) |
| P3 | high | A5 | F006 | Ingest source docs still claim implemented paths are scaffolded | [A5](findings/A5-rust-idioms.md) |
| P3 | high | A8 | F003 | Omega spec page hardcodes the non-repo wiki path | [A8](findings/A8-wiki-coherence.md) |
| P3 | medium | A7 | F006 | Signature-size ranges are not pinned in repo-local evidence | [A7](findings/A7-top-level-docs.md) |

## Must-fix-before-push

A1/F001: Redesign leaf hashing so every leaf preimage includes sub_tree_id and canonical index, then re-pin affected vectors. A1 suggests a versioned preimage such as `omega:v1:leaf || sub_tree_id || canonical_index || payload_len || payload`.

A1/F002: Add explicit leaf and internal-node domain-separated hash helpers and route witness verification through them. A1 suggests separate `omega:v1:leaf` and `omega:v1:node` tags.

A1/F003: Remove raw zero-hash padding as a valid membership target. A1 suggests domain-separated empty leaves and verifier-visible `item_count` checks.

A1/F004: Decide whether the SHA3 track is meant to be independent. If yes, build SHA3 leaves from canonical payloads rather than from Blake2b leaves; if no, publish it as aggregation drift detection rather than a Blake2b-break hedge.

A2/F001: Implement the real mainnet `GetUTxOWhole` decoder before claiming the LSQ producer is ingestible. A2 calls for a mainnet parser that decodes `(TransactionInput, TransactionOutput)`, preserves address/value/asset bytes, handles pointer TxIx as `u64`, and rejects trailing bytes.

A3/F001: Replace `address_hash: [u8; 32]` with canonical raw Cardano address bytes and fixtures for Byron/bootstrap, pointer, base, enterprise, and reward/stake variants.

A3/F002: Extend UTXO leaves to commit `datum_option` and `script_ref`. A3 suggests tagged, length-delimited encodings for none/hash/inline datum and reference-script data.

A3/F003: Replace raw `delegated_drep: [u8; 28]` with a tagged Conway DRep enum covering key hash, script hash, AlwaysAbstain, AlwaysNoConfidence, and none.

A3/F004: Introduce a snapshot manifest with target block hash, slot, epoch, stability depth, and selected Mark/Set/Go snapshot. Use fixed-point acquisition rather than `acquire(None)` for the UTXO stream.

A3/F005: Add committed governance/accounting facts for reserves, deposits, fee pot, and any other AccountState fields needed by reward semantics; fail closed when expected pots are absent.

A6/F001: Avoid the extra full-buffer copy in `omega-utxo-snapshot` and release/done the LSQ session before local disk write if minimizing acquisition lifetime is the goal. A6 suggests moving bytes out of `AnyCbor` instead of `raw.to_vec()`.

A7/F001: Either implement the README's documented Merkle domain separation or revise the README to describe the current raw Blake2b construction. This overlaps A1/F001 and A1/F002.

A7/F002: Make top-level README/ARCHITECTURE agree with the bundle crate on the SHA3 root construction. A7 found ARCHITECTURE says SHA3 hashes the seven Blake2b roots, while code aggregates seven per-sub-tree SHA3 roots.

A9/F001: Remove or rewrite the executable `--output-cbor` Task 2 body in the v1.0 plan. It directly contradicts the 2026-05-03 revision.

A9/F002: Update Task 14 to bump all five crate manifests, including `crates/omega-utxo-snapshot/Cargo.toml`, from the actual v0.9.1 baseline.

A9/F003: Fix the canonical headless runbook's claim that `omega-ingest utxo --format mainnet` exists, or implement the CLI/parser path before publication.

A9/F004: Turn `setup_daedalus.md` into clearly historical reference only; remove active `--output-cbor` instructions or mark them obsolete inline.

A10/F001: Pin all external Cargo dependencies at least to major.minor, or exact versions where reproducibility matters. This overlaps A5/F003 and A6/F003.

A10/F002: Add root-level `.github/workflows/ci.yml` for the experiments repository, with `working-directory: omega-commitment`; the nested workflow will not run for root pushes.

A10/F003: Replace direct `curl`/`tar` Mithril snapshot extraction with `mithril-client cardano-db download` plus verification keys, or mark the helper unauthenticated debug-only.

## Fix-in-follow-up

- A4/F001: Add tests that lock domain separation behavior; this becomes mandatory if the A1 fixes are implemented.
- A4/F002: Add per-leaf golden vectors for canonical encoded bytes and leaf hashes.
- A4/F003: Expand edge fixtures for non-UTF8 assets, Byron/pointer addresses, inline datums, reference scripts, AlwaysAbstain, malformed CBOR, and large/deep trees.
- A5/F001: Replace public `anyhow::Result` in the bundle library with a typed `BundleError`.
- A5/F002: Replace public `anyhow::Result` in the ingest library with typed ingest/CBOR errors.
- A5/F003: Resolve the no-lockfile plus major-only dependency policy conflict; likely fixed with A10/F001.
- A6/F002: Add the AnyCbor full-buffer and streaming fallback warning to the canonical headless runbook.
- A6/F003: Pin pallas-network/pallas-codec to the audited `=0.30.2`, or check in a lockfile for binary releases.
- A7/F003: Add the v1.1 chain-follower lane to the README architecture diagram or label header/tx-index as placeholders.
- A7/F004: Reconcile README Task 13 with the v1.0 plan's current pallas-vs-Koios task.
- A7/F005: Update omega-commitment README command examples and stale test-count/scaffold prose.
- A8/F001: Bring the two May 3 ingestion wiki pages into the declared frontmatter/provenance schema.
- A8/F002: Update SCHEMA.md to include current log operations and the Mainnet Ingestion index category, or mark the lists non-exhaustive.
- A9/F005: Update the older Codex briefing's workspace-count text or explicitly supersede that section.
- A10/F004: Add checksum/signature verification to the headless binary install steps.
- A1/F005: Reject or canonicalize duplicate semantic keys before root construction.
- A2/F002: Reject duplicate/non-canonical multi-asset CBOR map keys and preserve raw non-UTF8 names.

## Acknowledge-and-ship

- A4/F004: Unit tests that discard computed values can ship if the P1/P2 golden-vector work is tracked.
- A5/F004: rustfmt failure in `probe_ledger_state_paths.rs` is low-risk but should be cleaned before normal CI.
- A5/F005: Probe example `expect` on missing CLI input is a developer-experience issue.
- A5/F006: Ingest rustdoc saying implemented paths are scaffolded is stale prose.
- A8/F003: Hardcoded `/home/hoskinson/cardano-wiki` paths are local-specific but not a protocol blocker.
- A7/F006: PQ signature-size ranges should be locally sourced, but this can ship if exact numbers are not used as protocol parameters.

## Cross-cutting themes

Crypto-domain mismatch is convergent: A1, A4, and A7 independently flagged that the docs/tests claim or need domain separation while code hashes leaves and internal nodes in raw Blake2b domains.

Cardano semantic fidelity is under-modeled: A2, A3, and A4 all found that the current fixture-era data model cannot yet represent real mainnet UTxO/address/datum/script/DRep/accounting cases.

The v1.0 two-stream pivot is not consistently propagated: A7, A9, A5, and A6 found stale Daedalus, CBOR, command, scaffold, and runbook text after the 2026-05-03 architecture update.

Release reproducibility is weak: A5, A6, and A10 all flagged floating dependencies/no lockfile/pallas pinning/CI placement as publication-readiness gaps.

Operational trust boundaries need tightening: A3, A6, and A10 flagged fixed snapshot points, LSQ acquisition lifetime/memory, Mithril verification, and release-binary verification.

## Open questions for the repository owner

1. Pick one: implement domain-separated leaf/node hashes and re-pin all goldens before publication, or publish v0.9.1 as a non-final draft with docs explicitly saying the Merkle hash domain is not final?
2. Pick one: make SHA3 a truly independent parallel tree now, or reword all docs to say the current SHA3 track is only aggregation-level drift detection?
3. Yes/no: should v1.0 publication wait until UTXO leaves commit raw addresses, inline datums, reference scripts, tagged DReps, and all AccountState pots?
4. Pick one: require a single fixed snapshot manifest for both JSON and LSQ streams now, or allow latest-tip acquisition during experimental smoke tests only?
5. Pick one: check in Cargo.lock for this binary-shipping workspace, or keep no-lockfile and pin every external dependency tightly?
6. Yes/no: should the deprecated Daedalus runbook be removed from the publication artifact rather than retained as historical reference?
7. Pick one: ship the audit directory with all findings unresolved as a public transparency artifact, or fix P1s first and then publish the audit alongside the fix commits?
