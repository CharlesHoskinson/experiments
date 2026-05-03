#!/usr/bin/env bash
#
# Dump the Conway-era LedgerState as JSON from the headless mainnet node.
#
# Per the 2026-05-03 v1.0 ingestion revision, this file is the canonical
# input for the STAKE and GOVERNANCE sub-trees only. The UTXO sub-tree
# (and the derived token-policy + script sub-trees) are produced by
# `omega-utxo-snapshot` instead — see `crates/omega-utxo-snapshot/`.
# `cardano-cli query ledger-state` strips `utxoState.utxo` to `{}` on
# mainnet, and the cli's `--output-cbor` flag is not supported in 10.16
# (only --output-json | --output-text | --output-yaml).
#
# Usage:
#   ./scripts/dump_ledger_state.sh [--allow-incomplete] [output-path]
#
# Defaults:
#   CARDANO_HOME=$HOME/cardano
#   CARDANO_CLI=$CARDANO_HOME/bin/cardano-cli
#   CARDANO_NODE_SOCKET_PATH=$CARDANO_HOME/socket/node.socket
#   output-path=$CARDANO_HOME/snapshots/ledger_state_YYYYmmdd_HHMMSS.json
#
# Human-invoked only. This requires a fully synced node and writes a
# multi-GB JSON file (~2 GB on mainnet at epoch 628).

set -euo pipefail

ALLOW_INCOMPLETE=0
OUT_PATH=""

usage() {
  sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --allow-incomplete)
      ALLOW_INCOMPLETE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    -*)
      echo "ERROR: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [[ -n "$OUT_PATH" ]]; then
        echo "ERROR: multiple output paths supplied." >&2
        usage >&2
        exit 2
      fi
      OUT_PATH="$1"
      shift
      ;;
  esac
done

CARDANO_HOME="${CARDANO_HOME:-$HOME/cardano}"
CARDANO_CLI="${CARDANO_CLI:-$CARDANO_HOME/bin/cardano-cli}"
CARDANO_NODE_SOCKET_PATH="${CARDANO_NODE_SOCKET_PATH:-$CARDANO_HOME/socket/node.socket}"
export CARDANO_NODE_SOCKET_PATH

if [[ -z "$OUT_PATH" ]]; then
  OUT_PATH="$CARDANO_HOME/snapshots/ledger_state_$(date +%Y%m%d_%H%M%S).json"
fi

if [[ ! -x "$CARDANO_CLI" ]]; then
  echo "ERROR: cardano-cli not executable at $CARDANO_CLI" >&2
  echo "       Set CARDANO_CLI or see scripts/setup_headless_node.md." >&2
  exit 1
fi

if [[ ! -S "$CARDANO_NODE_SOCKET_PATH" ]]; then
  echo "ERROR: node socket not found at $CARDANO_NODE_SOCKET_PATH" >&2
  echo "       Start ~/cardano/start_node.sh after Mithril restore completes." >&2
  exit 1
fi

echo "== Checking node tip =="
TIP_JSON="$("$CARDANO_CLI" query tip --mainnet)"
SLOT="$(printf '%s' "$TIP_JSON" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("slot", "?"))')"
EPOCH="$(printf '%s' "$TIP_JSON" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("epoch", "?"))')"
SYNC="$(printf '%s' "$TIP_JSON" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("syncProgress", "?"))')"
SYNC_OK="$(printf '%s' "$TIP_JSON" | python3 -c 'import json,sys
v = json.load(sys.stdin).get("syncProgress", "0")
try:
    ok = float(str(v).rstrip("%")) >= 99.99
except ValueError:
    ok = False
print("1" if ok else "0")
')"

echo "Tip: slot=$SLOT epoch=$EPOCH syncProgress=$SYNC"

if [[ "$SYNC_OK" != "1" && "$ALLOW_INCOMPLETE" != "1" ]]; then
  echo "ERROR: node is not fully synced. Re-run with --allow-incomplete only for debugging." >&2
  exit 1
fi

mkdir -p "$(dirname "$OUT_PATH")"

echo
echo "== Dumping LedgerState JSON (stake + governance source) =="
echo "cardano-cli: $CARDANO_CLI"
echo "socket:      $CARDANO_NODE_SOCKET_PATH"
echo "output:      $OUT_PATH"
echo "note:        UTXO map is intentionally empty in cli output;"
echo "             use omega-utxo-snapshot for the UTXO sub-tree."
echo

"$CARDANO_CLI" conway query ledger-state \
  --mainnet \
  --out-file "$OUT_PATH"

echo
echo "== Dump complete =="
echo "Path:  $OUT_PATH"
echo "Size:  $(du -h "$OUT_PATH" | cut -f1)"
echo "Slot:  $SLOT"
echo "Epoch: $EPOCH"
echo
echo "Next: capture the UTXO sub-tree separately:"
echo "  cargo run -p omega-utxo-snapshot --release -- \\"
echo "    --socket $CARDANO_NODE_SOCKET_PATH \\"
echo "    --network mainnet --era 6 \\"
echo "    --out $CARDANO_HOME/snapshots/utxo_\$(date +%Y%m%d_%H%M%S).cbor"
