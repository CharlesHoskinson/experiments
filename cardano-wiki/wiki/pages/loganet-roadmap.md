---
title: LoganNet — milestone roadmap and Group 1 deferrals
slug: loganet-roadmap
tags: [loganet, omega-toy-consensus, omega-tui, omega-api, omega-goblins, roadmap, milestone]
sources:
  - omega-toy-consensus-group1-design
  - loganet-cli-experience-design
  - loganet-goblins-readme-expansion
  - add-goblin-agentic-framework
  - add-proof-experiment-harness
provenance:
  - omega-toy-consensus-group1-design -> Group 1 scope, error-code map, failure-injection table, what is deferred to Group 2
  - loganet-cli-experience-design -> omega-tui crate scope (banner, dashboard, palette, drain machine)
  - loganet-goblins-readme-expansion -> LoganNet topology framing (3 nodes, libp2p:4001-4003, omega-api:8001-8003, rusqlite per node) and the six-goblin role table
  - add-goblin-agentic-framework -> omega-api REST + JSON-RPC layer, omega-goblin-core / omega-goblin-runner crates, six default Goblin roles
  - add-proof-experiment-harness -> the upstream crates Group 1 wires together (omega-claim-tx, omega-claim-verifier, omega-mock-ledger, omega-network)
confidence: high
created: 2026-05-05
updated: 2026-05-05
aliases: [LoganNet roadmap, omega-toy-consensus roadmap, LoganNet groups]
cssclass: wiki-page
---

# LoganNet — milestone roadmap and Group 1 deferrals

LoganNet is the local 3-node Raft cluster that turns the proof-experiment harness into a thing that actually runs. Three nodes on one developer box, libp2p between them on `:4001-4003`, JSON-RPC on `:8001-8003`, rusqlite per node, the `omega-mock-ledger` writer actor as the state machine, openraft for consensus. LGN is the unit of value flowing on Starstream UTxOs after a claim applies. **Non-monetary** — there is no exchange, no listing, no liquidity story.

This page tracks what ships per group / per crate, and — crucially — what is **deferred** so future readers and Codex briefings know where each missing piece lands.

## Component status

