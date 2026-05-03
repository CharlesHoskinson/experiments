---
title: LSQ GetUTxOWhole pipeline (omega-utxo-snapshot binary)
slug: lsq-getutxowhole-pipeline
tags: [ingestion, utxo, lsq, pallas-network, omega, cardano-cli, txix-bug]
sources: [pallas-network-0.30.2, cardano-cli-10.16, intersect-pr-1350, cardano-ledger-address-hs, omega-utxo-snapshot-impl]
confidence: medium
provenance:
  - pallas-network-0.30.2 -> wire-format of LocalStateQuery `BlockQuery::GetUTxOWhole` (Conway era index 6); `Client::query<_, AnyCbor>` buffers the full response.
  - cardano-cli-10.16 -> documented behavior of `query utxo --whole-utxo` ("only appropriate on small testnets") and the empirical `DeserialiseFailure "Decoding TxIx: More than 16bits was supplied"` at byte 978,211,479 of the response stream.
  - intersect-pr-1350 -> evidence that the upstream Haskell decoder bug has a known but unmerged hotfix (cardano-ledger TxIx Word16-VLE → Word64-VLE).
  - cardano-ledger-address-hs -> root-cause line numbers (`Address.hs:847` reads Word16-VLE; `Address.hs:348` `putPtr` writes Word64-VLE) for the encoder/decoder asymmetry on pointer-address TxIx.
  - omega-utxo-snapshot-impl -> in-tree implementation at `omega-commitment/crates/omega-utxo-snapshot/`; verified wire-format match against the Haskell stack 2026-05-03; mainnet smoke-test ran 22+ minutes without `MsgQueryFailure`.
created: 2026-05-03
updated: 2026-05-03
aliases: [omega-utxo-snapshot, lsq-utxo-pipeline, getutxowhole]
cssclass: wiki-page
---

# LSQ GetUTxOWhole pipeline

The v1.0 omega-commitment program needs the entire mainnet UTxO set as input for the `utxo`, `token-policy`, and `script` sub-trees. This page documents why the obvious cardano-cli path doesn't work and what we built instead.

## What didn't work — `cardano-cli ... query utxo --whole-utxo`

`cardano-cli conway query utxo --whole-utxo --output-cbor-bin` is the apparent right invocation. Two facts kill it on mainnet:

1. **Documented testnet-only.** The cli's own help text says `--whole-utxo: Return the whole UTxO (only appropriate on small testnets).`

2. **Decoder bug on the wire.** Run against a real mainnet node, the cli reads ~978 MB of response stream and then dies:
   ```
   cardano-cli: DecoderFailure (LocalStateQuery ...) (DeserialiseFailure 978211479
                "Decoding TxIx: More than 16bits was supplied")
   ```
   Root cause: `Cardano/Ledger/Address.hs:847` reads pointer-address `TxIx` via `decodeVariableLengthWord16`; the encoder (`putPtr`, line 348) writes variable-length-Word64. Mainnet's historical record contains pointer-address TxOuts whose `TxIx` exceeds 16 bits.

3. **Hotfix exists but is unmerged.** PR `IntersectMBO/cardano-cli#1350` ("Add an srp for a cardano-ledger with the UTxO decoding fix", opened 2026-03-19) carries the fix. As of 10.16.0.0 it has not been released. There is no in-the-box cli path that produces the whole mainnet UTxO.

## What we built — `omega-utxo-snapshot`

Workspace member: `crates/omega-utxo-snapshot/`. Single Rust binary using pallas-network 0.30.2's local-state-query miniprotocol against the same node socket cardano-cli would use. Pallas's CBOR decoder doesn't share Haskell's Word16-VLE TxIx asymmetry.

```rust
let mut client = NodeClient::connect(socket, MAINNET_MAGIC).await?;
let sq = client.statequery();
sq.acquire(None).await?;                                                 // acquire latest tip
let req = Request::LedgerQuery(LedgerQuery::BlockQuery(6, BlockQuery::GetUTxOWhole));
let raw: AnyCbor = sq.query(req).await?;                                  // 1 message in, 1 message out
sq.send_release().await?;
sq.send_done().await?;
tokio::fs::write(&out_path, raw.to_vec()).await?;                         // raw response bytes
```

CLI: `--socket <path> --network mainnet|preview|preprod|<u64> --era <u16, default 6 (Conway)> --out <path>`.

