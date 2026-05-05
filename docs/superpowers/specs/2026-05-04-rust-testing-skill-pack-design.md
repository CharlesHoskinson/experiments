# Rust testing skill pack — design

**Date:** 2026-05-04
**Author:** Charles
**Status:** Draft pending user review
**Scope:** A 9-skill pack for Claude Code that orchestrates Rust testing across proptest, cargo-fuzz, Kani, Loom/Shuttle, failpoints, Turmoil, MadSim, and Stateright. Targets the `c:\experiments\` workspace (omega-commitment today; LoganNet + Goblins when they land) but the per-framework skills are codebase-agnostic.

## Why

`c:\experiments\omega-commitment\` is 9 crates, 282+ tests, soundness-bearing Plonky3 STARK code, golden-vector discipline at three layers, and a roadmap that adds a 3-node openraft + libp2p + rusqlite cluster (LoganNet) plus an autonomous-agent framework (Goblins). The testing surface spans cryptographic primitives, parsers over untrusted bytes, lock-shaped concurrency in mpsc actors, distributed consensus over a partition-able network, and abstract protocol composition. No single test framework covers all of those well.

The Rust ecosystem has good answers per surface — proptest, cargo-fuzz, Kani, Loom, Shuttle, failpoints, Turmoil, MadSim, Stateright — but knowing *which* one applies to *which* code is the load-bearing decision, and getting it wrong burns hours (e.g., reaching for proptest when the bug only appears under a specific schedule that needs Loom). A skill pack lets Claude make that decision with the user's explicit sign-off, then drive the framework-specific authoring loop with the right idioms baked in.

The pack also adds an Adversary-class invariant at the report layer: a soundness-negative test that *passes* (the verifier wrongly accepted bad input) is treated as a P0 regression and the orchestrator refuses to proceed. This mirrors the omega-commitment Goblin Adversary contract at the meta level — silent green is worse than loud red.

## Non-goals

- Not a CI configuration tool. Pack does not edit `.github/workflows/`, `Makefile`, or `cargo nextest` config files.
- Not a coverage tool. Pack does not invoke `cargo llvm-cov` or report line coverage.
- Not a benchmarking tool. Criterion lives elsewhere.
- Not a replacement for `omega-rustdoc-style` or `plonky3-friendly-rust`. Those are doc-style and circuit-design skills; this pack is testing only.
- Not a one-skill-fits-all umbrella. Tried that in alternative A; rejected because it produces a sprawling SKILL.md that triggers too broadly.
- Not a portable open-source release at v0.1.0. The orchestrator embeds omega-commitment-specific patterns (LoganNet, Plonky3 circuits, openraft actors). Generalisation is a v1.0 concern after the pack proves itself.

## Architecture

### Pack layout

```
c:\experiments\skills\local\
├── omega-rustdoc-style/        (existing, untouched)
├── plonky3-friendly-rust/      (existing, untouched)
│
├── rust-test-orchestrator/     (NEW)
│   ├── SKILL.md
│   └── references/
│       ├── capability-matrix.md
│       ├── decision-examples.md
│       ├── report-format.md
│       ├── trigger-suite.md
│       ├── iteration-log.md
│       └── fixtures/
│           ├── F1.md
│           ├── F2.md
│           └── F3.md
│
├── rust-test-proptest/         (NEW, S2)
├── rust-test-cargo-fuzz/       (NEW, S3 — scripts included)
├── rust-test-kani/             (NEW, S3 — scripts included)
├── rust-test-shuttle-loom/     (NEW, S2 — Loom + Shuttle merged)
├── rust-test-failpoints/       (NEW, S2)
├── rust-test-turmoil/          (NEW, S2)
├── rust-test-madsim/           (NEW, S2)
└── rust-test-stateright/       (NEW, S2)
```

Nine new skill folders. Total disk footprint: ~30-50 KB markdown + ~200 lines bash across the two S3 skills. Existing `skills/local/` skills untouched.

### Naming and installation

- **Naming convention:** `rust-test-<framework>` for the per-framework skills, `rust-test-orchestrator` for the entry point. Generic enough to be portable to other Rust codebases later, prefixed enough to group visually.
- **Installation:** `c:\experiments\skills\local\` (project-scoped, ships with repo, loads only in this working directory). Matches the existing `omega-rustdoc-style` + `plonky3-friendly-rust` pattern.
- **Promotion path:** Once a per-framework skill passes T1+T2+T3 across two consecutive sessions and proves portable, promote to `~/.claude/skills/` for cross-project use. v0.1.0 ships everything project-local.

### Bundling level per skill

| Skill | Bundling | Rationale |
|---|---|---|
| rust-test-orchestrator | S2 (no scripts) | Logic is decision-tree + report-emitter; no shell wrappers needed |
| rust-test-proptest | S2 | Cookbook patterns sufficient; no setup overhead |
| rust-test-cargo-fuzz | S3 | Target scaffolding + corpus paths + nightly flags pay back script investment |
| rust-test-kani | S3 | Bound configuration must be project-pinned; script enforces this |
| rust-test-shuttle-loom | S2 | Per-test config; no shared script |
| rust-test-failpoints | S2 | Cookbook patterns sufficient |
| rust-test-turmoil | S2 | Cookbook patterns sufficient |
| rust-test-madsim | S2 | Cookbook patterns sufficient |
| rust-test-stateright | S2 | Cookbook patterns sufficient |

## Orchestrator workflow

The orchestrator runs five phases with one hard gate (G2) between phases 2 and 3.

### Phase 1 — Ground (light A3 inspection)

The orchestrator runs concrete code inspection before reasoning about classification:

- `Read` the target file(s) under test.
- `Read` the nearest `Cargo.toml` (workspace root + crate-local).
- `Grep` the target file(s) for: `tokio::`, `Arc<Mutex`, `Arc<RwLock`, `unsafe`, `pub fn` taking `&[u8]`/`Vec<u8>`, `#[derive(Arbitrary)]`, `proptest!`, `kani::`, `loom::`, `shuttle::`.
- `Grep` Cargo.toml for: `tokio`, `openraft`, `libp2p`, `p3-`, `pallas`, `blake3`, `proptest`, `kani-verifier`, `loom`, `shuttle`, `turmoil`, `madsim`, `cargo-fuzz`, `fail`.

