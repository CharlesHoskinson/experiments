## ADDED Requirements

### Requirement: Versioned REST + JSON-RPC interface

The `omega-api` crate SHALL expose a versioned HTTP/1.1 API at `/v1/` on every Raft node, listening on `127.0.0.1:{8001,8002,8003}` for nodes 1/2/3 respectively. Every response MUST carry an `X-Omega-Api-Version` header equal to `0.1`. Adding a route is non-breaking; renaming or removing a route requires `/v2/` and a documented dual-running deprecation window.

#### Scenario: Health probe returns version header
- **WHEN** a client sends `GET /v1/state` to any of the three nodes
- **THEN** the response status is 200 and the `X-Omega-Api-Version` header equals `0.1`

#### Scenario: Mismatched version is rejected
- **WHEN** a client sends `GET /v2/state` against the v0.1 build
- **THEN** the response status is 404 and the body explains that `/v2` is not yet served

### Requirement: Submit endpoint forwards to leader transparently

`POST /v1/submit` SHALL accept a `ClaimTx` (CBOR or JSON) and return `{accepted, applied_at_log_idx, error}`. When called against a follower, the API node MUST forward the request to the current leader through openraft's `client_write` API; the caller SHALL NOT need to know which node is the leader.

#### Scenario: Submit against a follower succeeds
- **WHEN** a client submits a valid claim via `POST /v1/submit` to a follower's endpoint
- **THEN** the response status is 200, `accepted` is `true`, and `applied_at_log_idx` is set to the log index where the entry committed

#### Scenario: Submit during leader change retries cleanly
- **WHEN** a client submits a claim during a leader-change event
- **THEN** the API node either retries internally and returns 200, or returns 503 with a `Retry-After: 1` header — never returns an inconsistent acknowledgement

### Requirement: OpenAPI 3.1 spec is auto-generated

`GET /v1/openapi.json` SHALL return an OpenAPI 3.1 document describing every route, request body schema, and response shape. The spec MUST be auto-generated from the Rust route handlers via `utoipa` so it cannot drift from the implementation.

#### Scenario: OpenAPI document is parseable
- **WHEN** a client fetches `/v1/openapi.json` and feeds it to a standard OpenAPI 3.1 parser
- **THEN** the parser accepts the document without errors

#### Scenario: Adding a route updates the document
- **WHEN** a developer adds a new route handler annotated with `utoipa::path`
- **THEN** the next response from `/v1/openapi.json` includes the new route without any manual edits

### Requirement: Server-sent events for low-latency goblin observation

`GET /v1/events` (WebSocket upgrade) SHALL stream apply, leader-change, and snapshot events to subscribed clients in JSON-line format. The WebSocket MUST send a heartbeat ping every 15 seconds; subscribers MUST be disconnected after 60 s of missed pongs.

#### Scenario: Apply event is delivered
- **WHEN** a client subscribes to `/v1/events` and a claim is applied to log index 42
- **THEN** the client receives a JSON line `{"type":"applied","log_idx":42,"public_input_hash":"..."}` within 100 ms of apply

### Requirement: Prometheus metrics endpoint

`GET /v1/metrics` SHALL return Prometheus text-format metrics covering at minimum: apply latency histogram, raft state gauges (leader/follower/candidate), submit-rejected-total counter (by error type), and goblin-action counters (when goblins are active).

#### Scenario: Metrics endpoint is scrape-able
- **WHEN** Prometheus scrapes `GET /v1/metrics`
- **THEN** the response is text/plain with valid Prometheus exposition format and includes at least the four families above

## MODIFIED Requirements

### Requirement: CLI orchestrator

The `omega-experiment` binary SHALL provide four subcommands — `prove`, `submit`, `state`, `bench` — that together exercise the full prove → submit → apply → query loop on a local 3-node quorum. **Submit, state, and bench MUST go through the `omega-api` HTTP surface** (not the internal JSON-RPC channel that v0.1 of the harness shipped). Each subcommand MUST produce machine-readable output via a `--json` flag and human-readable output by default.

#### Scenario: prove subcommand writes a proof file
- **WHEN** `omega-experiment prove --commit var/bundle.json --leaves var/leaves.json --out var/proof.bin` is run
- **THEN** the command exits with code 0 and `var/proof.bin` exists with non-zero size

#### Scenario: submit subcommand uses the v1 API
- **WHEN** `omega-experiment submit --node http://127.0.0.1:8001 --proof var/proof.bin` is run against a healthy 3-node cluster
- **THEN** the command exits with code 0 within 5 seconds, the request hits `POST /v1/submit`, and node 1's `state` query reports the new nullifier

#### Scenario: bench reports latency percentiles
- **WHEN** `omega-experiment bench --leaves 100 --commit var/bundle.json --json` is run against the v1 API
- **THEN** the command emits a JSON object with at least `prove_p50_ms`, `prove_p95_ms`, `submit_p50_ms`, `submit_p95_ms` fields
