# Headless cardano-node + Mithril fast-bootstrap

Goal: a synced mainnet `cardano-node` plus a `cardano-cli` queryable socket,
without Daedalus or any GUI. This is the canonical v1.0 real-data source path.

This runbook is what's currently being executed against `/home/hoskinson/cardano/` to produce the v1.0 / v1.1 real-data inputs.

## System requirements

- Linux x86_64 (or arm64 — both binaries available)
- ~250 GB free disk for the synced ledger DB (217 GiB Mithril snapshot + headroom)
- 16+ GB RAM (cardano-node 10.7.1's LSM UTxO backend reduces RAM relative to older releases)
- Network: ~32 MB/s sustained for snapshot download = ~2 h wall time

## 1. Layout

```
~/cardano/
├── bin/                      cardano-node, cardano-cli, mithril-client
├── config/                   mainnet config + genesis files + verification keys
├── db/                       Mithril-restored ledger DB (target ~217 GiB)
├── socket/                   node.socket
├── logs/                     node.log + mithril_download.log
├── downloads/                tarballs (kept for reproducibility)
└── start_node.sh             cardano-node launcher
```

## 2. Download binaries

```bash
mkdir -p ~/cardano/{bin,config,db,socket,logs,downloads}
cd ~/cardano/downloads

# cardano-node 10.7.1 (matches Daedalus 8.0's bundled version)
curl -fSL -o cardano-node-10.7.1-linux-amd64.tar.gz \
  https://github.com/IntersectMBO/cardano-node/releases/download/10.7.1/cardano-node-10.7.1-linux-amd64.tar.gz
tar -xzf cardano-node-10.7.1-linux-amd64.tar.gz -C cn-extract --strip-components 0

# mithril-client 2617.0
curl -fSL -o mithril-2617.0-linux-x64.tar.gz \
  https://github.com/input-output-hk/mithril/releases/download/2617.0/mithril-2617.0-linux-x64.tar.gz
mkdir -p mithril-extract && tar -xzf mithril-2617.0-linux-x64.tar.gz -C mithril-extract

# Install
cp cn-extract/bin/cardano-node ../bin/
cp cn-extract/bin/cardano-cli  ../bin/
cp mithril-extract/mithril-client ../bin/
chmod +x ../bin/*

# Mainnet config + genesis files (bundled in the cardano-node tarball)
cp cn-extract/share/mainnet/* ../config/
```

Verify:
```bash
~/cardano/bin/cardano-node    --version | head -1   # cardano-node 10.7.1
~/cardano/bin/cardano-cli     --version | head -1   # cardano-cli 10.16.x
~/cardano/bin/mithril-client  --version             # mithril-client 0.13.x
```

## 3. Mithril verification keys (mainnet)

```bash
cd ~/cardano
curl -fsSL -o config/genesis.vkey \
  https://raw.githubusercontent.com/input-output-hk/mithril/main/mithril-infra/configuration/release-mainnet/genesis.vkey
curl -fsSL -o config/ancillary.vkey \
  https://raw.githubusercontent.com/input-output-hk/mithril/main/mithril-infra/configuration/release-mainnet/ancillary.vkey
```

## 4. Download the latest Mithril mainnet snapshot

```bash
cd ~/cardano
GVK=$(cat config/genesis.vkey)
AVK=$(cat config/ancillary.vkey)

# Background; ~2h wall time at 30+ MB/s; 217 GiB target
AGGREGATOR_ENDPOINT="https://aggregator.release-mainnet.api.mithril.network/aggregator" \
GENESIS_VERIFICATION_KEY="$GVK" \
ANCILLARY_VERIFICATION_KEY="$AVK" \
nohup ./bin/mithril-client cardano-db download latest \
  --download-dir db \
  --include-ancillary \
  > logs/mithril_download.log 2>&1 &
echo $! > logs/mithril_download.pid
```

`--include-ancillary` pulls the ledger-state + most-recent immutable. Without it, cardano-node would have to replay from the snapshot height to current tip (slower but smaller download).

Monitor:
```bash
watch -n 30 'echo "elapsed: $(ps -p $(cat ~/cardano/logs/mithril_download.pid) -o etime=)"; du -sh ~/cardano/db'
```

## 5. Start cardano-node

After the snapshot download completes:

```bash
nohup ~/cardano/start_node.sh > ~/cardano/logs/node.log 2>&1 &
echo $! > ~/cardano/logs/node.pid
```

The node will catch up from the snapshot tip to current chain tip — typically minutes (Mithril snapshots are ~6 hours behind real-time tip). Monitor:

```bash
tail -f ~/cardano/logs/node.log
```

## 6. Verify with cardano-cli

```bash
export CARDANO_NODE_SOCKET_PATH=~/cardano/socket/node.socket
~/cardano/bin/cardano-cli query tip --mainnet
```

Wait until `syncProgress` reaches `100.00`.

## 7. Dump per-sub-tree inputs (for v1.0 omega-ingest mainnet path)

The v1.0 ingestion pipeline uses **two independent input streams** — see the
2026-05-03 revision in
`docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`.

| Source | Sub-trees produced |
|---|---|
| `cardano-cli conway query ledger-state` (JSON, ~2 GB) | stake, governance |
| `omega-utxo-snapshot` (binary, CBOR) | utxo, token-policy, script |

### 7a. Stake + governance source — LedgerState JSON

```bash
cd ~/omega-commitment
./scripts/dump_ledger_state.sh
# → ~/cardano/snapshots/ledger_state_<TS>.json (~2 GB)
```

`cardano-cli query ledger-state` strips `utxoState.utxo` to `{}` on mainnet
and does not support `--output-cbor` in 10.16 (only json|text|yaml). The
JSON is otherwise complete: 1.47M stake accounts, 2.94k pools, 1,016 DReps,
3 snapshots × 1.32M activeStake, full governance state. Both stake AND
governance sub-trees parse this single file.

Manual equivalent:

```bash
export CARDANO_NODE_SOCKET_PATH=~/cardano/socket/node.socket
mkdir -p ~/cardano/snapshots
~/cardano/bin/cardano-cli conway query ledger-state --mainnet \
  --out-file ~/cardano/snapshots/ledger_state_$(date +%Y%m%d_%H%M%S).json
```

### 7b. UTXO source — `omega-utxo-snapshot` binary

```bash
cd ~/omega-commitment
. "$HOME/.cargo/env"
cargo build -p omega-utxo-snapshot --release

./target/release/omega-utxo-snapshot \
  --socket ~/cardano/socket/node.socket \
  --network mainnet \
  --era 6 \
  --out ~/cardano/snapshots/utxo_$(date +%Y%m%d_%H%M%S).cbor
```

This bypasses `cardano-cli ... query utxo --whole-utxo` (documented as
"only appropriate on small testnets" and broken on mainnet by the
`Cardano/Ledger/Address.hs:847` Word16-VLE TxIx decoder bug; PR
`IntersectMBO/cardano-cli#1350` carries the hotfix but is unmerged).
Pallas's CBOR decoder doesn't share that asymmetry; the output is
bit-identical to what a fixed cardano-cli would write with
`--output-cbor-bin`. `omega-ingest utxo --format mainnet` consumes this
file unchanged.

## 8. Stopping

```bash
kill $(cat ~/cardano/logs/node.pid)
# Wait a few seconds for cardano-node's clean shutdown
```

Re-starting is idempotent — the LSM DB picks up where it left off.

## Troubleshooting

- **Mithril certificate failure**: re-fetch `genesis.vkey` and `ancillary.vkey` (rare, only if mainnet keys rotate).
- **Disk full mid-download**: snapshot is 217 GiB compressed; needs ~250 GiB headroom for restore. If short, fall back to `--no-include-ancillary` (skip ledger state, replay from snapshot tip; saves a few GB but adds replay time).
- **Socket not appearing after node start**: cardano-node 10.7.1 creates the socket only after the chain DB opens cleanly. First run after a fresh snapshot can take 5–10 min before the socket appears. Watch `node.log` for `Listening on address: LocalAddress`.
- **`query tip` says `syncProgress < 100`**: chain catchup from snapshot tip to current tip; expect 5–30 min depending on how stale the snapshot was.
