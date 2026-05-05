---
name: rust-test-madsim
description: Deterministic distributed-system simulation via MadSim. Heavier and more invasive than Turmoil but covers code that does not use Tokio directly. Use when invoked by rust-test-orchestrator (Q1=yes, non-Tokio) or when the user asks to test code that uses madsim::* shims. Not for tokio-native code (use turmoil) or single-process actors (use shuttle-loom).
license: Apache-2.0
metadata:
  author: charles
  version: 0.1.0
  pack: rust-test
  invoked-by: rust-test-orchestrator
---

# rust-test-madsim

## When this skill applies

Q1 (network/IO) on the orchestrator's matrix, when the target is NOT tokio-native and Turmoil is therefore not viable. MadSim provides deterministic shims for runtime, network, fs, time, and rng.

In `c:\experiments\` today, almost everything is Tokio-shaped, so Turmoil is preferred. MadSim becomes useful if:
- A future crate is built on a non-Tokio executor (e.g., async-std, smol)
- A future crate needs full deterministic FS shims (Turmoil's FS coverage is thinner)
- Cross-runtime interop tests are required

## Authoring loop

1. Replace `tokio::*` imports with `madsim::*` (or use `madsim::tokio` re-exports).
2. Wrap test with `#[madsim::test]`.
3. Drive the simulation: spawn nodes via `madsim::Handle::current().create_host(...)`.
4. Run: `cargo +nightly test -p <crate> --features madsim <test_name>`. (MadSim sometimes requires nightly for proc-macro tweaks.)
5. Determinism check: pin seed via `MADSIM_TEST_SEED=42`; re-run; assert byte-identical output.
6. Hand back to orchestrator: `madsim: wrote N tests in <path>; <pass|fail>`.

## Idioms in this codebase

None today. MadSim is reserved for future non-Tokio crates. See `references/runtime-shims.md` for the shim list and migration patterns.

## Anti-patterns

- Using MadSim for tokio-native code → defer to `rust-test-turmoil`. Turmoil is lighter, more idiomatic, smaller blast radius.
- Mixing `tokio::*` and `madsim::*` types in the same crate — non-deterministic in practice.
- Forgetting `MADSIM_TEST_SEED` when reproducing a failure — different seeds give different outputs.

## Hand-back format

Single line for the orchestrator: `madsim: wrote N tests in <path>; <pass|fail>`.
