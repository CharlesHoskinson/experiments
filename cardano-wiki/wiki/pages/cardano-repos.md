---
title: Key Cardano Repositories
slug: cardano-repos
tags: [repositories, github, code]
sources: [iog-github, intersectmbo-github, cardano-foundation-github]
confidence: high
provenance:
  - intersectmbo-github -> 91 repos including cardano-node, plutus, ouroboros-network
  - iog-github -> 735 repos covering wallets, sidechains, research code
created: 2026-05-01
updated: 2026-05-01
aliases: [Cardano repos, GitHub orgs]
cssclass: wiki-page
---

# Key Cardano Repositories

Cardano's code lives across **four GitHub orgs** plus the broader community. Total active code surface is enormous; below is the strategically important set.

## `github.com/IntersectMBO` (~91 repos — protocol core)

| Repo | Stars | Purpose |
|---|---|---|
| **cardano-node** | 3.2k | The reference node implementation |
| **plutus** | 1.6k | Plutus language, compiler, UPLC interpreter |
| **ouroboros-network** | 291 | Network protocols supporting Ouroboros consensus |
| **ouroboros-consensus** | — | Praos / Genesis / Leios implementations |
| **cardano-ledger** | 288 | Ledger rules + formal specifications |
| **cardano-formal-specifications** | 4 | Agda / TeX specifications |
| **cardano-addresses** | 162 | Address derivation, mnemonics |
| **hf-wg-documentation** | — | Hard-fork working group docs |
| **governance-scripts** | 7 | Shell scripts for on-chain governance |
| **administration-data** | 1 | Budget data API (Rust) |
| **evolution-sdk** | 14 | TypeScript SDK |

## `github.com/input-output-hk` (~735 repos — engineering + research)

| Repo | Purpose |
|---|---|
| **daedalus** | Reference desktop full-node wallet |
| **mithril** | Stake-based threshold multi-signatures (Rust) |
| **adrestia** | APIs / SDK for Cardano client integration |
| **cardano-playground** | Testnet cluster scaffolding |
| **cardano-parameters** | Network parameters mirror |
| **partner-chains** | Sidechain framework |
| **atala-prism** / **identus** | DID / verifiable credentials |
| **plutus-halo2-verifier-gen** | Halo2 ZK proof verifier on Cardano |
| **haskell.nix** | Haskell build infrastructure on Nix |

## `github.com/cardano-foundation`

| Repo | Purpose |
|---|---|
| **CIPs** | Cardano Improvement Proposals — the spec process |
| **developer-portal** | docs / tutorial site |
| **cf-identity-wallet** | Foundation's identity wallet |
| **cardano-token-registry** | Off-chain token metadata |
| **cardano-rosetta-java** | Rosetta API in Java |

## `github.com/cardano-scaling`

| Repo | Purpose |
|---|---|
| **hydra** | Hydra Head L2 protocol implementation |
| **leios** | Ouroboros Leios research + simulators |
| **hydra-poc** | Demos / proofs of concept |

## Community / dApp stack

| Project | Purpose |
|---|---|
| **aiken-lang/aiken** | Aiken smart contract language |
| **lucid-evolution/lucid-evolution** | TypeScript tx builder |
| **MeshJS/mesh** | dApp toolkit |
| **HarmonicLabs/plu-ts** | TypeScript Plutus embedding |
| **OpShin-Lang/opshin** | Python → UPLC compiler |
| **Emurgo/cardano-serialization-lib** | Rust serialization |
| **dcSpark/cardano-multiplatform-lib** | Successor CSL (Rust + bindings) |

## Where to start reading code

| To understand… | Read… |
|---|---|
| Block validation pipeline | `cardano-node` → `ouroboros-consensus` |
| EUTXO + scripts | `cardano-ledger` → `Conway/` (current era) |
| Plutus on-chain validation | `plutus/` → `Plutus.Core.Evaluator` |
| Off-chain tx building | `lucid-evolution` |
| Hydra Head | `cardano-scaling/hydra/hydra-node/` |
| Mithril | `input-output-hk/mithril/mithril-stm` |

See also: [[cardano-orgs]], [[plutus-and-smart-contracts]].
