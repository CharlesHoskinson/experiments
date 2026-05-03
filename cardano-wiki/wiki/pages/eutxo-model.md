---
title: Extended UTXO (EUTXO) Model
slug: eutxo-model
tags: [protocol, ledger, eutxo, transactions]
sources: [cardano-docs-eutxo, iog-native-tokens, adapulse-eutxo-guide]
confidence: high
provenance:
  - cardano-docs-eutxo -> EUTXO extends Bitcoin's UTXO with arbitrary validators (datums + redeemers)
  - iog-native-tokens -> Cardano native tokens live alongside ADA in the same EUTXO outputs without contracts
  - adapulse-eutxo-guide -> EUTXO offers deterministic fees and off-chain validation
created: 2026-05-01
updated: 2026-05-01
aliases: [EUTXO, Extended UTXO]
cssclass: wiki-page
---

# Extended UTXO (EUTXO) Model

Cardano extends Bitcoin's UTXO ledger with two ideas:
1. Outputs carry **datums** — arbitrary script data
2. Inputs carry **redeemers** — arbitrary script witnesses

A spending validator sees `(datum, redeemer, script_context)` and either accepts or rejects, deterministically.

## Properties (vs. account model like Ethereum)

| Property | EUTXO (Cardano) | Account (Ethereum) |
|---|---|---|
| **Determinism** | Tx outcome computable off-chain before submission | Outcome depends on global state at execution time |
| **Fee predictability** | Exact fee known pre-submission | Gas can blow out mid-tx |
| **Atomicity** | Tx fully succeeds or fully fails — never partial | Out-of-gas can leave half-applied state |
| **Concurrency** | Disjoint UTXOs are independently updatable | Hot-account contention serialises calls |
| **Composability** | Achieved via "design patterns" (batchers, NFTs, oracles) — harder to reason about | Direct contract-to-contract calls — easy but costly |

## What this enables

- **Parallel validation** — workers can validate any disjoint subset of a block's inputs simultaneously
- **Off-chain proving / building** — the bulk of dApp logic can run client-side, then a deterministic on-chain check verifies it
- **Native tokens** — multi-asset outputs (ADA + arbitrary native tokens) without an ERC-20-style contract; minting policies are themselves Plutus scripts

## What it makes harder

- **Global state apps** (orderbooks, lending pools): require off-chain "batchers" that aggregate user intents into a single tx — see Sundae, Minswap V2, Aada, Liqwid.
- **Synchronous cross-contract calls**: there are no calls. Composition happens by chaining UTXOs across a transaction.
- **Onboarding devs from Solidity world** — mental model shift is real.

## EUTXO design patterns

- **State threads** — an NFT carried through evolving UTXOs to identify a contract's "current" state
- **Batchers** — off-chain matchers that fold many user intents into one tx
- **Reference inputs (Vasil, 2022)** — read another UTXO's data without consuming it; cuts contention dramatically
- **Inline datums** — store datum directly in output (not just hash) for simpler off-chain reads
- **Reference scripts** — script bytecode lives once on-chain; spent txs link to it instead of re-embedding
