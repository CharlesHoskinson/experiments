#!/usr/bin/env bash
# init-fuzz-target.sh — scaffolds a new cargo-fuzz target with omega-commitment seed corpus path.
# Usage: bash init-fuzz-target.sh <target_name>
# Run from the crate root that contains Cargo.toml.

set -euo pipefail

TARGET_NAME="${1:-}"
if [[ -z "$TARGET_NAME" ]]; then
    echo "Usage: $0 <target_name>" >&2
    exit 1
fi

# Initialise fuzz/ directory if missing.
if [[ ! -d fuzz ]]; then
    cargo +nightly fuzz init
fi

# Add the target.
cargo +nightly fuzz add "$TARGET_NAME"

# Seed corpus from omega-commitment golden vectors if available.
SEED_DIR="../../tests/golden_vectors/per_leaf/utxo"
CORPUS_DIR="fuzz/corpus/$TARGET_NAME"
if [[ -d "$SEED_DIR" ]]; then
    mkdir -p "$CORPUS_DIR"
    cp -n "$SEED_DIR"/* "$CORPUS_DIR"/ 2>/dev/null || true
    echo "Seeded $CORPUS_DIR from $SEED_DIR"
fi

echo "Fuzz target '$TARGET_NAME' scaffolded at fuzz/fuzz_targets/$TARGET_NAME.rs"
echo "Edit it to call the function under test, then run:"
echo "  bash $(dirname "$0")/run-fuzz.sh $TARGET_NAME 600"
