# Tasks

Implementation order. Depends on `add-proof-experiment-harness` having shipped (or at least having its `omega-toy-consensus` JSON-RPC stub mergeable into the new `omega-api`). Each task is gated on `cargo test --workspace --no-fail-fast` and `cargo clippy --workspace --all-targets -- -D warnings` passing.

## 1. omega-api crate

- [ ] 1.1 Create `crates/omega-api/` with deps: `axum = "0.7"`, `tower = "0.5"`, `tower-http = { version = "0.6", features = ["cors", "trace"] }`, `tokio` (workspace), `utoipa = { version = "5", features = ["axum_extras"] }`, `utoipa-swagger-ui` (optional, for `/v1/docs`), `prometheus = "0.13"`, `tracing` (workspace), `serde` + `serde_json`, `omega-claim-tx` + `omega-toy-consensus` (path deps).
- [ ] 1.2 Define request/response types with `#[derive(utoipa::ToSchema)]` so OpenAPI auto-gen captures the shape.
- [ ] 1.3 Implement route handlers for: `POST /v1/submit`, `GET /v1/state`, `GET /v1/state/nullifiers`, `GET /v1/state/starstream-utxos`, `GET /v1/proof/{public_input_hash}`, `GET /v1/genesis`, `GET /v1/peers`, `GET /v1/metrics`, `WS /v1/events`, `GET /v1/openapi.json`.
- [ ] 1.4 Wire `POST /v1/submit` to `omega_toy_consensus::ToyConsensusNode::submit`, transparently forwarding to the leader if the request hit a follower.
- [ ] 1.5 Add `X-Omega-Api-Version: 0.1` header to every response via a tower middleware.
- [ ] 1.6 Generate the OpenAPI document via `utoipa::OpenApi` derive on a single `ApiDoc` struct that lists all handlers; serve from `/v1/openapi.json`.
- [ ] 1.7 Implement Prometheus exposition: register histograms / counters / gauges; export from `/v1/metrics` via `prometheus::TextEncoder`.
- [ ] 1.8 Implement `WS /v1/events` using axum's WebSocket support; back it with a `tokio::sync::broadcast::Sender<Event>` that the consensus node fills on apply / leader-change / snapshot events.
- [ ] 1.9 Add `tests/api_smoke.rs`: spin up an in-process `omega-api` against a mock `ToyConsensusNode`, hit each route, assert response shapes match the OpenAPI schema.
- [ ] 1.10 Add `tests/openapi_stability.rs`: generate the OpenAPI document, compare to `tests/openapi.snapshot.json`. Fails if the document drifts; updating requires the developer to bump the snapshot file deliberately.
- [ ] 1.11 Update `omega-experiment` CLI to use `omega-api` HTTP endpoints (drop the direct JSON-RPC). This is the spec.md MODIFIED requirement.

## 2. omega-goblin-core crate

- [ ] 2.1 Create `crates/omega-goblin-core/` with deps: `async-trait = "0.1"`, `serde` + `serde_json`, `tokio`, `reqwest = { version = "0.12", features = ["json"] }`, `tracing`, `prometheus`, `omega-claim-tx` (path).
- [ ] 2.2 Define `Goblin` trait, `Role`, `GoblinId`, `Tick`, `Action`, `GoblinContext`, `LlmClient` trait, `Prompt`, `Plan`.
- [ ] 2.3 Implement `OllamaClient` against the Ollama HTTP API (`POST /api/generate` with `model: "gemma4:e4b"`).
- [ ] 2.4 Implement `LlamaCppClient` against llama.cpp's `--api` mode.
- [ ] 2.5 Implement `MockLlmClient { script: Vec<Plan> }` that returns scripted plans in order, looping when exhausted.
- [ ] 2.6 Implement deterministic-fallback Action picker per role (used when LLM fails or returns malformed JSON).
- [ ] 2.7 Implement structured-log emission per tick via `tracing::info_span!` so logs are usable for analysis.
- [ ] 2.8 Add unit tests for the Action JSON parser (round-trip + malformed input).

## 3. Per-role goblin implementations

