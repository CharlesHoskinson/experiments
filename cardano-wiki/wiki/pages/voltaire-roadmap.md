---
title: Voltaire Roadmap (2025–2030)
slug: voltaire-roadmap
tags: [roadmap, voltaire, plomin, leios, midnight]
sources: [cardano-roadmap, beincrypto-plomin-roadmap, iog-engineering-proposal]
confidence: medium
provenance:
  - cardano-roadmap -> Voltaire focuses on scalability, usability, utility
  - iog-engineering-proposal -> IOG 2025 engineering proposal scopes Leios + Hydra + state minimization
created: 2026-05-01
updated: 2026-05-01
aliases: [Voltaire era, Cardano roadmap, Vision 2030]
cssclass: wiki-page
---

# Voltaire Roadmap (2025–2030)

Voltaire is the fifth and final named Cardano era — focused on **decentralized governance + scalability + utility**. It's not a single milestone but an ongoing arc.

## Three-pillar framing

### 1. Scalability
- **Leios** — pipelined L1 throughput (see [[leios-scaling]])
- **Hydra 2.0** — alpha live; mainnet productionization through 2026
- **Mithril** — ubiquitous certificates for fast bootstrap and bridging
- **State minimization** — reference inputs, on-chain script storage, garbage collection of unused UTXOs
- **Sidechains / Partner chains** — `cardano-sidechains` toolkit; Midnight as flagship privacy chain

### 2. Usability
- **Wallet UX** — better account abstractions, Eternl/Yoroi/Lace/Daedalus differentiation
- **Account abstraction** (CIP-69+) and stake credential flexibility
- **Reference scripts in DApps** — cheaper composability
- **Aiken / Plu-ts / Helios** — friendlier dev languages
- **Pragma / Catalyst** — accelerator + funding pipeline

### 3. Utility
- **Stablecoins** — USDM (Mehen), USDC bridge, Djed (algorithmic)
- **DeFi TVL recovery** — Minswap, Liqwid, Indigo, Sundae, Aada, Lenfi
- **RWAs and tokenization** — Emurgo + Cardano Foundation pilots
- **Identity** — Atala PRISM (now Hyperledger Identus), did:cardano
- **Bitwala-style banking integrations** — partnerships with regulated entities

## Vision 2030

Set under Intersect's 2026 priorities. Goals being formalized include:
- Self-sustaining treasury without dilution from new ADA issuance (current 0.3% reserve drawdown per epoch)
- 1,000+ TPS sustained throughput (Leios + Hydra)
- Full on-chain decentralized governance with mature DRep ecosystem
- A diversified funding pipeline (Catalyst v3, Innovation Budget, on-chain bounties)
- Mature L2 portfolio: Hydra Heads, Tail (cheap rollup), Midnight (privacy), partner sidechains

## What's *under-roadmap'd* (worth watching)

- **MEV / ordering** — Cardano's deterministic EUTXO model dampens MEV but doesn't eliminate it; no canonical solution shipped
- **Privacy on L1** — currently outsourced to Midnight sidechain
- **AI-native primitives** — agent-driven txns, x402-style payments (Cardano added official x402 support April 2026)
- **Account abstraction for full smart-contract wallets** — partial via stake addresses but not feature-equivalent to ERC-4337

See also: [[plomin-hard-fork]], [[leios-scaling]], [[hydra-scaling]], [[cip-1694-governance]], [[midnight-sidechain]].
