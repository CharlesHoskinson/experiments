# Design: Goblin agentic framework + omega-api

## Goal

Two things in one change because they ship together:

1. A stable, machine-readable API in front of the proof-experiment harness so anything (CLI, simulator, browser, future wallet) can drive the cluster through one versioned surface.
2. A multi-agent simulation framework — Goblins — that exercises that API at parameterizable scale, with role-playing behaviour driven by a local Gemma-4 E4B model.

These are coupled because the goblin framework has no special access to the harness — it talks to the same public API every other client uses. The API is therefore the contract; the goblins are the first heavy user of it.

## Architecture

```
                                 ┌─────────────────────────────────────┐
                                 │         omega-goblin-runner         │
                                 │                                     │
                                 │  ┌─────┬──────┬─────────┬────────┐  │
                                 │  │Hold │Whale │Adversary│Lurker  │  │
                                 │  │Hold │Whale │Adversary│Lurker  │  │
                                 │  │ ... │ ...  │   ...   │ ...    │  │
                                 │  └─────┴──────┴─────────┴────────┘  │
                                 │     N goblins, role mix from CLI    │
                                 └────┬──────────────────┬─────────────┘
                                      │                  │
                          observe-plan-act         LLM plan call
                                      │                  │
                                      ▼                  ▼
                  ┌──────────────────────────────┐    ┌─────────────────────────┐
                  │      omega-api (axum)        │    │  Local LLM endpoint     │
                  │  /v1/submit  /v1/state       │    │  default: Ollama        │
                  │  /v1/proof/{id}              │    │  http://127.0.0.1:11434 │
                  │  /v1/genesis                 │    │  model: gemma4:e4b      │
                  │  /v1/peers                   │    │                         │
                  │  /v1/metrics  (prometheus)   │    │  swappable via          │
                  │  WebSocket /v1/events        │    │  `LlmClient` trait:     │
                  │  OpenAPI at /v1/openapi.json │    │   - OllamaClient        │
                  └────────────┬─────────────────┘    │   - LlamaCppClient      │
                               │                      │   - MockLlmClient       │
                       per-node               (default for CI / no-GPU runs)
                               │
                               ▼
                  ┌──────────────────────────────┐
                  │  omega-toy-consensus + Raft   │
                  │  (existing harness)            │
                  └──────────────────────────────┘
```

`omega-api` runs as a tokio task inside each Raft node's process and listens on `127.0.0.1:{8001,8002,8003}` (one port per node). The goblin runner picks one endpoint at random per request (or round-robin) — a real-network correctness property is "any node can answer; followers transparently forward writes to the leader."

## omega-api design

### Surface

- `POST /v1/submit` — accept a `ClaimTx` (CBOR or JSON), forward to leader if not on this node, return `{ accepted: bool, applied_at_log_idx: u64?, error: ApiError? }`.
- `GET /v1/state` — return summary `{ nullifier_count: u64, starstream_utxo_count: u64, last_applied_log_idx: u64, leader_id: u64, term: u64 }`.
- `GET /v1/state/nullifiers?cursor=&limit=` — paginated nullifier listing for inspection.
- `GET /v1/state/starstream-utxos?cursor=&limit=` — paginated UTxO listing.
- `GET /v1/proof/{public_input_hash}` — retrieve the stored proof bytes for a previously-applied claim (for verification by other nodes / wallets).
- `GET /v1/genesis` — return the pinned genesis params (Ω-Commitment, hash domain tags, era bytes, sub-tree item-counts).
- `GET /v1/peers` — list libp2p peers and their Raft membership status.
- `GET /v1/metrics` — Prometheus text-format metrics (apply latency histograms, raft state gauges, goblin role counters when goblins are active).
- `WS /v1/events` — server-side event stream of `{type: applied, log_idx, public_input_hash}`, `{type: leader_change, new_leader_id, term}`, `{type: snapshot_started, idx}`, etc. Goblins subscribe to drive observe loops without polling.
- `GET /v1/openapi.json` — auto-generated OpenAPI 3.1 spec (via `utoipa`), used by client codegen and by humans inspecting the API.

### Versioning + stability

- Path-prefixed (`/v1/`). Adding routes is non-breaking; renaming or removing requires `/v2/` and dual-running both for a deprecation window.
- Every response carries an `X-Omega-Api-Version: 0.1` header.
- The OpenAPI spec is the source of truth; CI lints PRs that change route shapes against the previous main's spec via `openapi-diff`.

### Non-goals

- Authentication / authorization. The harness is local. Adding auth is out of scope for v0.1.
- HTTPS. The API binds to localhost only.
- Multi-tenancy. One harness instance, one set of nodes.

## Goblin design

### Role traits

Every goblin implements:

```rust
#[async_trait]
pub trait Goblin: Send + Sync {
    fn role(&self) -> Role;
    fn id(&self) -> GoblinId;

    /// Tick once: observe state via API, plan via LLM, act via API.
    /// Returns the action taken and any structured reasoning trace.
    async fn tick(&mut self, ctx: &GoblinContext) -> Result<Tick, GoblinError>;
}

pub struct Tick {
    pub action: Action,
    pub reasoning: String,         // captured for log analysis
    pub llm_tokens_in: usize,
    pub llm_tokens_out: usize,
    pub wallclock_ms: u64,
}
```

`GoblinContext` carries: an `ApiClient` pointing at one or more `omega-api` endpoints, an `LlmClient`, the goblin's local memory (a small in-process state cache), the synthetic genesis fixture (so goblins can construct witnesses without re-fetching), and a tracing span ID.

### Action types

```rust
pub enum Action {
    SubmitClaim { kind: ClaimKind, leaf_indices: Vec<u64> },
    QueryState,
    QueryNullifiers { cursor: Option<u64>, limit: u32 },
    QueryProof { public_input_hash: [u8; 32] },
    SubscribeEvents,
    Sleep { duration_ms: u32 },
    Shutdown,
}
```

The `Action` enum is the bridge between the LLM plan and the executor. The LLM produces a JSON-shaped plan; a deterministic Rust parser maps it into an `Action`. If parsing fails, the goblin falls back to a deterministic per-role default action (e.g. a Holder defaults to `SubmitClaim { kind: Utxo, leaf_indices: vec![pick_random_unclaimed()] }`).

### Per-role behaviour

- **Holder.** Goal: claim one previously-unclaimed leaf, observe it apply, then idle. LLM prompt frames the goblin as "a normal Cardano holder migrating their UTxO to Omega; you have one Ed25519-derived PQ key; you want to submit a single claim." Tick budget: ~1 LLM call per minute.
- **Whale.** Goal: claim a batch of K leaves (K ∈ [10, 1024]) in a single `ClaimCollection`. LLM prompt asks the goblin to pick a batch size given current network state (apply latency on `/v1/metrics`, recent leader churn). Tick budget: ~1 LLM call per 5 minutes.
- **Adversary.** Goal: get rejected. Rotates strategies: replay a known-good claim, tamper a proof byte, submit malformed CBOR, submit a claim against a wrong bundle root. Each tick the LLM picks a strategy from a closed list, executes, and asserts the API returned a non-`Ok` status. Failure to be rejected is a HARNESS bug and is logged loudly. Tick budget: ~1 LLM call per 30 seconds.
- **Lurker.** Goal: passive observer. Subscribes to `WS /v1/events`, polls `/v1/state` every 30 s, emits a structured snapshot to its log. The LLM prompt asks the lurker to *summarise* the state in plain English every M ticks — useful for human readers of the log to skim what happened during a 30 min run. Tick budget: ~1 LLM call per 5 minutes.
- **SnapshotServer.** Goal: serve Merkle-path requests over a libp2p protocol that the harness's clients (Holders, Whales) consult before constructing claims. Stands in for the future T5 mirror partnerchain. The LLM is barely used here — only to generate human-readable status messages. Tick budget: ~1 LLM call per 10 minutes.
- **Validator.** Goal: stress the consensus surface. The validator goblin runs *outside* the Raft cluster and acts via a sidecar control channel that can request controlled disruptions: pause node 2 for 10 s, sever node 1's network for 5 s, force a snapshot. The LLM picks a disruption from a closed list. The cluster is expected to recover; if it doesn't, the validator emits a loud alert. Tick budget: ~1 LLM call per minute.

### LLM integration

```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn plan(&self, prompt: &Prompt) -> Result<Plan, LlmError>;
}

pub struct Prompt {
    pub system: String,        // role-specific system prompt
    pub user: String,          // structured observation summary
    pub max_output_tokens: u32,
    pub temperature: f32,
}

pub struct Plan {
    pub action_json: String,   // JSON shape parsed into Action by the goblin
    pub reasoning: String,     // free-form, captured for log
    pub tokens_in: usize,
    pub tokens_out: usize,
}

pub struct OllamaClient { url: String, model: String }
pub struct LlamaCppClient { url: String }
pub struct MockLlmClient { script: Vec<Plan> }
```

Default model: `gemma4:e4b` via Ollama at `http://127.0.0.1:11434`. The user runs `ollama pull gemma4:e4b` once before starting the goblin runner. CI uses `MockLlmClient` so no GPU/Ollama is required. A future enhancement (out of scope here) embeds llama.cpp directly via `llama-cpp-rs` for a single-binary deploy.

### Goblin scheduling

