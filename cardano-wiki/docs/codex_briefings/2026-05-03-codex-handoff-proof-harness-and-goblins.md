---
date: 2026-05-03
recipient: Codex GPT-5.5 xhigh
mode: autonomous implementation
scope: two OpenSpec changes shipped end-to-end
status: ready for handoff
---

# Codex handoff: Omega proof-experiment harness + Goblin agentic framework

You are Codex GPT-5.5 xhigh. You are picking up an end-to-end implementation in Rust that has been fully designed and reviewed; your job is to write the code, ship it green through CI, and produce a working developer-laptop demo. The design is fixed. Do not revisit architectural decisions unless you find a soundness issue, and if you do, raise it before changing scope.

## What ships

Two OpenSpec changes, in order. Both must end at `cargo test --workspace --no-fail-fast` green and `openspec archive <change-name>` succeeding.

1. **`add-proof-experiment-harness`** — local 3-node Raft (openraft 0.9) + libp2p 0.55 + rusqlite 0.32 + Plonky3 (git rev pinned) harness that round-trips a proof of Merkle inclusion against an Ω-Commitment. 7 new crates, 1 README section, ~25 tests. See `openspec/changes/add-proof-experiment-harness/`.
2. **`add-goblin-agentic-framework`** — `omega-api` (axum REST + WS + OpenAPI) on top of the harness, plus `omega-goblin-{core,runner}` for parameterizable Gemma-4 E4B agents that role-play on the network. 3 new crates, 1 README subsection, OpenAPI snapshot test, mock-LLM CI smoke. See `openspec/changes/add-goblin-agentic-framework/`.

Read both `proposal.md`, `design.md`, every file under `specs/*/spec.md`, and `tasks.md` for each change before writing any code. Read the `QA-REVIEW.md` in change #1 — every P0 has been addressed in the design but the issues are real and apply during implementation too.

## Repo state when you start

- `omega-commitment/` workspace at v0.10.0-rc1 (Blake3, 292 tests green). Don't break it.
- The seven sub-tree leaf encodings, `leaf_hash_v2`, `node_hash_v2`, the dual-hash bundle root tuple — all live in `omega-commitment-core` and `omega-commitment-bundle`. Use them as-is.
- `audit/` carries the 2026-05-02 ten-agent Codex audit and the 2026-05-03 fresh-pass synthesis. The findings are closed; you don't need to address them.
- `var/upstream/` has Plonky3, Starstream, amaru, pallas cloned for reference reading. **Do not depend on local paths in production code.** Read the source for API verification, then add the dep via crates.io / git rev.
- `cardano-wiki/wiki/log.md` is append-only. Add a `plan` entry per major task batch.
- OpenSpec is initialized at the repo root. Use `openspec validate <change>` after every artifact edit.

## Toolchain (pinned)

