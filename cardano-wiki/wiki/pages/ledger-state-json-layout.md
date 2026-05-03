---
title: LedgerState JSON layout (cardano-cli 10.16, mainnet, Conway era)
slug: ledger-state-json-layout
tags: [ingestion, ledger-state, cardano-cli, mainnet, conway, omega]
sources: ["cardano-cli 10.16.0.0 conway query ledger-state", "live mainnet dump 2026-05-02 epoch 628"]
confidence: high
provenance:
  - kind: live-measurement
    when: 2026-05-03
    artifact: /home/hoskinson/cardano/snapshots/ledger_state_20260502_235649.json (2.04 GiB)
    measured-by: omega-commitment-ingest/examples/probe_ledger_state_paths.rs
created: 2026-05-03
updated: 2026-05-03
---

# LedgerState JSON layout (cardano-cli 10.16 / mainnet / Conway)

Reusable reference: every documented path that omega-commitment's stake & governance ingestion (Tasks 7 & 8 of the v1.0 plan) navigates into a `cardano-cli conway query ledger-state` JSON dump. Verified live against a 2.04 GiB mainnet dump at epoch 628, slot 186,209,073.

> **Why this page exists:** the v1.0 ingestion plan was written before we had a real LedgerState dump in hand. The 2026-05-03 architecture revision claimed specific JSON paths and entity counts; this page records the live verification of those claims and the surprises we found.

## Top-level structure

```
{
  "blocksBefore":  { ... block-leader counts },
  "blocksCurrent": { ... block-leader counts },
  "lastEpoch":     <int>,
  "possibleRewardUpdate": { ... },
  "stakeDistrib":  { ... },
  "stateBefore":   { esChainAccountState, esLState, esNonMyopic, esSnapshots }
}
```

`stateBefore` is where everything the omega-commitment stake & governance sub-trees needs lives.

## Verified paths

Measured 2026-05-03 against `~/cardano/snapshots/ledger_state_20260502_235649.json` (epoch 628):

| Path | Shape | Count | First key (sample) |
|---|---|---|---|
| `stateBefore.esLState.delegationState.dstate.accounts` | object | **1,474,666** | `keyHash-00000211a65db1b14bc63eefc9eef212cf498a576129e9fc0e1a89c3` |
| `stateBefore.esLState.delegationState.dstate.genDelegs` | object | **7** | `162f94554ac8c225383a2248c245659eda870eaa82d0ef25fc7dcd82` |
| `stateBefore.esLState.delegationState.pstate.stakePools` | object | **2,940** | `00000036d515e12e18cd3c88c74f09a67984c2c279a5296aa96efe89` |
| `stateBefore.esLState.utxoState.stake.credentials` | object | **2,499,064** | `keyHash-…` |
| `stateBefore.esSnapshots.pstakeMark.activeStake` | object | **1,322,098** | `keyHash-…` |
| `stateBefore.esSnapshots.pstakeMark.stakePoolsSnapShot` | object | **2,941** | `00000036d…` |
| `stateBefore.esSnapshots.pstakeSet.activeStake` | object | **1,321,885** | `keyHash-…` |
| `stateBefore.esSnapshots.pstakeGo.activeStake` | object | **1,321,711** | `keyHash-…` |
| `stateBefore.esLState.delegationState.vstate.dreps` | object | **1,016** | `keyHash-002e87e3…` |
| `stateBefore.esLState.delegationState.vstate.committeeState` | object | 1 | `csCommitteeCreds` |
| `stateBefore.esLState.utxoState.ppups` | object | 7 | `committee` |
| `stateBefore.esLState.utxoState.ppups.proposals` | **array** | 15 | (full GovAction object — see below) |
| `stateBefore.esLState.utxoState.ppups.committee` | object | 2 | `members` |
| `stateBefore.esLState.utxoState.ppups.constitution` | object | 2 | `anchor` |
| `stateBefore.esLState.utxoState.ppups.currentPParams` | object | 31 | `collateralPercentage` |
| `stateBefore.esChainAccountState.reserves` | number | — | `6,400,352,755,719,133` lovelace (~6.4 PADA) |
| `stateBefore.esChainAccountState.treasury` | number | — | `1,624,922,431,230,784` lovelace (~1.6 PADA) |
| `stateBefore.esLState.utxoState.utxo` | object | **0** ⚠️ | (intentionally scrubbed by cardano-cli) |

The `utxoState.utxo = {}` is documented behavior — `query ledger-state` strips the UTxO map on mainnet. The UTxO sub-tree comes from a separate stream (`omega-utxo-snapshot`, see [[lsq-getutxowhole-pipeline]]).