Output: a small "ground state" structure passed to phase 2. This is the only inspection step; subsequent phases reason about the ground state without re-reading files.

### Phase 2 — Classify (A2 capability matrix)

For each unit of code under test, the orchestrator answers six yes/no questions and emits a matrix:

| Q | Question | Routes to |
|---|---|---|
| Q1 | Touches network sockets, filesystem, or async I/O? | turmoil / madsim / failpoints |
| Q2 | Shares mutable state across threads (Arc<Mutex>, channels, atomics)? | shuttle-loom |
| Q3 | Parses, decodes, or processes untrusted bytes? | cargo-fuzz |
| Q4 | Has a small, bounded input space and a clear safety invariant? | kani |
| Q5 | Is a property holding over many random inputs (codecs, round-trips, monotonic state)? | proptest |
| Q6 | Is a distributed protocol with N parties whose interleavings matter? | stateright |

Each "yes" routes to a framework. Each "no" is recorded with a one-line skip rationale. The matrix is the *artifact* produced at this phase, not just an internal intermediate.

Worked example (target = `omega-mock-ledger::apply_transaction`):

```
target: omega-mock-ledger::apply_transaction
ground: tokio=no, async=no, unsafe=no, openraft=no, pallas=yes, blake3=yes
matrix:
  Q1 network/IO?           → no    → skip turmoil, madsim, failpoints
  Q2 shared mut state?     → no    → skip shuttle-loom
  Q3 untrusted bytes?      → yes   → invoke rust-test-cargo-fuzz
  Q4 bounded invariant?    → yes   → invoke rust-test-kani  (bound: tx size ≤ 4KB)
  Q5 random-input prop?    → yes   → invoke rust-test-proptest
  Q6 protocol w/ N parties? → no    → skip stateright
soundness-negative cases planned: 3
  - malformed CBOR header
  - oversized asset bundle
  - duplicate nullifier
```

### Phase 3 — User sign-off (G2 hard gate)

Orchestrator presents the matrix as a test plan in chat and waits. User can:

- **Approve** — proceed to phase 4 with the matrix as posted.
- **Edit** — drop a framework, add a soundness-negative case, change a Kani bound, etc. Orchestrator re-emits the revised plan and waits again.
- **Reject** — abandon. Orchestrator stops cleanly.

No code is written until approval lands. This is the only hard gate in the workflow; phases 1, 2, 4, 5 flow automatically.

### Phase 4 — Invoke per-framework skills

