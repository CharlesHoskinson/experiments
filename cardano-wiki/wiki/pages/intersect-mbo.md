---
title: Intersect MBO
slug: intersect-mbo
tags: [governance, organization, intersect, mbo]
sources: [intersect-org, cardano-forum-intersect-2026, intersect-2026-news, intersect-github]
confidence: high
provenance:
  - intersect-org -> Member-driven, distributed organization with members in 74 countries
  - intersect-2026-news -> 3% admin fee on treasury withdrawals from 2026 onward
  - cardano-forum-intersect-2026 -> Committee elections April 2026
  - intersect-github -> Custodian of cardano-node, plutus, ouroboros-* repos
created: 2026-05-01
updated: 2026-05-01
aliases: [Intersect, IntersectMBO, Cardano MBO]
cssclass: wiki-page
---

# Intersect MBO

Intersect (Intersect MBO — "member-based organization") is a not-for-profit incorporated in Wyoming that acts as the **operational hub for Cardano governance**. Established to support Voltaire-era governance, Intersect plays the role of a treasury administrator, technical steward, and member coordinator.

## What Intersect actually does

- **Custodian of core repos** — cardano-node, plutus, ouroboros-network, cardano-ledger, cardano-formal-specifications all live under `github.com/IntersectMBO`
- **Treasury operations** — runs the on-chain machinery that pays out passed governance actions
- **Hard-fork working group** — coordinates technical readiness for HFs (Plomin, Van Rossem, etc.)
- **Working groups & committees** — Technical Steering Committee (TSC), Budget Committee, Open Source Committee, Growth & Marketing Committee, Membership Committee
- **Bug bounty program** — open-source security
- **Member services** — Associates (~individual / small org tier) and Enterprise members

## Funding model (2026)

From 2026 Intersect runs on a **3% administration fee** applied to treasury withdrawals it administers. This makes the org self-funding proportional to ecosystem activity rather than via grant rounds.

## 2026 priorities

1. Execute the 2026 budget process
2. Support the **Van Rossem** hard fork
3. Constitutional Committee elections (April 2026)
4. Strengthen open-source security (bug bounties)
5. Establish **Vision 2030** as ecosystem direction

## Power structure (and tension)

- **Members vote** for committees, board of directors
- **Committees** drive day-to-day operation
- **CC + DReps + SPOs** (defined in CIP-1694) are the actual on-chain authority
- Intersect is **not** the on-chain authority — it's the organization that *executes* what governance decides

This separation matters: Intersect can't override the chain's governance, only operationalize it.

## Where it sits among the "founding" entities

| Org | Role |
|---|---|
| **Input Output (IOG / IOHK)** | Original protocol R&D and engineering, now contracted |
| **Cardano Foundation** | Stewardship, marketing, education, certifications |
| **Emurgo** | Commercial / venture arm |
| **Intersect MBO** | Member organization, governance operations |

The shift in 2024–2026: founders' roles narrow into **contracted vendors** under governance; Intersect becomes the connective tissue.

See also: [[cip-1694-governance]], [[project-catalyst]], [[plomin-hard-fork]].
