#!/usr/bin/env bash
# kani-bound.sh — runs cargo kani with omega-commitment pinned bounds.
# Usage: bash kani-bound.sh <crate_name>

set -euo pipefail

CRATE="${1:-}"
if [[ -z "$CRATE" ]]; then
    echo "Usage: $0 <crate_name>" >&2
    exit 1
fi

# Pinned bounds:
#   --default-unwind 4        — small loops (7 sub-trees, 8 constraints)
#   --solver minisat          — stable baseline; cadical is faster but flakier
#   --output-format regular   — `regular` (cargo-kani >= 0.50) replaces the
#                                deprecated `old` format; orchestrator parses
#                                the "VERIFICATION:- SUCCESSFUL/FAILED" line.
cargo kani \
    -p "$CRATE" \
    --default-unwind 4 \
    --solver minisat \
    --output-format regular
