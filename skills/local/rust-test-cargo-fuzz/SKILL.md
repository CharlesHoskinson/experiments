---
name: rust-test-cargo-fuzz
description: libFuzzer-based fuzz testing for Rust via cargo-fuzz. Use when invoked by rust-test-orchestrator (Q3=yes) or when the user explicitly asks to fuzz a parser, decoder, or any function consuming untrusted bytes. Bundles scripts to scaffold targets and run with omega-commitment seed corpus paths. Not for bounded invariants (use kani) or codec round-trips on typed input (use proptest).
license: Apache-2.0
metadata:
  author: charles
  version: 0.1.0
  pack: rust-test
  invoked-by: rust-test-orchestrator
---

# rust-test-cargo-fuzz

## When this skill applies

Q3 (parses untrusted bytes) on the orchestrator's matrix. Concrete shapes:
- Wire-format decoders (CBOR, JSON, NDJSON, libp2p protocol messages)
- File parsers (snapshot files, ledger-state JSON, fuzz-target inputs)
- Anything taking `&[u8]` or `Vec<u8>` from outside the process

## Authoring loop

1. Identify the target function (must take `&[u8]` and not panic on any input).
2. Run `bash scripts/init-fuzz-target.sh <target_name>` to scaffold `fuzz/fuzz_targets/<name>.rs`.
3. Seed the corpus from existing golden vectors at `c:/experiments/omega-commitment/tests/golden_vectors/` (or wherever the crate's golden vectors live).
4. Run `bash scripts/run-fuzz.sh <target_name> <duration_seconds>`.
5. If a crash is found, the input is saved to `fuzz/artifacts/<target>/crash-<hash>`. Add as a regression test in the crate's normal test suite.
6. Hand back to orchestrator: `cargo-fuzz: ran <target> for <Ns>; <0 crashes | N crashes saved to fuzz/artifacts/>`.

## Idioms in this codebase

**One target per parser.** Don't bundle multiple decoders in one fuzz target — when libFuzzer finds a crash, you want to know which decoder broke. Targets to land first:
- `lsq_cbor_decode` — pallas-network LSQ response bytes
- `ledger_state_json_parse` — cardano-cli ledger-state JSON output
- `header_ndjson_row` — chain-follower NDJSON output
- `omega_leaf_decode` — omega-commitment-core leaf bytes

**Seed corpus from goldens.** The omega-commitment workspace already pins golden vectors at three layers. Use them as the initial corpus — libFuzzer will mutate from there. See `references/corpus-management.md`.

**Adversary class targets.** For verifier-shaped code (e.g., `verify_claim`), the fuzz target shape is "must never accept a malformed input". Wrap with: `if verify_claim(input).is_ok() && !is_well_formed(input) { panic!("ADVERSARY: accepted malformed claim"); }`. The Adversary panic is what the orchestrator's report flags as P0.

## Anti-patterns

- Fuzzing pure functions with bounded inputs → defer to `rust-test-kani` (model checking is faster than blind mutation when the space is small).
- Fuzzing without a seed corpus → wastes the first hour discovering basic structure.
- Fuzzing with `RUST_BACKTRACE=full` set globally — the per-input cost crushes throughput. Set only when triaging a found crash.
- Letting `cargo +nightly fuzz run` go indefinitely without `-max_total_time` — the orchestrator's report needs a finite duration to compute STATUS.

## Hand-back format

Single line for the orchestrator: `cargo-fuzz: ran <target> for <Ns>; <0 crashes | N crashes saved to fuzz/artifacts/>`.

## Scripts in this skill

- `scripts/init-fuzz-target.sh <name>` — scaffolds `fuzz/fuzz_targets/<name>.rs` with seed-corpus path
- `scripts/run-fuzz.sh <name> <duration>` — runs `cargo +nightly fuzz run <name> -- -max_total_time=<duration>`
