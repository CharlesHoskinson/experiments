---
title: Plutus, Aiken, Marlowe — Smart Contract Stack
slug: plutus-and-smart-contracts
tags: [smart-contracts, plutus, aiken, marlowe, tooling]
sources: [cardano-docs-plutus, cf-aiken, emurgo-plutus-marlowe, adapulse-aiken]
confidence: high
provenance:
  - cardano-docs-plutus -> Plutus is the canonical Haskell-based smart contract platform
  - cf-aiken -> Aiken is a Rust-syntax functional language compiling to UPLC
  - emurgo-plutus-marlowe -> Marlowe is a domain-specific contract language for finance
created: 2026-05-01
updated: 2026-05-01
aliases: [Plutus, Aiken, Marlowe, smart contracts on Cardano]
cssclass: wiki-page
---

# Smart Contract Stack on Cardano

All three top languages compile down to **Untyped Plutus Core (UPLC)** — Cardano's on-chain script ISA, a small lazy lambda calculus.

## Plutus (Haskell)

- The original. Validators written as Haskell, compiled via plugin to Plutus IR → UPLC.
- **Pros:** full Haskell type system, formal-verification adjacent, deeply integrated with IOG tooling.
- **Cons:** developer ergonomics are rough — builds are slow, cross-compilation is delicate, debugger story is weak.
- Plutus has gone through versions V1 → V2 (Vasil, reference inputs / inline datums) → V3 (Plomin, governance support, BLS primitives).

## Aiken (Rust-like)

- Newer, community-led. Pure functional but with familiar syntax (`fn`, `let`, `pub`).
- Compiles to UPLC; benchmarks much smaller than equivalent Plutus contracts on script size and execution units.
- Has emerged as the **default for new dApp teams** in 2024–2026.
- Tooling: `aiken check`, formatter, LSP, `aiken blueprint` (CIP-57 contract metadata).

## Marlowe (DSL)

- Domain-specific language for **financial contracts** — escrow, swaps, structured products.
- Visual editor (Marlowe Playground) lets non-developers compose contracts from a small grammar.
- Trade-off: not Turing complete, but every contract is statically analyzable for funds-at-rest, payouts, timeout behaviour.

## Helios, OpShin, Plu-ts, Scalus

- **Helios** — JavaScript-friendly contract language; popular for web devs.
- **OpShin** — Python-syntax compiler to UPLC.
- **Plu-ts** — TypeScript embedding.
- **Scalus** — Scala 3 DSL for Plutus.
- All emerged because UPLC is small enough that "front-end languages" are tractable to build.

## Off-chain side

- **Lucid / Lucid-Evolution** — TypeScript transaction builder; the de-facto standard for dApp front-ends.
- **MeshJS** — wallet-integration + tx-building toolkit.
- **PyCardano** — Python equivalent.
- **CSL (cardano-serialization-lib)** — low-level Rust library, ground truth for many wallets.

See also: [[eutxo-model]] for why off-chain construction matters so much on Cardano.
