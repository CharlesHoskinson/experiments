---
title: Plomin Hard Fork (Jan 2025)
slug: plomin-hard-fork
tags: [hard-fork, governance, plomin, voltaire]
sources: [cardano-plomin-explainer, beincrypto-plomin-roadmap, messari-plomin]
confidence: high
provenance:
  - cardano-plomin-explainer -> Plomin activates full CIP-1694 governance with treasury withdrawals
  - beincrypto-plomin-roadmap -> Marks transition out of bootstrap CC into elected CC
  - messari-plomin -> Plutus V3 introduced with BLS primitives, governance support
created: 2026-05-01
updated: 2026-05-01
aliases: [Plomin, Plomin HF, Chang2]
cssclass: wiki-page
---

# Plomin Hard Fork (Jan 2025)

Plomin (formerly "Chang+1") was the hard fork that **activated full on-chain governance** under [[cip-1694-governance]]. It marked Cardano's transition into the **Voltaire era**.

## What it shipped

- **DReps go live** — ADA holders can register, delegate, vote
- **Treasury withdrawals enabled** — governance actions can move ADA out of the treasury
- **Plutus V3** — BLS12-381 primitives, governance script support, simpler ScriptContext
- **Bootstrap CC → interim CC** — elected committee replaces the bootstrap one
- **Param changes by vote** only — no more federated key trigger

## Hard-fork stack chronology

| HF | Date | Key features |
|---|---|---|
| Byron → Shelley | Jul 2020 | PoS, delegation |
| Allegra | Dec 2020 | Token locking, time-lock scripts |
| Mary | Mar 2021 | Native multi-asset |
| Alonzo | Sep 2021 | Plutus V1, smart contracts |
| Vasil | Sep 2022 | Plutus V2, reference inputs, inline datums, pipelining |
| Valentine | Feb 2023 | SECP256k1 (Bitcoin/Ethereum signature compatibility) |
| Chang | Sep 2024 | Bootstrap governance (Conway era) |
| **Plomin** | **Jan 2025** | **Full on-chain governance, Plutus V3** |
| **Van Rossem** | 2026 (planned) | Throughput / param tuning, Leios prep |

## Why it matters strategically

Pre-Plomin: protocol upgrades required coordination between IOG, Foundation, Emurgo + community signaling. Post-Plomin: **the chain itself decides**, via DReps + CC + SPO votes.

This is the moment Cardano stops being a project run by founding entities and starts being a self-governing network. Whether it actually delivers on that depends on DRep participation rates and CC effectiveness — both still being worked out in 2026.

See also: [[cip-1694-governance]], [[voltaire-roadmap]], [[intersect-mbo]].