`omega-goblin-runner` uses a tokio multi-thread runtime with a configurable worker pool. Each goblin runs as one tokio task; tasks share the global `ApiClient` connection pool and a global `LlmClient` (Ollama serializes its inference, so only one goblin's plan call runs at a time even if 50 goblins are scheduled — this is intentional: it bounds GPU/CPU pressure).

Backpressure: if the LLM endpoint is saturated, the goblin scheduler issues fewer ticks per goblin per minute and writes a warning to its metrics gauge. Goblins with deterministic fallback actions (e.g. Holder's `SubmitClaim` default) still progress without LLM calls when the LLM is offline; goblins whose role is centrally about LLM-driven planning (e.g. Adversary, Validator) skip the tick instead.

### Determinism + reproducibility

Goblin runs are not bit-deterministic (LLM sampling is stochastic; `temperature > 0`). Reproducibility is approximate: `--seed N` fixes the goblin scheduling order, the role assignment, the synthetic-genesis fixture, and the LLM `seed` parameter (Ollama supports it). With a fixed seed and `MockLlmClient`, a run is fully deterministic. With a fixed seed and a real LLM, the same script runs, but the LLM's output may drift across model versions.

### Logging + metrics

- Per-tick structured log line via `tracing`: `goblin.tick {goblin_id, role, action, accepted, reasoning_summary, tokens_in, tokens_out, wallclock_ms}`.
- Prometheus metrics on `/v1/metrics`:
  - `goblin_ticks_total{role}`
  - `goblin_actions_total{role, action_type, accepted}`
  - `goblin_llm_tokens_total{role, direction}`
  - `goblin_llm_latency_seconds{role, direction}` (histogram)
- A separate Grafana dashboard JSON ships in `crates/omega-goblin-runner/dashboards/goblins.json` (out of scope to actually run a Grafana, but the dashboard JSON is committed for posterity).

## Failure modes (tracked, not closed)

- **LLM goes off-the-rails.** Gemma-4 E4B at temp=0.7 sometimes emits malformed JSON. The goblin parser falls back to the deterministic per-role default action and increments a `goblin_llm_parse_failures_total{role}` counter. If the rate exceeds 10% over 5 min, the goblin runner emits a loud warning.
- **Adversary keeps getting accepted.** This is a harness bug, not a goblin bug. The runner asserts on every Adversary tick that the API returned a rejection; on success, the runner crashes with a verbose error pointing at the proof bytes that should have been rejected. (This is a feature: adversaries are how we surface harness regressions.)
- **Ollama is not running.** `OllamaClient::plan` returns `Err(LlmError::Connection)`; the runner switches roles with deterministic fallbacks to "no-LLM" mode and continues. Roles that cannot run without LLM (Adversary, Validator) emit warnings and skip ticks.
- **Goblin count is too high for the LLM.** With 100 goblins and Ollama serializing inference at ~5 s/plan, each goblin gets ~1 plan / 8 minutes — the load mix is real but the LLM is the bottleneck. Documented as an expected cost; the runner's metrics surface the bottleneck.
- **API path-version drift.** If `omega-api` v2 is introduced before the goblin runner updates, goblins emit 404s. Mitigation: every goblin sends `Accept: application/json; version=1` and the API rejects mismatched versions with a clear error.
- **Validator-goblin loses the cluster.** If the goblin tells node 2 to pause and node 2 never comes back, the cluster degrades to 2-of-3 (still quorate) but eventually loses a node. The validator role tracks node liveness and emits a panic if it inadvertently kills the cluster.

## Test strategy

- Unit tests per crate.
- `omega-api/tests/openapi_stability.rs` — generates the OpenAPI spec on every build, asserts the JSON shape matches a committed `tests/openapi.snapshot.json`. Updates require an explicit task (and a major-version bump if a route changes).
- `omega-goblin-runner/tests/mock_llm_smoke.rs` — runs 5 goblins (1 of each non-Validator role) for 30 seconds against an in-process 3-node harness with `MockLlmClient`; asserts at least one Holder claim applied, one Adversary rejection, one Lurker observation logged.
- `tests/e2e_goblin_run.rs` (gated on `RUN_GOBLIN_E2E=1`, off in CI by default) — runs a 5-minute live simulation with `--holders 5 --adversaries 2 --lurkers 1` against Ollama; asserts cluster stayed healthy throughout.

## What this design defers

- Real GPU profiling of Gemma-4 E4B latency under N concurrent goblins. We assume Ollama serializes; v0.2 may stand up multiple Ollama instances and load-balance.
- llama-cpp-rs in-process embedding. v0.1 ships only the HTTP-Ollama path.
- Scenario scripting (a goblin DSL where a developer hand-writes "first 10 holders submit, then 1 adversary replays each one, then 1 whale submits 1024-leaf"). v0.1 is purely role-mix-driven; complex scenarios run multiple separate `omega-goblins run` invocations with different mixes.
- Distributed multi-machine goblin runners. v0.1 is one runner process; if you want 1000 goblins, run them on one beefy box.
