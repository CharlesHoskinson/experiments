---
title: Hydra — Layer 2 Heads
slug: hydra-scaling
tags: [scaling, layer2, hydra, state-channels]
sources: [cardano-docs-hydra, hydra-family, iog-hydra-scaling, coinalert-hydra2]
confidence: high
provenance:
  - cardano-docs-hydra -> Hydra is Cardano's family of L2 protocols, with Hydra Head as the first
  - hydra-family -> Hydra Head opens an isomorphic state channel between a fixed party set
  - coinalert-hydra2 -> Hydra 2.0 alpha removed the commit phase and the non-abortable head issue
created: 2026-05-01
updated: 2026-05-01
aliases: [Hydra, Hydra Head, Hydra 2.0]
cssclass: wiki-page
---

# Hydra — Layer 2 Heads

Hydra is Cardano's L2 family. The first member, **Hydra Head**, is a state channel where a small set of parties run an *isomorphic* off-chain ledger using exactly the same EUTXO + Plutus rules as L1.

## How a Head works

1. **Init** — N parties post a Head contract on L1, depositing collateral
2. **Commit** — each party commits some L1 UTXOs into the Head
3. **Open** — once all committed, the Head becomes a private mini-ledger
4. **Off-chain txs** — parties co-sign transactions over the local state at near-instant latency
5. **Close / Contest / Fanout** — settle back to L1, with a contestation window for fraud proofs

## Why "isomorphic"?

Anything that runs on L1 runs unchanged inside a Head — same Plutus scripts, same datums, same fee model. No rewriting required.

## Hydra 2.0 (alpha — 2026)

Major reworks shipped or shipping in 2026:
- **Commit phase removed** — Heads can open immediately and accept rolling deposits
- **Non-abortable head** issue eliminated by removing `collectCom`/`abort` round-trip
- **Plutus script timing** improved on mainnet
- **~4× cost reduction** for opening a Head
- Multi-Head deposit isolation

## Limitations of Heads (and why "Hydra ≠ Lightning")

- **Closed party sets** — you must know who's in the Head; no permissionless fan-in
- **All parties online** for the optimistic path
- **Head exits cost L1 fees** proportional to UTXO count

## Beyond Heads — the "L2 portfolio"

In June 2025 IOG framed Hydra as one piece of a larger L2 strategy, including:
- **Tail / Hydra-as-cheap-rollup** research
- **Midnight** as a privacy sidechain (separate token, ZK-based)
- **Sidechain toolkit** (cardano-sidechains)
- **Optimistic / ZK rollups** under research

See also: [[leios-scaling]] for L1 throughput improvements that complement Hydra.