| Crate | Role | Status (2026-05-05) |
|---|---|---|
| `omega-claim-tx` | Wire-format claim types + CBOR codec | shipped (proof-experiment harness batch 1) |
| `omega-claim-prover` | Plonky3 prover for membership AIR | shipped (proof-experiment harness batch 2) |
| `omega-claim-verifier` | Plonky3 verifier (used inside writer actor) | shipped (proof-experiment harness batch 2/3) |
| `omega-mock-ledger` | SQLite + openraft state machine, writer actor | shipped (PR #4 merged) |
| `omega-network` | libp2p request-response + CBOR Raft RPC + snapshot chunking | shipped (commit `a614f50`, branch `feat/omega-network-group5`) |
| `omega-toy-consensus` | Conductor: openraft + mock-ledger + network + JSON-RPC | **Group 1 in flight** (spec 2026-05-05) |
| `omega-tui` | Banner / dashboard / palette / drain machine | designed, not built (spec 2026-05-03 v3.1) |
| `omega-api` | REST + JSON-RPC public API + OpenAPI gen | proposed (`add-goblin-agentic-framework`), not built |
| `omega-goblin-core` | Goblin runtime types + LLM trait | proposed, not built |
| `omega-goblin-runner` | Spawns N goblins per role mix | proposed, not built |

## omega-toy-consensus — Group 1 (current work)

Spec: `docs/superpowers/specs/2026-05-05-omega-toy-consensus-design.md`. L3 scope = library + `omega-toy-consensus run` binary + minimal `jsonrpsee 0.26` JSON-RPC surface.

**In Group 1:**

- `omega_submitClaim(claim)` → `SubmitOutcome { accepted, applied_index, reject_reason }`
- `omega_getState()` → `NodeState { node_id, role, leader_id, last_log_id, applied_index, nullifier_count, starstream_utxo_count }`
- Client-side leader forwarding via `−32000 NotLeader` with `data: { leader_id, leader_rpc_url }`
- Error codes `−32000..−32005` (NotLeader / Verify / InvalidClaim / Replay / WriterClosed / Timeout)
- 3-node bring-up + single-claim round-trip
- Failure-injection scope **7c (maximal)**: turmoil partition + leader-change-mid-submit + snapshot-install-mid-submit, failpoints byzantine replay + writer close, shuttle-loom on rpc/writer handshake, kani bounded check on snapshot install, proptest on JSON-RPC inputs
- 13 integration tests + 1 Kani harness; orchestrator G2 gate (`STATUS=GREEN`) before "ready to commit"

## omega-toy-consensus — Group 2 deferrals

Items the Group 1 spec explicitly defers to Group 2:

| Item | Reason for deferral | Will land in |
|---|---|---|
| Membership change (add / remove node) | Out of scope for L3; openraft supports it but exercising it needs its own test fixtures | `omega-toy-consensus` Group 2 |
| `omega_clusterStatus` diagnostic RPC | Not needed for single-claim round-trip; useful once multi-leader / handover stories matter | `omega-toy-consensus` Group 2 |
| Persistent peer discovery (mdns / kademlia bootstrap) | Group 1 uses static `--peer` flags, sufficient for 3 known nodes on localhost | `omega-toy-consensus` Group 2 |
| `omega-toy-consensus::raft_node` test fixture (T2-F3) | rust-test-pack v0.1.0 release notes deferred to LoganNet milestone; lands inside Group 1 PR's test pack rows | `omega-toy-consensus` Group 1 (in flight) |
| `omega-mock-ledger::WriterActor` test fixture (T2-F2) | rust-test-pack v0.1.0 deferral; will surface inside Group 1's failpoints / shuttle-loom tests | `omega-toy-consensus` Group 1 (in flight) |
| Linux / macOS CI matrix | Group 1 stays on Windows + 1.95.0 to mirror PR-5 verification; cross-platform CI is a milestone gate | `omega-toy-consensus` Group 2 |
| T4 performance comparison run | rust-test-pack v0.1.0 baseline captured; comparison run is rust-test-pack v0.2.0 work | `rust-test-pack` v0.2.0 |

## Cross-component deferrals (separate crates)

Deferred *out of `omega-toy-consensus` entirely* because they belong to other crates:

| Item | Group 1 says | Where it actually lives |
|---|---|---|
| WebSocket subscriptions (`omega_subscribeApplied`, observer streams) | not in Group 1 | `omega-api` (or `omega-goblin-core` observer trait) — needed by Goblins observe-plan-act loop |
| TLS / auth on the JSON-RPC surface | not in Group 1 (localhost-only, documented) | `omega-api` |
| REST surface + OpenAPI generation | not in Group 1 | `omega-api` (openspec change `add-goblin-agentic-framework`) |
| Versioned `/v1/` API path with stability guarantees | not in Group 1 | `omega-api` |
| Banner / six-goblin strip / dashboard / sparklines / alarm row | not in Group 1 | `omega-tui` (spec `2026-05-03-loganet-cli-experience-design`) |
| `omega-experiment` / `omega-toy-consensus run` `--style pretty` polish | not in Group 1 (Group 1 uses `tracing-subscriber` plain logs) | `omega-tui` |
| Two-phase drain on `q` keystroke | not in Group 1 (Group 1 has `NodeHandle::shutdown` only) | `omega-tui` |
| `OMEGA_CLI_TIER` / `OMEGA_CLI_THEME` env-var detection | not in Group 1 | `omega-tui` |
| Six Goblin roles (Holder, Whale, Adversary, Lurker, SnapshotServer, Validator) | not in Group 1 | `omega-goblin-runner` |
| LLM-driven planning loop (Gemma-4 E4B via Ollama, mock-LLM mode) | not in Group 1 | `omega-goblin-core` + `omega-goblin-runner` |
| `cargo-fuzz` / `stateright` / `madsim` test-pack skills | not in Group 1 (orchestrator decision: not warranted yet) | future Group 2 / 3 of one of the above crates |

## Build order (dependency graph)

```
                 omega-claim-tx
                       │
            ┌──────────┼──────────┐
            ▼          ▼          ▼
    omega-claim-     omega-claim-  …
    prover           verifier
                                  │
                                  ▼
                          omega-mock-ledger ◀── omega-network
                                          ╲          ╱
                                           ╲        ╱
                                            ▼      ▼
                                    omega-toy-consensus  (Group 1, current)
                                            │
                            ┌───────────────┼───────────────┐
                            ▼               ▼               ▼
                       omega-tui      omega-api      omega-toy-consensus G2
                                            │
                                            ▼
                                   omega-goblin-core
                                            │
                                            ▼
                                   omega-goblin-runner
```

`omega-toy-consensus` is the keystone — every downstream crate depends on it being runnable end-to-end.

## See also

- [[spec-ouroboros-omega]] — the parent program
- [[omega-testnet-e2e-plan]] — the Cardano-preview end-to-end demo (separate axis from LoganNet local consensus)
- [[ouroboros-consensus]] — what we're forking from at the program level