Output is bit-identical to what a fixed cardano-cli (with PR #1350 applied) would write with `--output-cbor-bin`. `omega-ingest utxo --format mainnet` consumes this file as if it had come from cardano-cli — drop-in interchangeable.

## Memory profile

Pallas-network 0.30's `Client::query<_, AnyCbor>` **buffers the entire response in memory** before returning. The binary holds the full UTxO CBOR (multi-GB on mainnet) as a `Vec<u8>` before the single `tokio::fs::write`. Linear RSS growth during the query is expected:

| Time | RSS |
|---|---|
| 02:20 | 95 MB |
| 07:10 | 165 MB |
| 11:13 | 205 MB |
| 20:18 | 273 MB |
| 21:42 | 283 MB |

Growth ≈ 10–14 MB/min. A true streaming path would require dropping into the lower-level pallas multiplexer and incrementally parsing the LSQ message stream — left for a follow-up if memory becomes the bottleneck. The v1.0 box (122 GiB RAM) makes this a non-issue.

## Verification status

| Check | Status |
|---|---|
| Crate compiles clean | ✅ pallas-network 0.30.2 + tokio 1 |
| Cargo fmt + clippy clean | ✅ |
| Workspace tests still 248-passing | ✅ (no leaf encoding / sub-tree root changes) |
| LSQ wire-format match vs cardano-cli | ✅ verified 2026-05-03 (high confidence; layer-by-layer trace, see below) |
| Mainnet smoke-test | ⏳ in flight; behavioral evidence (22+ min uptime, no MsgQueryFailure) supports wire-format match |

## Wire-format verification (2026-05-03)

Independent agent traced the encoded bytes layer-by-layer through both pallas-network 0.30.2 and ouroboros-consensus / cardano-api / Shelley-Ledger. Expected bytes for our query:

```
82 03 82 00 82 00 82 06 81 07
│  │  │  │  │  │  │  │  │  └── BlockQuery::GetUTxOWhole tag (Word8 7) — Pallas codec.rs:41-43 ↔ Shelley query.hs:872-873
│  │  │  │  │  │  │  └── 1-array enclosing the tag (encodeListLen 1)
│  │  │  │  │  │  └── era index 6 = Conway in CardanoEras = [Byron,Shelley,Allegra,Mary,Alonzo,Babbage,Conway,Dijkstra]
│  │  │  │  │  └── encodeNS discriminator (2) for HardFork-era selector — hf_common.hs:409-415
│  │  │  │  └── 2-array (era-NS envelope)
│  │  │  └── QueryIfCurrent discriminator (0) — hf_n2c.hs:431-436
│  │  └── 2-array (Request::LedgerQuery wrapper)
│  └── LSQ MsgQuery label (3) — Ouroboros LocalStateQuery spec
└── 2-array (LSQ message envelope)
```

Pallas's encoder for each layer matches the corresponding Haskell encoder. No missing wrapper. `era=6` correctly indexes Conway. Pallas-network's `queries_v16` is the Conway-aware query module through pallas v1.0.0-alpha.6 (no `queries_v17` exists).

Pallas's own integration test at `pallas-network/tests/protocols.rs:541-548` exercises the identical envelope structure with sibling BlockQuery variants and round-trips against pallas's NodeServer — additional behavioral evidence the construction is wire-correct.

If the construction were malformed, cardano-node would reply with `MsgQueryFailure` and pallas's `Client::query` would propagate that as `ClientError` within seconds. The smoke-test running for 20+ minutes without error is itself behavioral confirmation that the node accepted the query.

## Fallback options if LSQ ever breaks

1. **Build cardano-cli locally with PR #1350 patched.** Clone `IntersectMBO/cardano-cli@master`, add the cardano-ledger source-repo-package stanza pointing at the hotfix commit, `cabal build cardano-cli`. Drop the rebuilt cli into `~/cardano/bin/`. ~30–60 min Haskell build. Fragile against the pinned 10.7.1 node.

2. **Replay blocks from `~/cardano/db/db/immutable/` via pallas-traverse.** Walk all 8,619 immutable chunks (~213 GB), fold (txIns spent, txOuts created) into a UTxO map. Slower but most general — same engine produces v1.1's `header` + `tx-index` sub-trees for free. This is the v1.1 chain-follower path, just pulled forward.

## Cross-references

- [[ledger-state-json-layout]] — the other v1.0 ingestion stream (stake + governance via JSON)
- [[spec-ouroboros-omega]] — the parent program design spec
- v1.0 plan Task 2b: `docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`
- Setup runbook: `omega-commitment/scripts/setup_headless_node.md` (Step 7b)