- [ ] 3.1 Implement `HolderGoblin`. Memory: list of unclaimed leaves; submitted-but-not-yet-applied claim queue; observed apply events. Tick: observe → plan → submit one claim; deterministic fallback picks a random unclaimed leaf.
- [ ] 3.2 Implement `WhaleGoblin`. Same as Holder but submits `ClaimCollection` of K leaves; LLM picks K based on observed apply latency.
- [ ] 3.3 Implement `AdversaryGoblin`. Closed-list strategies: replay, tamper-proof-byte, malformed-CBOR, wrong-bundle-root. On accepted-but-should-have-rejected: panic the runner with a verbose error.
- [ ] 3.4 Implement `LurkerGoblin`. Subscribes to `WS /v1/events`; polls `/v1/state` every 30 s; LLM summarises every M ticks for human readability.
- [ ] 3.5 Implement `SnapshotServerGoblin`. Hosts a libp2p protocol that serves Merkle-path requests. LLM is barely used — only for status messages.
- [ ] 3.6 Implement `ValidatorGoblin`. Operates via a sidecar admin channel (NOT a Raft impersonation). Closed-list disruptions: pause node, sever node's network, force snapshot. Tracks cluster liveness; panics on cluster-not-recovered.

## 4. omega-goblin-runner binary

- [ ] 4.1 Create `crates/omega-goblin-runner/` with deps: all of the above, plus `clap`, `prometheus_exporter` (or just expose a `/metrics` endpoint via axum on a separate port).
- [ ] 4.2 Implement CLI: `omega-goblins run --holders N --whales N --adversaries N --lurkers N --snapshot-servers N --validators N --duration <dur> --llm <url>|mock|llamacpp:<url> --api <node-url>... --seed N`.
- [ ] 4.3 Spawn one tokio task per goblin; share a single `LlmClient` instance to bound concurrent LLM inflight (Ollama serializes anyway).
- [ ] 4.4 Implement scheduling backpressure: if the LLM endpoint is saturated, reduce per-goblin tick rate proportionally.
- [ ] 4.5 Implement the `--seed` flag for approximate-reproducibility (fixed scheduling, fixed role assignment, fixed LLM `seed` parameter passed through to Ollama).
- [ ] 4.6 Expose Prometheus metrics on `:9090` (configurable via `--metrics-port`).
- [ ] 4.7 Add `tests/mock_llm_smoke.rs` per the spec scenario: 5 goblins for 30 s with `MockLlmClient` against an in-process 3-node harness; assert at least one Holder apply, one Adversary rejection, one Lurker observation.
- [ ] 4.8 Add `tests/e2e_goblin_run.rs` (gated on `RUN_GOBLIN_E2E=1`): 5-min live run with real Ollama; assert cluster stayed healthy.

## 5. Documentation

- [ ] 5.1 Add a "Run a goblin simulation" subsection to the README's "Run a proof experiment" section: ≤ 8 commands covering `ollama pull gemma4:e4b`, building, spawning a 10-goblin mix, observing metrics, shutting down.
- [ ] 5.2 Write `cardano-wiki/wiki/pages/omega-goblin-framework.md` documenting the role taxonomy, prompt-engineering decisions, and the LLM-offline fallback semantics.
- [ ] 5.3 Append a `cardano-wiki/wiki/log.md` entry tagged `plan | goblin-agentic-framework`.
- [ ] 5.4 Commit `crates/omega-goblin-runner/dashboards/goblins.json` (a Grafana JSON skeleton) so anyone running Prometheus + Grafana can see goblin activity at a glance.

## 6. CI

- [ ] 6.1 Add a CI job that runs `cargo test --workspace --features cardano-tx-validation` (existing) plus the goblin mock smoke (`cargo test --package omega-goblin-runner --test mock_llm_smoke`).
- [ ] 6.2 Skip the `e2e_goblin_run` test in CI by default; gate behind `RUN_GOBLIN_E2E=1` env var.
- [ ] 6.3 Add an OpenAPI-stability check: `cargo test --package omega-api --test openapi_stability` runs on every PR.

## 7. Validation gate

- [ ] 7.1 `cargo test --workspace --no-fail-fast` green.
- [ ] 7.2 `cargo fmt --check` clean.
- [ ] 7.3 `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] 7.4 `openspec validate add-goblin-agentic-framework --strict` green.
- [ ] 7.5 `omega-goblins run --holders 5 --adversaries 2 --lurkers 1 --duration 30s --llm mock` exits with code 0 and emits non-zero `goblin_ticks_total` for each role.
- [ ] 7.6 OpenAPI document at `/v1/openapi.json` is parseable by a standard OpenAPI 3.1 parser.
