#!/usr/bin/env bash
#
# !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
# !! DEBUG ONLY — does NOT verify the Mithril certificate.                  !!
# !!                                                                        !!
# !! This helper fetches a snapshot tarball directly from the aggregator    !!
# !! over HTTPS and extracts it. It does NOT run mithril-client and it      !!
# !! does NOT cross-check the snapshot against the Mithril genesis or       !!
# !! ancillary verification keys. Use it ONLY for ingestion-experiment      !!
# !! smoke-tests where the source of truth is the in-tree hand-crafted     !!
# !! CBOR fixture.                                                          !!
# !!                                                                        !!
# !! For PRODUCTION snapshots (anything that ends up feeding omega-ingest   !!
# !! mainnet, omega-utxo-snapshot pinned-manifest acquisition, or any v1.0  !!
# !! reproducible-build artefact) follow Step 4 of                          !!
# !!     scripts/setup_headless_node.md                                     !!
# !! which uses `mithril-client cardano-db download --include-ancillary`   !!
# !! with GENESIS_VERIFICATION_KEY and ANCILLARY_VERIFICATION_KEY set       !!
# !! against the published mainnet keys. That path verifies the Mithril    !!
# !! certificate before unpacking; this script does not.                    !!
# !!                                                                        !!
# !! Audit reference: A10/F002 (2026-05-03 Codex audit).                    !!
# !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
#
# Download a recent Mithril-attested Cardano snapshot for manual QA.
#
# Usage:
#   ./scripts/download_snapshot.sh [aggregator-url]
#
# Default: pre-release-preview Mithril aggregator (testnet, smaller
# than mainnet — appropriate for ingestion experiments).
#
# Output:
#   var/snapshots/<digest>/  — extracted snapshot directory
#
# This script is HUMAN-INVOKED only. Tests do NOT call it; they use
# the in-tree hand-crafted CBOR fixture.

set -euo pipefail

cat >&2 <<'WARN'
================================================================================
download_snapshot.sh: DEBUG ONLY — Mithril certificate is NOT verified.
For production snapshots, see setup_headless_node.md Step 4 (mithril-client
with GENESIS_VERIFICATION_KEY + ANCILLARY_VERIFICATION_KEY).
================================================================================
WARN

AGGREGATOR_URL="${1:-https://aggregator.pre-release-preview.api.mithril.network/aggregator}"
DEST_DIR="$(cd "$(dirname "$0")/.." && pwd)/var/snapshots"

echo "Mithril aggregator: $AGGREGATOR_URL"
echo "Destination:        $DEST_DIR"
mkdir -p "$DEST_DIR"

echo
echo "== Fetching snapshot list =="
SNAPSHOT_JSON="$(curl -fsSL "$AGGREGATOR_URL/snapshots")"

# Pick the most recent snapshot.
DIGEST="$(echo "$SNAPSHOT_JSON" | python3 -c "import json,sys; data=json.load(sys.stdin); print(data[0]['digest'])")"
SIZE_GB="$(echo "$SNAPSHOT_JSON" | python3 -c "import json,sys; data=json.load(sys.stdin); print(round(data[0]['size']/1024/1024/1024,2))")"

echo "Latest digest: $DIGEST"
echo "Approx size:   ${SIZE_GB} GB"

if [[ -d "$DEST_DIR/$DIGEST" ]]; then
  echo
  echo "Snapshot $DIGEST already present at $DEST_DIR/$DIGEST"
  echo "Skipping download. To re-download: rm -rf $DEST_DIR/$DIGEST"
  exit 0
fi

echo
echo "== Downloading snapshot =="
DOWNLOAD_URL="$(echo "$SNAPSHOT_JSON" | python3 -c "import json,sys; data=json.load(sys.stdin); print(data[0]['locations'][0])")"
echo "URL: $DOWNLOAD_URL"

mkdir -p "$DEST_DIR/$DIGEST"
curl -fSL --progress-bar "$DOWNLOAD_URL" -o "$DEST_DIR/$DIGEST/snapshot.tar.gz"

echo
echo "== Extracting =="
tar -xzf "$DEST_DIR/$DIGEST/snapshot.tar.gz" -C "$DEST_DIR/$DIGEST/"
rm "$DEST_DIR/$DIGEST/snapshot.tar.gz"
echo "Extracted to $DEST_DIR/$DIGEST/"

echo
echo "== Done =="
echo "Snapshot ready at: $DEST_DIR/$DIGEST/"
echo
echo "NOTE: omega-ingest's UTXO subcommand currently parses a"
echo "      simplified hand-crafted CBOR fixture, NOT the real"
echo "      Conway LedgerState shape. Real LedgerState ingestion"
echo "      lands in the follow-up omega-commitment-ingest-mainnet plan."
