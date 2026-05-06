# Raftlet FizzBee counterexamples

Accepted counterexample traces from running the safety model. Each trace classifies per `raftlet.md` lines 350–360:

- **Model bug** — the FizzBee model does not match the paper.
- **Paper assumption gap** — the paper relies on synchrony, dissemination, or rotation assumptions that need to become explicit crate configuration.
- **Implementation requirement** — the Rust core (`raftlet-core`) must reject or persist additional evidence to preserve the modelled invariant.

Initially empty. Populated as the model produces traces during plan execution (Task 19 onward).

## Format

Each trace is a directory `<date>-<short-name>/` containing:

- `trace.json` — the FizzBee trace file
- `summary.md` — one-paragraph human summary
- `classification.md` — model bug / paper gap / implementation requirement, plus the corresponding `raftlet-core` requirement if the last category applies
