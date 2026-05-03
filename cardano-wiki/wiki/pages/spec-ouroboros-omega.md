---
title: Ouroboros Omega — Spec Pointer
slug: spec-ouroboros-omega
tags: [spec, omega, post-quantum, fork, zk]
sources: [omega-design-doc]
confidence: speculative
provenance:
  - omega-design-doc -> Brainstorm-derived design spec for clean-slate PQ Cardano fork
created: 2026-05-01
updated: 2026-05-01
aliases: [Omega, Cardano Omega, Ouroboros Omega]
cssclass: wiki-page
---

# Ouroboros Omega — Spec Pointer

A **clean-slate post-quantum fork** of Cardano with ZK-proved continuity to all prior eras. New chain inherits no UTXOs, no scripts, no governance state — only an **Ω-Commitment**: a single hash rooting seven sub-trees of the old chain's final state, attested by Mithril-PQ + recursive Plonky3 STARK + CIP-1694 governance.

**Full design doc:** `docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md` (relative to the cardano-wiki root)

## Locked decisions (one-line each)

1. Goal ranking: **B > C > A** (velocity > storage > wallet thinness)
2. Migration: **lazy / pull-based** ZK resurrection
3. Provable scope: **everything** (UTXOs, tokens, history, scripts, stake, governance)
4. Trust: **belt-and-braces** (Mithril-PQ + recursive STARK + CIP-1694)
5. Sunset: **D → C → A** (partner-chain → read-only → hard sunset, governance-gated)
6. Crypto: **post-quantum throughout, no exceptions**
7. Sigs: **PQ-only from day one** (no dual-sig)
8. ZK system: **Plonky3** (FRI / hash-only / recursion-friendly)
9. **Dual-hash:** selective — bundle root is a `(blake2b, sha3)` tuple; per-leaf and per-sub-tree are Blake2b-only. Decision doc: `docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md` (relative to the cardano-wiki root)

## Six claim types on the new chain

| Tx type | Resurrects |
|---|---|
| `claim_utxo` | ADA + native tokens at fork height |
| `claim_token_policy` | Native-token minting policy lineage |
| `claim_tx` | Verifiable receipt of historical tx |
| `claim_script` | Plutus validator hash (provenance only) |
| `claim_stake` | Delegation, DRep, pool history |
| `claim_governance` | Treasury, DRep ID, CC seat, gov action history |

Each carries a Plonky3 STARK proof against the Ω-Commitment + a PQ signature.

## See also

- [[ouroboros-consensus]] — what we're forking from
- [[eutxo-model]] — what's being redesigned
- [[plutus-and-smart-contracts]] — script ISA changes
- [[mithril-certificates]] — Mithril-PQ is the upgrade path
- [[hydra-scaling]] · [[leios-scaling]] — coexist with Omega's redesign
- [[cip-1694-governance]] — the ratification mechanism
- [[plomin-hard-fork]] — the era we're escaping