For each `→ invoke X` row in the approved matrix, the orchestrator calls the corresponding skill via the `Skill` tool, in this order: kani → proptest → cargo-fuzz → shuttle-loom → failpoints → turmoil → madsim → stateright. (Cheapest-to-write first, heaviest-runtime last.) Each invoked skill writes its tests; the orchestrator does not write test code itself.

Each per-framework skill returns a single hand-back line: `<framework>: wrote N tests in <path>; <pass|fail>`.

### Phase 5 — Run + report

For each invoked skill the orchestrator runs the appropriate command:

- proptest, kani: `cargo nextest run -p <crate> <test_name>`
- cargo-fuzz: `bash scripts/run-fuzz.sh <target> <duration>` (from `rust-test-cargo-fuzz/scripts/`)
- kani: `bash scripts/kani-bound.sh <crate>` (from `rust-test-kani/scripts/`)
- shuttle-loom: `LOOM_MAX_PREEMPTIONS=3 cargo test -p <crate> --release` or `cargo test -p <crate> --features shuttle`
- failpoints: `cargo test -p <crate> --features failpoints`
- turmoil: `cargo test -p <crate> --features turmoil`
- madsim: `cargo +nightly test -p <crate> --features madsim`
- stateright: `cargo run -p <crate>-model --release`

Captures output, then emits a structured report (next section). The Adversary invariant runs here.

## Per-framework skill structure

### Common SKILL.md template (~600-1200 words each)

```markdown
---
name: rust-test-<framework>
description: <what it does> + <when orchestrator routes here> + <key trigger phrases>
license: Apache-2.0
metadata:
  author: charles
  version: 0.1.0
  pack: rust-test
  invoked-by: rust-test-orchestrator
---

# rust-test-<framework>

## When this skill applies
<the matrix question(s) this answers + concrete code shapes>

## Authoring loop (TDD-shaped)
1. Read target code (already done by orchestrator)
2. Identify property / invariant / failure mode
3. Write the test scaffold (idiomatic for this framework)
4. Run + observe failure
5. Implement / patch until green (orchestrator does the running)
6. Hand back to orchestrator with a one-line summary

## Idioms in this codebase
<2-4 patterns specific to omega-commitment / LoganNet>

## Anti-patterns
<things this framework is bad at, and which sibling skill to invoke instead>

## Hand-back format
A single line for the orchestrator: `<framework>: wrote N tests in <path>; <pass|fail>`
```

### Per-skill specifics

| Skill | Key idiom for omega-commitment | Anti-pattern flagged |
|---|---|---|
| rust-test-proptest | Custom `Strategy` for sub-tree leaf encodings; round-trip CBOR ↔ struct ↔ leaf-hash | Code that needs *exhaustive* schedule exploration → defer to shuttle-loom |
| rust-test-cargo-fuzz | Fuzz target per parser (LSQ CBOR, ledger-state JSON, NDJSON header rows); seed corpus from real golden vectors | Pure functions with bounded inputs → defer to kani |
| rust-test-kani | Bound on payload size ≤ 4 KB; verify `leaf_hash_v2` collision-resistance over `(sub_tree_id, idx, payload)` triples; verify nullifier-set inserts are monotonic | Code calling Plonky3 (state explodes) → defer to proptest |
| rust-test-shuttle-loom | Shuttle for the mpsc-actor that serialises rusqlite writes against openraft's apply path; Loom for any lock-free primitive added to the same crate | Multi-node Raft cluster → defer to turmoil/stateright |
| rust-test-failpoints | Plant `fail::fail_point!` at: WAL fsync sites, libp2p send sites, snapshot read paths, mempool decrypt path | Replacing actual integration tests → failpoints inject *additional* coverage, not replacement |
| rust-test-turmoil | 3-node LoganNet topology fixture; partition node 1 from {2,3} mid-leader-election; force snapshot-mid-leader-change | Single-process unit tests → no value |
| rust-test-madsim | Use only when target uses `madsim::*` shims (not `tokio::*` directly); deterministic seed pinning across runs | Already-tokio-shaped code → defer to turmoil |
| rust-test-stateright | Abstract Crypsinous + Chronos + Minotaur composition as a 4-actor model: leader, follower, attestor, mempool; check safety (no two leaders per slot) and liveness (every valid tx eventually applies) | Concrete byte-level behavior → defer to turmoil |

### S3 scripts (cargo-fuzz and kani only)

`rust-test-cargo-fuzz/scripts/init-fuzz-target.sh <target_name>` — scaffolds `fuzz/fuzz_targets/<name>.rs` with the omega-commitment seed corpus path under `tests/golden_vectors/`.

