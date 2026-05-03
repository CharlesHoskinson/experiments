## Why

The proof-experiment harness (`add-proof-experiment-harness`) makes one human submit one proof at a time. That is enough to demonstrate end-to-end soundness, but not enough to *exercise* the network: we have no realistic mix of well-behaved holders, adversarial replays, batched-collection whales, snapshot-service consumers, validator outages, or lurker observers. A behavioral simulation needs autonomous agents that play these roles continuously and at parameterizable scale.

Two concurrent needs land in this change. First, a rich machine-readable interface — REST + JSON-RPC — so any client (CLI, simulator, browser, future wallet) can drive the harness through one stable surface. Second, an agentic framework using **Gemma-4 E4B** (Google DeepMind, Apache-2.0, edge-tier 4B-effective-parameter model, runs locally via Ollama or llama.cpp) where each agent — a "Goblin" — takes a role on the network and plans its actions via local LLM inference. N goblins are spawned with assigned role distributions; the operator parameterises the mix.

Goblins pressure-test the harness, surface emergent bugs, and produce realistic load mixes for benchmarking. They are not part of the protocol — they are tooling.

## What Changes

- Add 3 new crates to the workspace:
  - `omega-api` — the public interface layer. Axum-based REST + JSON-RPC server fronting the existing `omega-toy-consensus` JSON-RPC. Stable versioned API at `/v1/` with OpenAPI spec auto-generated. Hosts on each Raft node so any client can hit any node and have it forwarded to the leader.
  - `omega-goblin-core` — Goblin runtime types: `Role`, `GoblinId`, `ObservationLoop`, `Plan`, `Action`, `LlmClient` trait. Pure Rust, no LLM-vendor lock-in.
  - `omega-goblin-runner` — binary that spawns N goblins per role mix, drives each goblin's observe-plan-act loop, talks to one or more `omega-api` endpoints and one local LLM endpoint (Ollama by default).
- Add 6 default goblin roles:
  - `Holder` — constructs a single-leaf `claim_utxo` against a synthetic genesis fixture, submits, observes apply.
  - `Whale` — same as Holder but constructs a `ClaimCollection` of K (10..1024) leaves at once.
  - `Adversary` — replays an already-submitted claim, tampers a proof byte, submits a malformed CBOR; expects rejection.
  - `Lurker` — passive observer: polls `/v1/state` every M seconds, emits a structured trace.
  - `SnapshotServer` — answers Merkle-path requests over a separate libp2p protocol (a stand-in for the real T5 mirror partnerchain).
  - `Validator` — represents one of the 3 Raft nodes' ops behaviour: random restarts, simulated network partitions, snapshot-trigger dares. Runs *outside* the cluster and acts via the API + a sidecar control channel; does NOT impersonate a Raft node.
- LLM integration via Ollama by default (`gemma4:e4b` model); abstract via an `LlmClient` trait so a developer can swap to llama.cpp HTTP, vLLM, or a mock LLM for CI.
- Goblin role mix is parameterised: `omega-goblins run --holders 20 --whales 2 --adversaries 5 --lurkers 3 --snapshot-servers 1 --validators 0 --duration 30m --llm http://127.0.0.1:11434`.
- Mock-LLM mode (`--llm mock`) replaces real Gemma calls with deterministic scripted responses so the framework can be exercised in CI without a GPU.

## Capabilities

### New Capabilities

- `omega-api`: stable REST + JSON-RPC interface for the proof-experiment harness. Versioned (`/v1/`), OpenAPI-documented, hosted on each consensus node.
- `goblin-framework`: multi-agent behavioral simulation framework. Parameterizable N; per-role action loops driven by Gemma-4 E4B (or any `LlmClient` impl); observable via structured logs and a Prometheus metrics endpoint.

### Modified Capabilities

- `proof-harness` (depends on `add-proof-experiment-harness`): the JSON-RPC submit endpoint shipped in v0.1 of the harness becomes an *internal* surface; clients use `omega-api` instead. The modified-spec entry restricts the existing scenarios to the API-fronted path and adds API-stability scenarios.

## Impact

- 3 new workspace crates; 1 new binary (`omega-goblins`).
- New runtime deps: `axum` 0.7+, `tower` / `tower-http`, `utoipa` (for OpenAPI auto-gen), `reqwest` (for Ollama HTTP), `prometheus` (metrics), `tracing` + `tracing-subscriber` (already present).
- LLM integration ships an `LlmClient` trait with three impls: `OllamaClient` (default, talks to a local Ollama HTTP at `:11434`), `LlamaCppClient` (talks to llama.cpp's `--api` mode), `MockLlmClient` (deterministic, no LLM, for CI).
- Documentation: a "Run a goblin simulation" subsection inside the README's "Run a proof experiment" section, plus a wiki page documenting role design and prompt engineering decisions.
- Optional Cardano-tx-validation feature on `omega-mock-ledger` is unchanged; goblins do NOT submit Cardano txs in v0.1.
- Out of scope: real production deployment of goblins (they are a simulation tool, not part of the consensus protocol); fine-tuning Gemma-4 (we use the base instruct model with role prompts only).
