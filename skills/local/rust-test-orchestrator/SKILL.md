---
name: rust-test-orchestrator
description: Orchestrates Rust test authoring for c:\experiments. Use when the user asks to test, write tests for, add coverage for, or improve testing of Rust code in this workspace. Classifies the target via a 6-question capability matrix, posts a test plan for sign-off, then invokes the right per-framework skill (proptest, cargo-fuzz, kani, shuttle-loom, failpoints, turmoil, madsim, or stateright). Triggers on: "write tests", "test this", "add coverage", "fuzz", "property test", "soundness test", "concurrency test", "test the cluster", "test the verifier".
license: Apache-2.0
metadata:
  author: charles
  version: 0.1.0
  pack: rust-test
  role: orchestrator
---

# rust-test-orchestrator

Workflow skill that classifies Rust code under test, gets user sign-off on a test plan, then dispatches to per-framework skills and reports results. Hard gate on the test plan; the rest flows automatically.

## When this skill applies

Use whenever the user asks to test, fuzz, property-check, model-check, or add coverage for Rust code in `c:\experiments\`. Skip when the user is asking about CI configuration, coverage tooling, criterion benchmarking, or anything that isn't authoring tests against application code.

## The five-phase workflow

### Phase 1 — Ground

Before reasoning about classification, inspect the actual code:

1. Read the target file(s) the user pointed at (or asked you to test).
2. Read the nearest `Cargo.toml` (workspace root + the crate's own `Cargo.toml`).
3. Grep the target file(s) for: `tokio::`, `Arc<Mutex`, `Arc<RwLock`, `unsafe`, `pub fn` taking `&[u8]` or `Vec<u8>`, `#[derive(Arbitrary)]`, `proptest!`, `kani::`, `loom::`, `shuttle::`.
4. Grep `Cargo.toml` for: `tokio`, `openraft`, `libp2p`, `p3-`, `pallas`, `blake3`, `proptest`, `kani-verifier`, `loom`, `shuttle`, `turmoil`, `madsim`, `cargo-fuzz`, `fail`.

Output: a "ground state" structure (just notes; no formal schema). Do not re-read files in later phases.

### Phase 2 — Classify

Answer 6 yes/no questions about the target. Each "yes" routes to a framework; each "no" gets a one-line skip rationale. See `references/capability-matrix.md` for the canonical question list and routing table.

Emit a matrix block in chat (see `references/decision-examples.md` for worked examples).

### Phase 3 — User sign-off (HARD GATE)

Post the matrix as a test plan. Wait for one of:
- "approve" / "looks good" / "yes" → proceed to Phase 4
- An edit (drop a framework, add a soundness-negative case, change a Kani bound) → revise the plan and re-post
- "reject" / "stop" → abandon cleanly

Do not write any test code until approval lands. Do not skip this gate.

### Phase 4 — Invoke per-framework skills

For each `→ invoke X` row in the approved matrix, call the per-framework skill via the `Skill` tool, in this order: `kani` → `proptest` → `cargo-fuzz` → `shuttle-loom` → `failpoints` → `turmoil` → `madsim` → `stateright`. (Cheapest-to-write first, heaviest-runtime last.)

Each invoked skill writes its tests and returns one line: `<framework>: wrote N tests in <path>; <pass|fail>`. Do not write test code yourself in this phase.

### Phase 5 — Run + report

For each invoked skill, run the appropriate command:

- proptest, kani: `cargo nextest run -p <crate> <test_name>`
- cargo-fuzz: `bash skills/local/rust-test-cargo-fuzz/scripts/run-fuzz.sh <target> <duration>`
- kani: `bash skills/local/rust-test-kani/scripts/kani-bound.sh <crate>`
- shuttle-loom (Shuttle): `cargo test -p <crate> --features shuttle`
- shuttle-loom (Loom): `LOOM_MAX_PREEMPTIONS=3 cargo test -p <crate> --release`
- failpoints: `cargo test -p <crate> --features failpoints`
- turmoil: `cargo test -p <crate> --features turmoil`
- madsim: `cargo +nightly test -p <crate> --features madsim`
- stateright: `cargo run -p <crate>-model --release`

Capture output, then emit a structured report following `references/report-format.md`. Compute the report STATUS:

- `GREEN` — all tests pass; soundness-negatives correctly reject
- `AMBER` — non-soundness tests failed; user judgment call
- `P0_REGRESSION` — soundness-negative test wrongly accepted, OR a previously-green test now red
- `PLAN_DRIFT` — planned soundness-negative case missing from report

Append the report to `c:/experiments/var/test-runs/<UTC-timestamp>.md` and post a one-line summary to chat.

## The Adversary invariant (CRITICAL)

If a soundness-negative test (Expected: reject) was Observed: accept, the report STATUS is `P0_REGRESSION`. The orchestrator MUST refuse to print "ready to commit" and MUST surface the offending input bytes. Silent green is worse than loud red. This mirrors the Goblin Adversary contract documented in `c:/experiments/README.md`.

## Storage policy

Reports go to `c:/experiments/var/test-runs/<UTC-timestamp>.md`. The orchestrator does not auto-commit reports to git. If `var/` is not in `.gitignore`, print a warning on first run.

## Anti-patterns

- Skipping the Phase 3 gate "because the plan looks obvious"
- Writing test code yourself in Phase 4 instead of invoking the per-framework skill
- Reporting `GREEN` when the Adversary table has any `Expected: reject / Observed: accept` row
- Re-reading the target file in Phase 2 or later (Phase 1 is the only inspection step)
- Auto-committing the report file to git