## Proposal object shape (governance sub-tree input)

`utxoState.ppups.proposals[i]` is a fully-populated `GovAction` record:

```jsonc
{
  "actionId": { "govActionIx": 0, "txId": "<hex32>" },
  "committeeVotes":   { "<credId>": "VoteYes"|"VoteNo"|"Abstain", ... },
  "dRepVotes":        { "<credId>": "VoteYes"|"VoteNo"|"Abstain", ... },
  "stakePoolVotes":   { "<poolId>": "VoteYes"|... },
  "expiresAfter":     <epoch-int>,
  "proposedIn":       <epoch-int>,
  "proposalProcedure": {
    "anchor":      { "dataHash": "<hex32>", "url": "<ipfs://… or https://…>" },
    "deposit":     <lovelace>,
    "returnAddr":  { "credential": { "keyHash"|"scriptHash": "<hex28>" }, "network": "Mainnet" },
    "govAction": {
      "tag": "TreasuryWithdrawals" | "InfoAction" | "HardForkInitiation" | "ParameterChange" | "NewConstitution" | "NewCommittee" | "NoConfidence",
      "contents": [ ... type-dependent ... ]
    }
  }
}
```

`govAction.contents` shape varies per `tag`. For `TreasuryWithdrawals`: `[ [ [ {credential, network}, lovelace ], ... ], policyHash ]` — a list of (recipient, amount) pairs plus a policy reference.

The first proposal in our dump (epoch 628) is a 8.0 trillion-lovelace TreasuryWithdrawals to a script-hash recipient, with 100 G-lovelace deposit, that expired at epoch 627.

## activeStake entry shape (stake sub-tree input)

`esSnapshots.pstakeSet.activeStake.<credId>` is a 2-element object:

```jsonc
{ "delegation": "<poolId-hex28>", "rewardAccountBalance": <lovelace> }
```

(Length 2 confirmed across all three snapshots.) Pool snapshots in `stakePoolsSnapShot.<poolId>` are 10-element objects (pool params + active stake size).

## currentPParams keys (31 protocol params, Conway era)

`utxoState.ppups.currentPParams` example field: `collateralPercentage`. Other expected Conway fields per `Cardano.Ledger.Conway.PParams`: `committeeMaxTermLength`, `committeeMinSize`, `dRepActivity`, `dRepDeposit`, `dRepVotingThresholds`, `govActionDeposit`, `govActionLifetime`, `minFeeRefScriptCostPerByte`, `poolVotingThresholds`, plus all Babbage/Alonzo carryovers.

## Memory & timing measurements

Probe binary: `omega-commitment-ingest/examples/probe_ledger_state_paths.rs`. Reads the entire file via `BufReader<File> → serde_json::from_reader::<_, serde_json::Value>(...)`, then walks each path.

| Metric | Value |
|---|---|
| File size | 2,141,924,029 bytes (1.99 GiB) |
| Parse time (`from_reader → Value`) | 6.47 s |
| Total wall time (parse + 17 path walks + print) | 9.31 s |
| Peak RSS (`/proc/self/status:VmHWM`) | 6,776,568 KiB (6.46 GiB) |
| **RAM-to-file ratio** | **3.24×** |
| Page faults (minor) | 1,693,658 |
| CPU | 99% single-threaded |

**Implication for Tasks 7 & 8:** parsing the full file into `serde_json::Value` is fast (sub-10s) but holds 3.24× the file size in heap. On the v1.0 box (122 GiB RAM) this is comfortably within budget. Production-grade ingestion should use serde-derived structs (avoids the generic `Value` allocator overhead, ~10× memory reduction in our experience) or a true streaming parser like `jiter`/`ijson` that processes nodes without retaining them. For the v1.0 happy path, `from_reader → Value` is acceptable.

## How to reproduce

```bash
cd /home/hoskinson/omega-commitment
cargo run --release -p omega-commitment-ingest --example probe_ledger_state_paths -- \
  /home/hoskinson/cardano/snapshots/ledger_state_<TS>.json
```

The probe is deterministic for a given snapshot — entity counts move with epoch boundaries (~5 days on mainnet) but path shapes are stable across the Conway era.

## Cross-references

- [[lsq-getutxowhole-pipeline]] — the other v1.0 ingestion stream (UTxO via `omega-utxo-snapshot`)
- [[spec-ouroboros-omega]] — the parent program design spec
- v1.0 plan: `docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md` (REVISION 2026-05-03 + Tasks 7 + 8)
