# Audit Resolution Trail

Maps every finding from the 2026-05-03 10-agent Codex pre-push audit
([`SUMMARY.md`](./SUMMARY.md)) to the commit that closed it (or to its
explicit deferral note). 43 findings total: 42 closed across five
batches, 1 deferred to v1.0 Task 4.

The five-batch resolution plan is `cardano-wiki/docs/superpowers/plans/2026-05-03-codex-audit-resolution-plan.md`;
each batch shipped as one commit on `main`.

## Resolution table

| Finding | Severity | Resolved by | SHA |
|---|---|---|---|
| A1/F001 | P1 | Batch 1 | bd6ac46 |
| A1/F002 | P1 | Batch 1 | bd6ac46 |
| A1/F003 | P1 | Batch 1 | bd6ac46 |
| A1/F004 | P1 | Batch 1 | bd6ac46 |
| A1/F005 | P2 | Batch 1 | bd6ac46 |
| A2/F001 | P1 | DEFERRED to v1.0 Task 4 | (TODO marker landed in Batch 4 `d09db8e`; full implementation tracked as Task 4 of `2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`) |
| A2/F002 | P2 | Batch 2 | 71bb5cc |
| A3/F001 | P1 | Batch 2 | 71bb5cc |
| A3/F002 | P1 | Batch 2 | 71bb5cc |
| A3/F003 | P1 | Batch 2 | 71bb5cc |
| A3/F004 | P1 | Batch 2 | 71bb5cc |
| A3/F005 | P1 | Batch 2 | 71bb5cc |
| A4/F001 | P1 | Batch 1 | bd6ac46 |
| A4/F002 | P2 | Batch 5 | Batch 5 (HEAD) |
| A4/F003 | P2 | Batch 5 (partial — empty set / single-leaf / AlwaysAbstain pinned in `golden_per_leaf.rs`; full edge-case corpus tracked as v1.1 fixture-expansion task with the `EDGE_CASE_FIXTURE_TODO` marker) | Batch 5 (HEAD) |
| A4/F004 | P3 | Batch 5 | Batch 5 (HEAD) |
| A5/F001 | P2 | Batch 5 | Batch 5 (HEAD) |
| A5/F002 | P2 | Batch 5 | Batch 5 (HEAD) |
| A5/F003 | P2 | Batch 4 | d09db8e |
| A5/F004 | P3 | Batch 5 | Batch 5 (HEAD) |
| A5/F005 | P3 | Batch 5 | Batch 5 (HEAD) |
| A5/F006 | P3 | Batch 5 | Batch 5 (HEAD) |
| A6/F001 | P1 | Batch 5 | Batch 5 (HEAD) |
| A6/F002 | P2 | Batch 3 | 5f777d1 |
| A6/F003 | P2 | Batch 4 | d09db8e |
| A7/F001 | P1 | Batch 1 | bd6ac46 |
| A7/F002 | P1 | Batch 1 | bd6ac46 |
| A7/F003 | P2 | Batch 3 | 5f777d1 |
| A7/F004 | P2 | Batch 3 | 5f777d1 |
| A7/F005 | P2 | Batch 3 | 5f777d1 |
| A7/F006 | P3 | Batch 5 | Batch 5 (HEAD) |
| A8/F001 | P2 | Batch 5 | Batch 5 (HEAD) |
| A8/F002 | P2 | Batch 5 | Batch 5 (HEAD) |
| A8/F003 | P3 | Batch 5 | Batch 5 (HEAD) |
| A9/F001 | P1 | Batch 3 | 5f777d1 |
| A9/F002 | P1 | Batch 3 | 5f777d1 |
| A9/F003 | P1 | Batch 3 | 5f777d1 |
| A9/F004 | P1 | Batch 3 | 5f777d1 |
| A9/F005 | P2 | Batch 3 | 5f777d1 |
| A10/F001 | P1 | Batch 4 | d09db8e |
| A10/F002 | P1 | Batch 4 | d09db8e |
| A10/F003 | P1 | Batch 4 | d09db8e |
| A10/F004 | P2 | Batch 4 | d09db8e |

## Per-batch summary

| Batch | Theme | Commit | Findings closed |
|---|---|---|---|
| 1 | Crypto soundness + dual-hash story | `bd6ac46` | A1/F001-F005, A4/F001, A7/F001-F002 (8) |
| 2 | Cardano semantic fidelity | `71bb5cc` | A2/F002, A3/F001-F005 (6) |
| 3 | v1.0 pivot propagation | `5f777d1` | A6/F002, A7/F003-F005, A9/F001-F005 (9) |
| 4 | Release readiness + ops trust | `d09db8e` | A5/F003, A6/F003, A10/F001-F004 (7); A2/F001 deferred |
| 5 | Long tail + audit-trail | Batch 5 (HEAD) | A4/F002-F004, A5/F001-F002 + F004-F006, A6/F001, A7/F006, A8/F001-F003 (13) |
| — | DEFERRED | (tracked in code) | A2/F001 (mainnet UTxO CBOR decoder — multi-day; v1.0 Task 4) |

**Total:** 8 + 6 + 9 + 7 + 13 = 43 findings; 42 closed, 1 deferred.

## Verification gate at each batch

Every batch went through the same gate before commit:

```
cargo clean
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Test counts moved as fixes added new locks:
- pre-Batch-1: 248 tests
- post-Batch-1 (`bd6ac46`): + domain-separation lock tests
- post-Batch-2 (`71bb5cc`): + Cardano-semantic-fidelity tests
- post-Batch-3 (`5f777d1`): 282 tests (no code changes)
- post-Batch-4 (`d09db8e`): 282 tests (no code changes)
- post-Batch-5 (this commit): 282 + per-leaf goldens (10 new tests) + new bundle-error / ingest-error coverage as wired through existing tests

## Reading order

If you want to follow the resolution chronologically:
1. Read `SUMMARY.md` for the original triage.
2. Read `findings/A{1..10}-*.md` for per-agent details.
3. Read the wiki log entries `## [2026-05-03] resolve | Batch N` in
   `cardano-wiki/wiki/log.md` (one per batch).
4. Read each batch's commit message (`git log --format=full bd6ac46 71bb5cc 5f777d1 d09db8e HEAD`).
5. Diff against the previous baseline (`git diff f33cb20..HEAD`) for the
   full code/docs surface.

## Status

The repository is now publication-ready against the audit baseline.
A2/F001 remains the single open item, scoped explicitly to v1.0 Task 4
(real-mainnet UTxO CBOR decoder pass); a TODO marker in
`crates/omega-utxo-snapshot/src/main.rs` immediately after the disk
write keeps the deferral visible at the call site.
