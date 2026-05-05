#!/usr/bin/env bash
# run-fuzz.sh — runs cargo-fuzz with a finite time budget for orchestrator-friendly output.
# Usage: bash run-fuzz.sh <target_name> <duration_seconds>
# Run from the crate root.

set -euo pipefail

TARGET_NAME="${1:-}"
DURATION="${2:-600}"

if [[ -z "$TARGET_NAME" ]]; then
    echo "Usage: $0 <target_name> <duration_seconds>" >&2
    exit 1
fi

# -max_total_time is libFuzzer's wall-clock cap.
# -print_final_stats prints execs/sec and total execs at the end.
cargo +nightly fuzz run "$TARGET_NAME" -- \
    -max_total_time="$DURATION" \
    -print_final_stats=1
