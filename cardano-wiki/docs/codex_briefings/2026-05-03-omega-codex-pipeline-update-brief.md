# Codex briefing — 2026-05-03 — v1.0 ingestion pipeline update

> Drop into Codex when its mental model still assumes the single-LedgerState-CBOR dump path. Updates Codex on the two-stream split (LSQ-client for utxo/token-policy/script; ledger-state JSON for stake/governance) and the new `omega-utxo-snapshot` workspace member.

---

**SUBJECT: Workflow update — drop the LedgerState-CBOR plan; v1.0 ingestion is now a two-stream pipeline**

You are auditing the omega-commitment workspace at `/home/hoskinson/omega-commitment`. Your previous mental model assumed a single dump path: `cardano-cli query ledger-state --output-cbor → one CBOR file → omega-ingest parses all 5 sub-trees out of it`. That model is obsolete as of 2026-05-03. Update yourself before continuing any audit.

**What changed (verified live against headless cardano-node 10.7.1 + cardano-cli 10.16.0.0 on Mithril-restored mainnet, epoch 628, slot 186,222,341):**

1. **`cardano-cli query ledger-state` does not support `--output-cbor` in 10.16.** Only `--output-json | --output-text | --output-yaml`. The single-CBOR-dump path doesn't exist in the released cli.

2. **`cardano-cli query ledger-state --output-json` strips `utxoState.utxo` to `{}` but keeps everything else.** A live 2.04 GB dump (`~/cardano/snapshots/ledger_state_20260502_235649.json`) verified contains: 1,474,666 stake accounts; 2,940 stake pools; 1,016 DReps; 2,499,064 stake credentials; 3 snapshots × 1.32M activeStake + 2.94k stakePoolsSnapShot; full governance state (proposals, committee, constitution, treasury, reserves); `blocksBefore`/`blocksCurrent` block-leader counts. The single JSON file is the **complete input** for the stake AND governance sub-trees — there is no need for separate `query stake-distribution` / `query drep-state` / `query pool-state` / `query gov-state` / `query committee-state` / `query treasury` calls. If you see an interim revision recommending six separate JSON queries, ignore it; it was redundant.

3. **`cardano-cli ... query utxo --whole-utxo` is documented "only appropriate on small testnets"** (verbatim from `cardano-cli conway query utxo --help`) and FAILS on mainnet at offset ~978 MB into the response stream with `DeserialiseFailure "Decoding TxIx: More than 16bits was supplied"`. Root cause: `Cardano/Ledger/Address.hs:847` reads pointer-address `TxIx` via `decodeVariableLengthWord16`; the encoder (`putPtr`, line 348) writes variable-length-Word64. Mainnet's historical record contains pointer-address TxOuts whose `TxIx` exceeds 16 bits. Hotfix lives in PR `IntersectMBO/cardano-cli#1350` ("Add an srp for a cardano-ledger with the UTxO decoding fix", opened 2026-03-19), unmerged. There is no in-the-box cardano-cli path that produces the whole mainnet UTxO.

**The new v1.0 ingestion pipeline is split into two independent streams:**

| Source | Sub-trees produced |
|---|---|
| `cardano-cli conway query ledger-state --mainnet --out-file ...json` (single ~2 GB JSON) | stake, governance |
| `omega-utxo-snapshot` binary (NEW crate, see below) → CBOR file | utxo, token-policy, script |

**NEW WORKSPACE MEMBER:** `crates/omega-utxo-snapshot/` (added to root `Cargo.toml` `members`). Contents:
- `Cargo.toml` — depends on `pallas-network = "0.30"`, `pallas-codec = "0.30"`, `tokio` (rt-multi-thread+macros+fs+io-util), `clap`, `anyhow`, `hex`. Single `[[bin]]` target named `omega-utxo-snapshot`.
- `src/main.rs` — Tokio binary that:
  1. `NodeClient::connect(socket, magic)` — handshakes against the Cardano node Unix socket
  2. `client.statequery().acquire(None)` — acquires the latest tip
  3. Issues `Request::LedgerQuery(LedgerQuery::BlockQuery(era, BlockQuery::GetUTxOWhole))` via `client.query::<_, AnyCbor>(...)`
  4. Writes the raw CBOR response bytes to `--out`
  5. `send_release` + `send_done`
- CLI surface: `--socket <path> --network mainnet|preview|preprod|<u64> --era <u16, default 6 (Conway)> --out <path>`
- Output is bit-identical to what a fixed cardano-cli would have written with `--output-cbor-bin`.

The pallas-network 0.30.2 CBOR decoder does NOT share the Haskell ledger's Word16-VLE TxIx asymmetry bug — verified by smoke-test against the live mainnet node (process running healthy at 99% CPU, RSS climbing linearly with no decoder failure as of this writing).

**What this means for your audit:**

- **Original v1.0 plan Tasks 4–8 are still live** but their *input file* changes:
  - Task 4 (UTxO mainnet parser): input is now the `omega-utxo-snapshot --out` CBOR file (`BlockQuery::GetUTxOWhole` raw response), NOT a slice of a full LedgerState CBOR.
  - Tasks 5, 6 (token-policy, script): unchanged — both derive from the UTxO walk in Task 4.
  - Tasks 7, 8 (stake, governance): input is the `cardano-cli query ledger-state` JSON file. Use `serde_json::from_reader` streaming. Stake reads `stateBefore.esLState.delegationState` + `stateBefore.esLState.utxoState.stake.credentials` + `stateBefore.esSnapshots.{pstakeMark,pstakeSet,pstakeGo}`. Governance reads `stateBefore.esLState.delegationState.vstate` + `stateBefore.esLState.utxoState.ppups`. Both sub-trees read the SAME 2 GB JSON file — the parser dispatch is the only difference.
- **`scripts/setup_headless_node.md` is the canonical setup runbook** (replaces the old `setup_daedalus.md` — Daedalus path was abandoned because this is a headless box).
- **A new Task 2b in the v1.0 plan** documents the `omega-utxo-snapshot` binary; it is SHIPPED (compiles clean, smoke-test in flight). Do not propose rebuilding it via cardano-cli #1350 patching unless smoke-test fails — that's option B and is the fallback.
- **Leaf encodings, sub-tree roots, bundle assembly, and the 18 pinned golden vectors are UNCHANGED.** Only the input pipeline + parser strategy revise. v0.9.1's 248 tests all still pass — those are unaffected by this change.

**Updated reference docs (read these for context, in order):**
1. `docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md` — the "REVISION 2026-05-03" section at the top + the new Task 2b
2. `wiki/log.md` — the 2026-05-03 entries (infra + discovery)
3. `omega-commitment/scripts/setup_headless_node.md`
4. `omega-commitment/crates/omega-utxo-snapshot/{Cargo.toml,src/main.rs}`

**What we want from you (after you've absorbed the above):** re-audit the workspace under the new model. Flag any place where stale assumptions persist (code comments, doc strings, plan sections, test fixtures, README). Do not propose reverting to the single-CBOR-dump architecture — that path is closed at the cardano-cli level, not at our level.
