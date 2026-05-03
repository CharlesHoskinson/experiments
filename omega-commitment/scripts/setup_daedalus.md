# Daedalus 8.0.0 + Mithril Fast Bootstrap Setup Runbook

Deprecated alternative: Omega v1.0 real-data work is currently using the
headless `cardano-node` + `mithril-client` path in
`scripts/setup_headless_node.md`. Keep this document for reference only unless
the v1.0 source plan is explicitly moved back to Daedalus.

This runbook gets a workstation from no local Cardano mainnet node to a synced
node with a `cardano-cli` queryable socket, ready to dump Conway-era
LedgerState CBOR for Omega v1.0 ingestion.

Use Daedalus 8.0.0 for the v1.0 golden-vector capture unless the plan is
updated and the output vectors are deliberately re-pinned. Newer Daedalus or
cardano-node releases may be valid, but they are a different reproducibility
baseline.

For a server or non-GUI host, see `scripts/setup_headless_node.md`; it targets
the same end-state using standalone `cardano-node`, `cardano-cli`, and
`mithril-client`.

## Requirements

- Disk: at least 100 GB free for the mainnet ledger DB plus growth headroom.
- RAM: 16 GB minimum; 24 GB or more recommended.
- Network: enough bandwidth for a multi-GB Mithril bootstrap download.
- A separately installed `cardano-cli`; Daedalus 8.0.0 does not bundle it.

## 1. Install Daedalus

Download the Daedalus Mainnet 8.0.0 installer for your platform from the
official Daedalus download page:

```text
https://daedaluswallet.io/download/
```

Expected installer families:

- macOS Apple Silicon: `Daedalus Mainnet 8.x.x-darwin-arm64.pkg`
- macOS Intel: `Daedalus Mainnet 8.x.x-darwin-x86_64.pkg`
- Linux: `Daedalus Mainnet 8.x.x.bin`
- Windows: `Daedalus Mainnet 8.x.x.exe`

On Linux:

```bash
chmod +x ./Daedalus*.bin
./Daedalus*.bin
```

## 2. First-run Mithril bootstrap

Start Daedalus Mainnet. On a fresh install, Daedalus 8.0.0 uses Mithril
fast-bootstrap to restore a recent mainnet snapshot, then catches up from the
snapshot point to the current chain tip.

Expected phases:

1. Mithril mainnet aggregator selection.
2. Snapshot download.
3. Snapshot certificate verification.
4. Snapshot restore into the local node DB.
5. Tip catch-up from snapshot height to current tip.

Typical wall time is 2-8 hours depending on network, disk, and CPU.

Do not continue until Daedalus reports the node is fully synced.

## 3. Locate the node socket

Default socket paths:

- Linux: `~/.local/share/Daedalus/mainnet/cardano-node.socket`
- macOS: `~/Library/Application Support/Daedalus mainnet/cardano-node.socket`
- Windows: `\\.\pipe\Daedalus_mainnet`

Linux verification:

```bash
ls -la "$HOME/.local/share/Daedalus/mainnet/cardano-node.socket"
```

The file should exist while Daedalus is running.

## 4. Install cardano-cli

Install a `cardano-cli` release compatible with Daedalus 8.0.0's bundled
`cardano-node` version. The v1.0 plan pins Daedalus 8.0.0 because it bundles
node 10.7.1.

Linux x86_64 example:

```bash
NODE_VER=10.7.1
mkdir -p "$HOME/cardano-cli-download"
cd "$HOME/cardano-cli-download"
wget "https://github.com/IntersectMBO/cardano-node/releases/download/${NODE_VER}/cardano-node-${NODE_VER}-linux.tar.gz"
tar -xzf "cardano-node-${NODE_VER}-linux.tar.gz"
sudo install ./bin/cardano-cli /usr/local/bin/cardano-cli
cardano-cli --version
```

The reported `cardano-cli` version should be from the same 10.7.x release
family used by the node.

## 5. Verify cardano-cli can query Daedalus

Linux:

```bash
export CARDANO_NODE_SOCKET_PATH="$HOME/.local/share/Daedalus/mainnet/cardano-node.socket"
cardano-cli query tip --mainnet
```

Expected output is JSON containing fields such as `slot`, `epoch`,
`block`, `hash`, and `syncProgress`.

Continue only when `syncProgress` is `100.00`.

## 6. Dump LedgerState CBOR

After Task 2 adds `scripts/dump_ledger_state.sh`, use it as the canonical dump
wrapper:

```bash
./scripts/dump_ledger_state.sh
```

Manual equivalent:

```bash
mkdir -p var/ledger_state
cardano-cli query ledger-state \
  --mainnet \
  --output-cbor \
  --out-file "var/ledger_state/ledger_state_$(date +%Y%m%d_%H%M%S).cbor"
```

The output is multi-GB and may take several minutes to write. This CBOR file is
the input for the Omega v1.0 `omega-ingest --format mainnet` path.

## Troubleshooting

- Socket missing: Daedalus must be running. Restart Daedalus if the UI says it
  is synced but the socket is absent.
- `syncProgress` below `100.00`: wait for Daedalus to finish tip catch-up.
- CLI query fails: verify `CARDANO_NODE_SOCKET_PATH` and make sure
  `cardano-cli` is compatible with Daedalus's bundled node.
- Ledger DB corruption: shut down Daedalus, back up wallet state, then rebuild
  the node DB and re-run Mithril bootstrap.