`rust-test-cargo-fuzz/scripts/run-fuzz.sh <target_name> <duration_seconds>` — wraps `cargo +nightly fuzz run <name> -- -max_total_time=<duration>`.

`rust-test-kani/scripts/kani-bound.sh <crate>` — runs `cargo kani --default-unwind 4 --solver minisat -p <crate>` with omega's pinned bounds. Bounds documented in `rust-test-kani/references/bound-tuning.md`.

## Report format and Adversary invariant

### Report schema

Markdown, posted to chat and appended to `c:\experiments\var\test-runs\<UTC-timestamp>.md`:

```markdown
# Test run report — <target> @ <commit-sha>
**Plan approved at:** <timestamp>
**Frameworks invoked:** kani, proptest, cargo-fuzz
**Frameworks skipped:** shuttle-loom, failpoints, turmoil, madsim, stateright
**Skip rationale per matrix:** Q1=no, Q2=no, Q6=no

## Results
| Skill | Tests written | Passed | Failed | Time | Notes |
|---|---|---|---|---|---|
| kani         | 3 harnesses  | 3/3 | 0/3 | 47s   | bounds: payload≤4KB, idx<2^20 |
| proptest     | 8 cases      | 8/8 | 0/8 | 12s   | 4096 inputs/case |
| cargo-fuzz   | 1 target     | n/a | n/a | 600s  | 0 crashes, 14M execs, corpus +312 |

## Soundness-negative tests (Adversary class)
| Case | Expected | Observed | Status |
|---|---|---|---|
| malformed CBOR header                | reject | reject | ✅ |
| oversized asset bundle (>64KB)       | reject | reject | ✅ |
| duplicate nullifier replay           | reject | reject | ✅ |

## P0 alerts
None.

## Coverage delta vs prior run
+ 3 kani harnesses, + 8 proptest cases, + 312 fuzz corpus entries
- 0 regressions on prior tests (rerun: 282/282 green)

STATUS: GREEN
```

### Adversary invariant

Every skill that writes a soundness-negative test (a test asserting that bad input is *rejected*) registers it in the report's "Soundness-negative" table. The orchestrator then enforces:

1. If `Expected: reject` and `Observed: accept` → report's "P0 alerts" section is populated with the offending input bytes, run is marked `STATUS: P0_REGRESSION`, orchestrator refuses to proceed to "ready to commit". Mirrors the Goblin Adversary contract: silent green is worse than loud red.
2. If a soundness-negative test is missing from the planned matrix when the matrix said it should exist (e.g., plan said "3 cases" but report shows 2), orchestrator flags `STATUS: PLAN_DRIFT` and surfaces which case was dropped.
3. If the prior-run regression check shows previously-green tests now red, that's also P0, treated identically to (1).

### Terminal states

| Status | Meaning | Orchestrator behavior |
|---|---|---|
| `GREEN` | All tests pass; soundness-negatives correctly reject | Print "ready to commit" with summary |
| `AMBER` | Some non-soundness tests failed | Print failures, suggest next steps; user judgment call |
| `P0_REGRESSION` | Soundness-negative test wrongly accepted, OR prior green test now red | Refuse to proceed; print offending bytes/test name; demand investigation |
| `PLAN_DRIFT` | Planned soundness-negative case missing from report | Refuse to proceed; demand the missing case be added before re-run |

### Storage policy

Reports go to `c:\experiments\var\test-runs\<UTC-timestamp>.md` (matches existing `var/` convention). The orchestrator does not auto-commit reports to git; it leaves a one-line pointer in chat. If `var/` is not in `.gitignore`, the orchestrator's first run prints a warning to add it.

## Testing the skill pack itself

### T1 — Triggering tests (per skill)

Fixture file `rust-test-orchestrator/references/trigger-suite.md` lists 25 prompts that should trigger the orchestrator and 10 prompts that should NOT. Triggering verified manually: open a fresh Claude Code session, paste each prompt, observe whether the orchestrator loads.

Sample positive triggers: "write tests for `apply_transaction`", "I need fuzz coverage on the LSQ decoder", "how do I test the LoganNet Raft cluster", "add property tests for leaf encoding", "test soundness of the verifier", "what tests should this module have".

Sample negative triggers: "configure CI", "fix this clippy warning", "what does cargo nextest do", "set up coverage reporting", "rename a function", "explain this code".

