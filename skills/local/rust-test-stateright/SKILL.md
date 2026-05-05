---
name: rust-test-stateright
description: Abstract protocol model checking via Stateright. Use when invoked by rust-test-orchestrator (Q6=yes) or when the user asks to model-check a consensus algorithm, leader election, or any distributed protocol at the abstract behavioral level (not the byte level). Best for safety / liveness / always-eventually properties on N-actor protocols. Not for concrete byte behaviour (use turmoil) or single-process concurrency (use shuttle-loom).
license: Apache-2.0
metadata:
  author: charles
  version: 0.1.0
  pack: rust-test
  invoked-by: rust-test-orchestrator
---

# rust-test-stateright

## When this skill applies

Q6 (distributed protocol with N parties) on the orchestrator's matrix, when the property is about the *protocol's* behaviour rather than its bytes. Concrete shapes:
- Consensus safety: "no two leaders ever elected for the same term"
- Consensus liveness: "every valid client request eventually applies"
- Threshold-encryption committee: "no decryption before quorum"
- Leader election: "exactly one leader emerges from any partition heal"

## Authoring loop

1. Define the actor types: each role in the protocol gets one actor.
2. Define the message types passed between actors.
3. Define the state per actor (just the abstract state needed for the property).
4. Define properties as `Property::always(...)` (safety) or `Property::eventually(...)` (liveness).
5. Run: `cargo run -p <crate>-model --release` — Stateright explores the state space.
6. If a counterexample is found, the trace is printed.
7. Hand back to orchestrator: `stateright: wrote N properties in <path>; <pass|fail>`.

## Idioms in this codebase

**4-actor Crypsinous + Chronos + Minotaur model.** See `references/actor-modeling.md` for the canonical 4-actor abstraction: leader, follower, attestor, mempool.

**Safety properties first, liveness second.** Safety violations are easier to debug (a single trace shows the violation); liveness violations require liveness-fairness assumptions.

**Bound the state space.** Stateright explores all reachable states; bound everything (max-rounds, max-clients, max-messages) explicitly.

## Anti-patterns

- Modelling concrete byte behaviour (CBOR encoding, hash function) → defer to `rust-test-turmoil` or `rust-test-proptest`.
- Modelling at too low a level (bit-level transitions) — state space explodes.
- Skipping liveness properties because they're harder — without them you can't catch deadlock.
- Not bounding the model — Stateright will run forever on an unbounded model.

## Hand-back format

Single line for the orchestrator: `stateright: wrote N properties in <path>; <pass|fail>`.
