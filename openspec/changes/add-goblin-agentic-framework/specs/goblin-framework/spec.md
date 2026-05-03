## ADDED Requirements

### Requirement: Parameterizable goblin role mix

The `omega-goblin-runner` binary SHALL spawn N goblins per role from a CLI-specified mix and run them concurrently against an `omega-api` endpoint for a configurable duration. Roles in v0.1 are: `Holder`, `Whale`, `Adversary`, `Lurker`, `SnapshotServer`, `Validator`. The CLI SHALL accept `--<role-plural> <N>` flags that default to zero so an operator can spawn any subset.

#### Scenario: All-zero spawn produces no goblins
- **WHEN** `omega-goblins run --duration 1m` (no role flags) is invoked
- **THEN** the runner exits cleanly after 1 minute with zero goblin ticks reported in metrics

#### Scenario: Role mix spawns N goblins per role
- **WHEN** `omega-goblins run --holders 5 --adversaries 2 --lurkers 1 --duration 30s --llm mock` is invoked
- **THEN** within 5 seconds the runner reports 5 Holder + 2 Adversary + 1 Lurker = 8 goblins running, and metrics show non-zero `goblin_ticks_total{role}` for each role

### Requirement: Pluggable LlmClient with three implementations

The framework SHALL provide an `LlmClient` trait with at least three implementations: `OllamaClient` (default; talks to a local Ollama HTTP at `:11434`), `LlamaCppClient` (talks to llama.cpp's `--api` mode), and `MockLlmClient` (deterministic, no network, for CI). The runner CLI MUST accept `--llm <url>|mock|llamacpp:<url>` to select the client.

#### Scenario: Default Ollama client is selected when --llm is omitted
- **WHEN** `omega-goblins run --holders 1 --duration 10s` is invoked without `--llm`
- **THEN** the runner uses `OllamaClient` against `http://127.0.0.1:11434` with model `gemma4:e4b`

#### Scenario: Mock LLM allows CI without GPU
- **WHEN** `omega-goblins run --holders 3 --llm mock --duration 30s` is invoked on a machine with no Ollama and no GPU
- **THEN** the runner exits with code 0 and reports goblin ticks; no network calls to `:11434` are made

#### Scenario: Ollama unreachable falls back gracefully
- **WHEN** `omega-goblins run --holders 3 --llm http://127.0.0.1:11434` is invoked and Ollama is not running
- **THEN** the runner emits a warning, switches Holder/Whale/Lurker to deterministic-fallback mode (no LLM), and exits with code 0 after the configured duration; Adversary and Validator emit `goblin_llm_offline_skips_total` counters and produce no actions

### Requirement: Goblin observe-plan-act loop

Each goblin SHALL implement an observe-plan-act loop where it (a) reads cluster state via `omega-api`, (b) calls `LlmClient::plan` to produce a structured plan in JSON, (c) parses the JSON into an `Action`, (d) executes the action via `omega-api`, and (e) emits a structured tracing event. If the LLM returns malformed JSON, the goblin MUST fall back to a deterministic per-role default action and increment a `goblin_llm_parse_failures_total{role}` counter.

#### Scenario: Holder goblin completes one full cycle
- **WHEN** a Holder goblin ticks once with `MockLlmClient` returning a valid `SubmitClaim` plan
- **THEN** the cluster's nullifier set grows by exactly one entry corresponding to the goblin's chosen leaf

#### Scenario: Malformed LLM output triggers deterministic fallback
- **WHEN** a Holder goblin ticks with `MockLlmClient` returning the literal string `"not json"`
- **THEN** the goblin emits `goblin_llm_parse_failures_total{role="Holder"}=1` and falls back to its deterministic action (submit a single-leaf claim against an unclaimed leaf)

### Requirement: Adversary goblin asserts rejection

The `Adversary` role SHALL submit only invalid claims (replays, byte-tampered proofs, malformed CBOR, wrong-bundle-root attacks). On every Adversary tick the runner MUST verify the API returned a non-`Ok` status; if an Adversary submission is *accepted*, the runner SHALL crash with a verbose error pointing at the offending bytes.

#### Scenario: Adversary replay is rejected
- **WHEN** an Adversary goblin replays a previously-applied claim
- **THEN** the API returns `accepted=false` with `error.kind="NullifierExists"` and the Adversary records a successful "rejection-as-expected" tick

#### Scenario: Adversary success is treated as a harness bug
- **WHEN** an Adversary submission is *accepted* (which should be impossible)
- **THEN** the runner emits a panic-level log with the proof bytes, the public inputs, and the API response, and exits with code 2

### Requirement: Validator goblin runs outside the cluster

The `Validator` role SHALL operate via a sidecar control channel (a separate libp2p protocol or an admin API) and MUST NOT impersonate a Raft node. Validator goblins request controlled disruptions from a closed list (pause node, sever node's network for N seconds, force a snapshot). The runner SHALL track cluster liveness across disruptions and emit a panic-level log if the cluster fails to recover within the documented recovery window.

#### Scenario: Pause-then-resume preserves quorum
- **WHEN** a Validator goblin pauses node 2 for 5 seconds via the control channel
- **THEN** the cluster (nodes 1 and 3 remaining) maintains quorum throughout, and node 2 rejoins the cluster within 5 seconds of resume

### Requirement: Mock-LLM smoke test runs in CI

The framework SHALL include an integration test `omega-goblin-runner/tests/mock_llm_smoke.rs` that runs 5 goblins (1 Holder, 1 Whale, 1 Adversary, 1 Lurker, 1 SnapshotServer; 0 Validator) for 30 seconds against an in-process 3-node harness with `MockLlmClient`. The test MUST assert at least one Holder claim applied, one Adversary rejection observed, one Lurker observation logged. CI SHALL run this test on every PR.

#### Scenario: Mock smoke completes within 60 seconds
- **WHEN** `cargo test --package omega-goblin-runner --test mock_llm_smoke` is run on a developer laptop
- **THEN** the test completes within 60 seconds (30 s simulation + 30 s setup/teardown overhead) and exits with code 0
