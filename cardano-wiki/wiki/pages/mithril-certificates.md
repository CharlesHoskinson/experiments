---
title: Mithril — Stake-Based Threshold Multi-Signatures
slug: mithril-certificates
tags: [scaling, mithril, signatures, light-client]
sources: [mithril-paper, mithril-repo, cardano-docs-mithril]
confidence: high
provenance:
  - mithril-paper -> STM aggregates individual signatures provided cumulative stake exceeds threshold
  - cardano-docs-mithril -> Primary application is fast bootstrap of nodes from certified state
  - mithril-repo -> Active production network with certified snapshots
created: 2026-05-01
updated: 2026-05-01
aliases: [Mithril, STM, stake-threshold multisig]
cssclass: wiki-page
---

# Mithril — Stake-Based Threshold Multi-Signatures

Mithril is a stake-based threshold multi-signature (STM) scheme. Stake pool operators each sign messages individually; signatures are aggregated into a compact certificate **iff** the cumulative stake of signers exceeds a threshold (e.g., 60% of total stake).

## What it solves

- **Trustless light clients** — verify a state with one short certificate instead of replaying the chain
- **Fast node bootstrap** — sync a Cardano node from a certified snapshot in **minutes** instead of days
- **Inter-chain bridging** — give an external system a succinct, stake-attested view of Cardano's state

## How signatures work

For each message, a pseudo-random subset of stake pools is **eligible** to sign (sampled by VRF + stake weight). Each eligible signer produces an individual signature; an aggregator collects them and produces one short multi-signature.

A verifier needs only:
- The aggregated signature
- The set of signer keys
- The stake distribution

…and can confirm: "≥ X% of total stake attested to this message."

## Properties

- **Compact** — verification cost is constant in the number of signers
- **Provably secure** — paper at IACR ePrint 2021/916; published at LATINCRYPT 2024
- **Live on Cardano** — Mithril mainnet network is operational; ~80% of mainnet stake participates as signers
- **Open source** — Rust implementation in `input-output-hk/mithril`

## Where this matters for design

Anything that needs to **transport Cardano state to another system** can lean on Mithril rather than running a full node or trusting a single oracle:
- L2 bridges
- Light wallets on phones / embedded devices
- ZK rollups that want stake-weighted finality witnesses
- Cross-chain lending and identity systems

See also: [[hydra-scaling]] for off-chain compute and [[leios-scaling]] for on-chain throughput.