- Rust **1.79** stable. Do not require nightly. (Amaru uses nightly; we don't.)
- Cargo edition 2021.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --no-fail-fast` are CI gates.
- Plonky3: pin a 40-char git rev once in `[workspace.dependencies]`. Capture the rev at first build. Don't double-pin per crate.
- openraft `= "0.9"`. Verify trait names and signatures against the actual pinned version's docs before claiming the impl works.
- libp2p `= "0.55"`. Default-features off; opt in only the protocols listed in the design.
- rusqlite `= "0.32"` with `bundled` feature.
- pallas-validate `= "=1.0.0-alpha.6"` behind `cardano-tx-validation` feature, default off.
- axum `= "0.7"`, utoipa `= "5"`, prometheus `= "0.13"`, reqwest `= "0.12"` (json feature).

## Implementation rules

1. **Follow the design verbatim.** The actor-pattern (mpsc + dedicated OS thread) for the SQLite writer is required, not optional. The snapshot wire protocol is the one in `add-proof-experiment-harness/design.md` ("Snapshot wire protocol — explicit"). Gossipsub is intentionally dropped from v0.1. The leaf-preimage soundness restriction is ≤ 64 bytes for v0.1.
2. **Verify upstream APIs before writing test code.** If the design says "openraft's `RaftStateMachine::apply` returns a future" and the actual trait differs in the pinned version, fix the design, not the test. Read `var/upstream/pallas/pallas-validate/src/phase1/mod.rs` for the actual `validate_tx` signature before writing the Cardano-tx test.
3. **Ship one task list at a time.** Mark tasks done in `tasks.md` as you complete them. After every group (1.x, 2.x, ...) re-run the validation gate.
4. **Don't break v0.10.0-rc1.** The 292 existing tests must stay green. If you need to expose a new API on `omega-commitment-core`, add it without changing existing surfaces.
5. **No AI attribution in commits.** Commits authored as `charles hoskinson <charles.hoskinson@gmail.com>`. No Co-Authored-By lines. No "🤖" emoji. Match the existing commit style in `git log` (lowercase tag prefix, terse subject, bulleted body).
6. **Working in a git worktree is fine** if you want isolation. Otherwise commit to a feature branch and PR into main.
7. **Openraft is async, rusqlite is sync.** This is the dominant integration hazard. Do not reach for `tokio::task::spawn_blocking` per call — that is the design's explicit anti-pattern. Use the actor pattern: dedicated OS thread owning the `Connection`, mpsc channel for writes, oneshot replies. The heartbeat-continuity test (`tasks.md` 4.11) is the catch-all for this.
8. **libp2p `request_response` is unordered.** The snapshot chunking protocol is serial-in-flight by construction. Do not concurrently fire chunks.
9. **Pin every git rev.** The proposal explicitly forbids "HEAD" pinning. If you find an unpinned dep, fix it before the validation gate.
10. **Mock LLM is real test infrastructure.** `MockLlmClient` is what runs in CI. Make it deterministic, scripted, and fast.
11. **Do not skip the QA review's failure modes.** The restart-durability test, the snapshot-skew protection, the WAL truncate task, the LAN-flooding mDNS workaround — all are tasks. Implement them.
12. **OpenAPI is the API contract.** When a route changes shape, the OpenAPI snapshot test fails. Update the snapshot deliberately; never via blind `--update`.

## Build + test loop

```bash
# every cycle
cd /c/experiments
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
openspec validate add-proof-experiment-harness --strict
openspec validate add-goblin-agentic-framework --strict

# at end of change #1
omega-experiment prove --commit var/synthetic-bundle.json --leaves var/leaves.json --out var/proof.bin
omega-experiment submit --node http://127.0.0.1:8001 --proof var/proof.bin
omega-experiment state --node http://127.0.0.1:8001

# at end of change #2
ollama pull gemma4:e4b   # one-time prerequisite, off the critical path
omega-goblins run --holders 5 --adversaries 2 --lurkers 1 --duration 30s --llm mock
```

## Acceptance gates

For change #1:
- All 12 task groups in `add-proof-experiment-harness/tasks.md` have every checkbox ticked.
- `omega-experiment bench --leaves 256 --samples 1000 --json` runs and emits real percentile data; spec.md performance scenarios are updated with measured numbers.
- The QA review's `## Recommendations` section has every numbered item addressed (or explicitly deferred with a one-paragraph rationale appended to the QA review).

For change #2:
- All 7 task groups in `add-goblin-agentic-framework/tasks.md` have every checkbox ticked.
- `omega-goblins run --holders 5 --adversaries 2 --lurkers 1 --duration 30s --llm mock` exits with code 0 and the metrics endpoint reports non-zero `goblin_ticks_total{role}` for each role.
- The OpenAPI document at `GET /v1/openapi.json` parses cleanly with a standard OpenAPI 3.1 validator.

When both gates pass:
- Run `openspec archive add-proof-experiment-harness` and `openspec archive add-goblin-agentic-framework` to move the changes into `openspec/changes/archive/<date>-<id>/` and merge their specs into `openspec/specs/`.
- Append a `cardano-wiki/wiki/log.md` entry summarising the shipped surface, the measured perf numbers, and any open follow-ups.
- Tag the workspace `v0.11.0` (across all crates).

## Things you may safely defer (do NOT silently skip)

If you hit any of these, write a short rationale into the relevant change's `tasks.md` (or a new `DEFERRED.md`) and proceed:

- C6 (PQ signature SLH-DSA inside the AIR). v0.1 mocks with Ed25519. Real SLH-DSA gadget is v0.2.
- Variable-length leaf preimages. v0.1 restricts to ≤ 64-byte leaves with explicit error path.
- Real `--tap-cardano preview` runs. Ship the code; CI runs in offline mode only.
- llama-cpp-rs in-process LLM. v0.1 ships only Ollama HTTP + Mock.
- Multi-machine goblin runners.

## Things you may NOT defer

- The actor pattern for the SQLite writer. Per-call `spawn_blocking` will not pass the heartbeat-continuity test under load.
- Snapshot chunking with the explicit wire protocol. The default `request_response` flow will OOM under InstallSnapshot.
- The restart-durability test. SQLite persistence with no restart test is an empty promise.
- The OpenAPI auto-generation. Hand-written API docs drift; they're not allowed here.
- Pinning Plonky3 / openraft / libp2p / rusqlite / pallas-validate to concrete versions or 40-char revs. "HEAD" or "1.x" is rejected at PR review.

## Where to ask for help

- `audit/wiki/00-synthesis.md` — cross-cutting risks the prior audit found.
- `cardano-wiki/wiki/pages/omega-testnet-e2e-plan.md` — the e2e narrative this code makes real.
- `cardano-wiki/docs/superpowers/specs/2026-05-03-blake3-migration-design.md` — context on why we use Blake3 and v2 domain tags.
- `cardano-wiki/wiki/log.md` — what shipped recently, dated.
- The QA review at `openspec/changes/add-proof-experiment-harness/QA-REVIEW.md` — challenges to keep in mind during implementation.

## Final guardrails

- This is a prototype harness. It is not a chain. It is not a node. It is not a wallet. It is not for production. Every doc says so; keep saying it.
- The Goblins are simulation tools. They are not part of the protocol. Don't let scope creep into "actually a real consensus participant."
- The `omega-api` is the *contract*. Once it's stable in v0.1, breaking it requires a `/v2/` and a deprecation window. No exceptions.
- The audit closure trail (`audit/findings/A1-A10`, `audit/SUMMARY.md`, `audit/RESOLUTION.md`) is the record of what was found and fixed in v0.9 → v0.10. Do not regress those findings.

Ship cleanly.
