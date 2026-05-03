## MODIFIED Requirements

### Requirement: CLI orchestrator

The `omega-experiment` binary SHALL provide four subcommands — `prove`, `submit`, `state`, `bench` — that together exercise the full prove → submit → apply → query loop on a local 3-node quorum. **Submit, state, and bench MUST go through the `omega-api` HTTP surface introduced in this change** (not the internal JSON-RPC channel that v0.1 of the harness shipped). Each subcommand MUST produce machine-readable output via a `--json` flag and human-readable output by default.

#### Scenario: prove subcommand writes a proof file
- **WHEN** `omega-experiment prove --commit var/bundle.json --leaves var/leaves.json --out var/proof.bin` is run
- **THEN** the command exits with code 0 and `var/proof.bin` exists with non-zero size

#### Scenario: submit subcommand uses the v1 API
- **WHEN** `omega-experiment submit --node http://127.0.0.1:8001 --proof var/proof.bin` is run against a healthy 3-node cluster
- **THEN** the command exits with code 0 within 5 seconds, the request hits `POST /v1/submit`, and node 1's `state` query reports the new nullifier

#### Scenario: bench reports latency percentiles
- **WHEN** `omega-experiment bench --leaves 100 --commit var/bundle.json --json` is run against the v1 API
- **THEN** the command emits a JSON object with at least `prove_p50_ms`, `prove_p95_ms`, `submit_p50_ms`, `submit_p95_ms` fields
