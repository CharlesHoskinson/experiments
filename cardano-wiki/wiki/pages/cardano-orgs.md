---
title: Cardano Founding Organizations
slug: cardano-orgs
tags: [organization, iog, cardano-foundation, emurgo, intersect]
sources: [iog-cardano, cardano-foundation, intersect-org]
confidence: high
provenance:
  - iog-cardano -> IOG was the original R&D contractor and is now one of several engineering vendors
  - intersect-org -> Intersect is the member-organization layer
created: 2026-05-01
updated: 2026-05-01
aliases: [IOG, IOHK, Cardano Foundation, Emurgo, founding entities]
cssclass: wiki-page
---

# Cardano Founding Organizations

Cardano launched in 2017 with three "founding entities" plus a community. In 2024–2026 the structure rebalanced into four operational orgs, with on-chain governance as the actual authority above all of them.

## Input | Output (IOG / IOHK)

- Founded by Charles Hoskinson + Jeremy Wood, 2015
- Original R&D contractor — designed Ouroboros, Plutus, Hydra, Leios
- Today: an engineering services org operating under treasury contract
- GitHub: `github.com/input-output-hk` — Daedalus, Mithril, Adrestia, cardano-playground, partner chains, Catalyst tooling, Atala/Identus
- Plays a key role in **research** (peer-reviewed papers — IACR, Crypto, etc.)

## Cardano Foundation

- Swiss not-for-profit
- Stewardship of the Cardano brand, education, certifications
- Custodian of `github.com/cardano-foundation` — CIPs repo, identity wallet, developer portal, governance tools
- Picks up Catalyst stewardship from IOG in 2026
- Organizes Cardano Summit + community programs

## Emurgo

- Japanese commercial arm
- Builds and invests in Cardano startups (Yoroi wallet, Cardano Spot, NMKR, etc.)
- Venture Hub — accelerator + capital for ecosystem builders
- USDA stablecoin work (paused), RWA tokenization pilots

## Intersect MBO

- Newest entity (2024–) — see [[intersect-mbo]]
- Custodian of core protocol repos: cardano-node, plutus, ouroboros-network, cardano-ledger
- Operationalizes governance, runs treasury machinery
- Member-organization model — anyone can join

## Where authority actually sits (2026)

```
            ┌──────────────────────────────┐
            │   On-chain governance        │
            │   (DReps + CC + SPOs)        │
            │   = the actual sovereign     │
            └──────────────┬───────────────┘
                           │ (CIP-1694 actions)
            ┌──────────────┼───────────────┐
            │              │               │
    ┌───────▼────┐ ┌───────▼─────┐ ┌──────▼───────┐
    │ Intersect  │ │ IOG         │ │ Cardano Fdn   │
    │ (operates) │ │ (engineers) │ │ (stewards)    │
    └────────────┘ └─────────────┘ └───────────────┘
                           │
                    ┌──────▼──────┐
                    │   Emurgo    │
                    │ (commercial)│
                    └─────────────┘
```

This is **explicitly** different from Ethereum (foundation + EF research + protocol clients), Solana (Foundation + Labs), or Bitcoin (no formal entity). Cardano is the most explicitly *constitutional* of major L1s.

See also: [[intersect-mbo]], [[cardano-repos]], [[cip-1694-governance]].
