---
title: CIP-1694 — On-Chain Governance
slug: cip-1694-governance
tags: [governance, voltaire, cip-1694, drep]
sources: [cip-1694, intersect-cip1694, cardano-docs-governance, lido-cc]
confidence: high
provenance:
  - cip-1694 -> Defines DReps, CC, SPOs as the three voting bodies and seven action types
  - cardano-docs-governance -> Stake-weighted voting (one-lovelace-one-vote) for SPOs and DReps
  - lido-cc -> CC votes only on constitutionality, can be replaced by no-confidence motion
created: 2026-05-01
updated: 2026-05-01
aliases: [CIP-1694, Voltaire governance, Cardano governance]
cssclass: wiki-page
---

# CIP-1694 — On-Chain Governance

CIP-1694 (named for Voltaire's birth year) is Cardano's on-chain governance specification. Activated by the **Plomin hard fork (Jan 2025)** with full DRep + treasury powers, it gives ADA holders direct power over the protocol.

## Three voting bodies

| Body | Vote weight | Role |
|---|---|---|
| **DReps** (Delegated Representatives) | One-lovelace-one-vote (delegated from ADA holders) | Vote on all 7 governance action types |
| **Constitutional Committee (CC)** | One-member-one-vote | Vote *only* on constitutionality of actions; can be replaced by no-confidence motion |
| **SPOs** (Stake Pool Operators) | One-lovelace-one-vote (their pool stake) | Vote on a subset of actions (esp. hard forks, no-confidence) |

## Seven governance action types

1. **Motion of no confidence** — replace the CC
2. **Update committee** — change CC members or quorum
3. **New constitution / guardrails** — amend the constitutional document
4. **Hard fork initiation**
5. **Protocol parameter changes** — economic, technical, governance, network
6. **Treasury withdrawals** — pay ADA from treasury
7. **Info action** — non-binding signal

Different actions require different majorities of the three bodies.

## Default voting options

- **Auto-abstain** — your stake is registered to vote but you delegate to no DRep
- **Auto-no-confidence** — special DRep that votes against everything (a "veto by default" stance)
- **Delegate to DRep** — actively delegate to a person/org

## The Cardano Constitution

A text document codifying the network's values. Currently informational/guardrail role. Constitutional Committee evaluates whether governance actions violate it. Ratified at the Cardano Constitutional Convention (Buenos Aires, Dec 2024).

## Practical implications

- **Treasury** — ~1.7B+ ADA sits under on-chain control; spent only by passing governance action
- **Param changes** — protocol params (block size, fees, k, etc.) now changeable by vote rather than IOG / Foundation fiat
- **Hard forks** — must pass governance vote; no more federated trigger
- **Foundation/IOG/Emurgo** — original "founding entities" no longer have unilateral powers post-Plomin

See also: [[intersect-mbo]] (the org running governance infrastructure), [[plomin-hard-fork]], [[voltaire-roadmap]].
