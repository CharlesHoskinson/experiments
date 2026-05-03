---
title: Ouroboros Consensus Family
slug: ouroboros-consensus
tags: [protocol, consensus, ouroboros, proof-of-stake]
sources: [cardano-docs-ouroboros, iog-chronos, wikipedia-ouroboros, nomos-ouroboros-family]
confidence: high
provenance:
  - cardano-docs-ouroboros -> Ouroboros is the first peer-reviewed PoS protocol underlying Cardano
  - nomos-ouroboros-family -> evolution from Classic -> BFT -> Praos -> Genesis
  - iog-chronos -> Praos uses VRF for local block-leader determination
  - wikipedia-ouroboros -> Genesis adds chain selection rule that allows bootstrap from genesis without trusted checkpoints
created: 2026-05-01
updated: 2026-05-01
aliases: [Ouroboros, Praos, Genesis, Cardano consensus]
cssclass: wiki-page
---

# Ouroboros Consensus Family

Ouroboros is Cardano's proof-of-stake consensus protocol — the first **provably secure**, peer-reviewed PoS protocol. It has evolved through several variants, each strengthening its threat model.

## Variants

| Version | Key idea | Security model |
|---|---|---|
| **Classic** (2017) | Multi-party coin-flip + leader schedule from prior epoch | Synchronous, static adversary |
| **BFT** | Federated bridge during initial Cardano launch | Permissioned set of nodes |
| **Praos** | Each slot, every stake-pool runs a **VRF** locally to discover if it leads. Adversary can corrupt any party with delay. | Semi-synchronous, adaptive adversary |
| **Genesis** | Adds a chain-selection rule that lets a fresh node bootstrap **from the genesis block** without trusted checkpoints. | Same as Praos + dynamic availability |
| **Chronos** | Network-time agreement protocol layered on Genesis. | Removes dependence on NTP |
| **Leios** | Pipelines block production into Input/Endorser/Ranking blocks → 30–65× throughput. See [[leios-scaling]]. | Currently under R&D |

## Why VRFs matter

A VRF lets a stake pool prove it was elected for a slot without any global broadcast — and an adversary cannot predict who will lead next. This is the foundation for Cardano's resistance to **grinding attacks** and selective DoS.

## Stake distribution & SPOs

- Block-production rights are proportional to delegated stake
- Stake delegation is **non-custodial** — ADA never leaves the wallet to delegate
- ~3,000 stake pools; "k parameter" sets the saturation point that incentivizes diversification

## Where it sits in the stack

Ouroboros is the **consensus layer**. Above it: the [[eutxo-model]] ledger and Plutus/native scripting. Below it: the network layer (`ouroboros-network` repo) handling pipelined block diffusion.
