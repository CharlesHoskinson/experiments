---
title: Ouroboros Leios — Pipelined Throughput
slug: leios-scaling
tags: [protocol, scaling, leios, ouroboros]
sources: [iog-advancing-ouroboros, leios-faq, cip-0164, eu-reporter-leios]
confidence: high
provenance:
  - iog-advancing-ouroboros -> Leios decouples block production into IB/EB/RB stages for 30-65x throughput
  - leios-faq -> RBs anchor every ~20s with Praos finality; EBs every ~5s
  - cip-0164 -> Linear Leios variant proposed for simpler initial deployment
created: 2026-05-01
updated: 2026-05-01
aliases: [Leios, Ouroboros Leios, Linear Leios]
cssclass: wiki-page
---

# Ouroboros Leios — Pipelined Throughput

Leios is an **L1 throughput upgrade** for Cardano. It keeps Praos's finality and security but pipelines transaction processing into three parallel stages.

## Three block types

| Block type | Cadence | Role |
|---|---|---|
| **Input Block (IB)** | Frequent (sub-second) | Carries raw transactions; produced freely by SPOs |
| **Endorser Block (EB)** | ~5 s | Committee members aggregate + endorse IBs |
| **Ranking Block (RB)** | ~20 s | Praos-style block; references endorsed EBs and provides linear final order |

## Throughput claim

Simulations target **30–65× current Praos throughput**, exceeding 1,000 TPS at sustained load. This is achieved by parallelizing the costly part (validation, propagation) while keeping the **single-threaded ordering** for finality.

## Two flavors under discussion

- **Full Leios** (the research paper): the most ambitious pipelining, requires committee infrastructure
- **Linear Leios** (CIP-0164): simpler — drop the explicit committee/IB-EB layering, ship most of the throughput first

## Status (early 2026)

- Active R&D — Haskell + Rust simulators, formal validation underway
- 24/7 development tracker live
- Mainnet target hasn't been pinned; expect Linear Leios to land before full Leios
- "Plomin's successor" — Plomin shipped governance, Leios is the throughput leg of Voltaire

## What it doesn't fix

- **State growth** — more TPS = more UTXOs and Plutus scripts; storage / state-snapshotting are still open problems
- **Mempool propagation cost** at high TPS
- **Mev / ordering** — leios doesn't introduce per-tx ordering rules; the "fair-ordering on Cardano" debate is separate

See also: [[ouroboros-consensus]] for the Praos foundation and [[hydra-scaling]] for off-chain complement.
