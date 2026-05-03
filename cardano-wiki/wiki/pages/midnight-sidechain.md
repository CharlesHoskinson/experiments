---
title: Midnight — Privacy Sidechain
slug: midnight-sidechain
tags: [scaling, sidechain, privacy, midnight, zk]
sources: [iog-midnight, midnight-network]
confidence: medium
provenance:
  - iog-midnight -> Midnight is a data-protection sidechain using ZK proofs and shielded smart contracts
created: 2026-05-01
updated: 2026-05-01
aliases: [Midnight, $NIGHT]
cssclass: wiki-page
---

# Midnight — Privacy Sidechain

Midnight is a **privacy-focused sidechain partner-chain of Cardano**, using zero-knowledge proofs to allow shielded smart contracts. Its native token is **NIGHT** (with **DUST** as the gas/utility token).

## Key design choices

- **Compact runtime** — TypeScript-based DSL ("Compact") that compiles to a ZK-aware execution model
- **Selective disclosure** — contracts hold both private and public state; users prove statements without revealing inputs
- **Proof system** — uses Halo 2 / similar SNARK family (active topic in Midnight tooling)
- **Token issuance** — NIGHT distributed via "Glacier Drop" to ADA + BTC + ETH + several others, with vesting

## Relation to Cardano

- Built by IOG (originally, now spun out under Midnight Foundation)
- Settles to Cardano via Mithril-style certificates
- Cardano holders received NIGHT in the Glacier Drop snapshot (Q4 2024 / 2025)
- Bridge / token-flow between ADA and DUST is an active area of work

## Use cases

- Confidential DeFi (private order books, dark pools)
- Compliance-friendly data — contracts that prove "user passed KYC" without exposing PII
- Enterprise data sharing with verifiable provenance
- Healthcare / academic research with ZK-attested data

## Open questions

- **Adoption** — privacy chains historically struggle for liquidity (Zcash, Aleo, etc.)
- **Regulatory positioning** — privacy is a feature *and* a target
- **Bridge security** — most-failed L2 surface area
- **Composability with Cardano dApps** — separate execution model, separate tooling, not "drop-in"

See also: [[hydra-scaling]] for the broader L2 strategy and [[mithril-certificates]] for the certificate substrate.
