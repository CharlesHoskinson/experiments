# MadSim runtime shims

MadSim provides deterministic replacements for stdlib + tokio APIs that touch nondeterminism (time, network, fs, rng).

## Shim table

| Stdlib / Tokio | MadSim shim |
|---|---|
| `tokio::time::*` | `madsim::time::*` |
| `tokio::net::*` | `madsim::net::*` |
| `tokio::fs::*` | `madsim::fs::*` |
| `std::time::Instant` | `madsim::time::Instant` |
| `rand::thread_rng` | `madsim::rand::*` |

## Importing shims (cfg-gated)

```rust
#[cfg(madsim)]
use madsim::{net, time};
#[cfg(not(madsim))]
use tokio::{net, time};
```

The `madsim` cfg flag is set automatically by `cargo +nightly test --features madsim`.

## Determinism contract

A MadSim test is deterministic if and only if every nondeterministic API call goes through a MadSim shim. The most common bug is a `std::time::Instant::now()` lurking inside a dependency — that breaks determinism silently.

## Seed pinning

```bash
MADSIM_TEST_SEED=42 cargo +nightly test --features madsim
```

If a test passes with seed 42 and fails with seed 43, the test is finding a real bug — pin the failing seed in the test attribute:

```rust
#[madsim::test(seed = 43)]
async fn reproduces_partition_bug() { ... }
```

## When MadSim is overkill

For tokio-native code, Turmoil covers 90% of MadSim's value with 30% of the invasiveness. Default to Turmoil; reach for MadSim only for the gaps.