### T2 — Functional tests (golden harness)

Three end-to-end fixtures, each a real omega-commitment file with a known-correct test plan stored alongside:

| Fixture | Target | Expected matrix | Expected skills invoked |
|---|---|---|---|
| F1 | `omega-commitment-core::leaf_hash_v2` | Q3=yes, Q4=yes, Q5=yes; rest=no | proptest, cargo-fuzz, kani |
| F2 | `omega-mock-ledger::WriterActor` | Q1=yes (fs), Q2=yes; rest=no | shuttle-loom, failpoints |
| F3 | `omega-toy-consensus::raft_node` (planned) | Q1=yes, Q6=yes; rest=no | turmoil, stateright |

For each fixture, the test runs the orchestrator end-to-end and asserts (a) the matrix matches the expected, (b) the right skills were invoked, (c) the report's STATUS is `GREEN`. Stored at `rust-test-orchestrator/references/fixtures/F{1,2,3}.md` with the expected output pinned as a golden vector.

F1 must pass before pack ships. F2 ships when `omega-mock-ledger::WriterActor` lands. F3 ships when LoganNet (`omega-toy-consensus`) lands.

### T3 — Adversary self-test

A deliberately broken fixture (F4) that includes a soundness-negative test designed to wrongly succeed — e.g., a verifier with a hard-coded `true` short-circuit. The pack's pass condition for T3 is that the report comes back with `STATUS: P0_REGRESSION`. If the orchestrator reports `GREEN` on F4, the pack itself is broken and must not ship. This is the verify-the-verifier check, mirroring the Goblin Adversary contract at the meta level.

### T4 — Performance comparison

Per the PDF's pattern, run the same task ("test `leaf_hash_v2` thoroughly") with and without the pack enabled, then compare:

- Tool calls before reaching a complete test plan (target: ≤8 with pack vs. ~30 without)
- Failed `cargo test` runs during the session (target: 0 with pack)
- Whether soundness-negative cases were proposed at all (without pack: usually no; with pack: always per matrix)

Run once at v0.1.0; re-run quarterly.

### T5 — Iteration loop

Each test run that fails T1/T2/T3 produces a one-line entry in `rust-test-orchestrator/references/iteration-log.md` with the failure mode and the SKILL.md change that fixed it. Matches the PDF's "iterate based on feedback" guidance. Promote stable skills to `~/.claude/skills/` only after they pass T1+T2+T3 cleanly across two consecutive sessions.

### Test gate for shipping

Before tagging v0.1.0 of the pack:

- T1: 25/25 positive triggers fire, 10/10 negative triggers do not
- T2-F1: orchestrator end-to-end matches golden output, STATUS=GREEN
- T3: orchestrator returns STATUS=P0_REGRESSION on broken fixture F4
- T4: baseline measurement captured (no threshold required at v0.1.0; comparison happens at v0.2.0)
- T5: iteration log exists, even if empty

T2-F2 and T2-F3 are deferred to LoganNet milestone.

## Open design questions

None as of 2026-05-04. All six clarifying questions resolved in brainstorming session: skill granularity (B), orchestrator shape (G2 workflow), framework lineup (B1, 9 skills), naming + install (N3 + I1), gate calibration + Adversary invariant (G2 + yes), bundling level (S2 + S3 for cargo-fuzz/kani), classification architecture (A2 + light A3 grounding).

If new questions surface during implementation, they go in this section with date + resolution.

## References

- Anthropic, *The Complete Guide to Building Skills for Claude* (read 2026-05-04). Particularly chapters 2 (planning), 3 (testing), 5 (patterns).
- `c:\experiments\README.md` — Goblin Adversary contract, golden-vector layers, omega-commitment scope.
- `c:\experiments\skills\local\plonky3-friendly-rust\` — pattern for project-local skill structure.
- `c:\experiments\skills\local\omega-rustdoc-style\` — same pattern, doc-style skill.
- Turmoil: https://docs.rs/turmoil/latest/turmoil/
- MadSim: https://github.com/madsim-rs/madsim
- Stateright: https://github.com/stateright/stateright
- Loom: https://github.com/tokio-rs/loom
- Shuttle: https://github.com/awslabs/shuttle
- cargo-fuzz: https://github.com/rust-fuzz/cargo-fuzz
- proptest: https://github.com/proptest-rs/proptest
- Kani: https://model-checking.github.io/kani/
- failpoints (fail-rs): https://github.com/tikv/fail-rs
