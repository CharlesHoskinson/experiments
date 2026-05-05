# Corpus management

The corpus is the set of inputs libFuzzer mutates from. A good seed corpus saves hours.

## Where omega-commitment seed corpus lives

```
c:/experiments/omega-commitment/tests/golden_vectors/
├── per_leaf/          # individual leaf encodings (sub-tree id × test case)
├── per_subtree/       # full sub-tree roots
└── bundle/            # bundle-root tuples
```

Use `per_leaf/` as the seed corpus for `omega_leaf_decode`. Use raw LSQ response captures (when available) for `lsq_cbor_decode`.

## Initialising the corpus

After running `init-fuzz-target.sh <name>`, copy seeds:

```bash
mkdir -p fuzz/corpus/<name>/
cp omega-commitment/tests/golden_vectors/per_leaf/utxo/* fuzz/corpus/<name>/
```

## Corpus minimisation

After a long fuzz run, minimise to the smallest set with the same code coverage:

```bash
cargo +nightly fuzz cmin <name>
```

Commit the minimised corpus to git so subsequent runs start fast.

## Crash triage

When libFuzzer finds a crash, the offending input is at `fuzz/artifacts/<name>/crash-<hash>`.

To reproduce:

```bash
cargo +nightly fuzz run <name> fuzz/artifacts/<name>/crash-<hash>
```

To convert into a regression test, hex-dump the input and add a `#[test]` to the crate's normal test suite that calls the function on the same bytes and asserts a clean `Err` (not a panic).

## OSS-Fuzz handoff (future)

When the pack matures, omega-commitment fuzz targets become candidates for OSS-Fuzz integration. The target signature already matches OSS-Fuzz's expectations. Out of scope for v0.1.0 of this skill pack.
