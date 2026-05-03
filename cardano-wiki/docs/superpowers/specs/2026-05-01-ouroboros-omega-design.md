# Ouroboros Omega — Design Spec

**Status:** Draft v0.1
**Date:** 2026-05-01
**Authors:** Charles Hoskinson (with Claude as sparring partner)
**Wiki:** [[cardano-wiki overview|/home/hoskinson/cardano-wiki/wiki/overview.md]]

---

## 1. Summary

Ouroboros Omega is a clean-slate fork of Cardano that escapes the accumulated weight of every prior era (Byron → Shelley → Allegra → Mary → Alonzo → Vasil → Chang → Plomin → Van Rossem) by **starting with zero state** and using a **post-quantum zero-knowledge framework** to prove arbitrary properties — balances, transactions, scripts, stake, governance — about the old chain on demand.

The new chain is **post-quantum from genesis**: ML-DSA signatures, hash-based KES, lattice-or-hash VRF, Mithril-PQ aggregation, and a STARK-friendly script ISA. Recovery from the pre-fork era is *lazy* and *pull-based*: users submit `claim_*` transactions carrying Plonky3 STARK proofs against a frozen genesis commitment bundle.

This document captures the design decisions, architecture, claim semantics, sunset trajectory, and risk surface for an initial implementation phase.

---

## 2. Goals — ranked

1. **B — Protocol velocity (priority #1).** Free to redesign ledger model, address format, script ISA, fee model, KES/VRF, and consensus tuning without dragging compatibility shims for any prior era.
2. **C — State / storage scalability (priority #2).** Live chain stops carrying ~7+ years of dead UTXOs, rotted scripts, dormant stake. New chain inherits *only* a commitment, not the data.
3. **A — Wallet / client thinness (priority #3).** Wallets, light clients, and tooling speak only the new chain's rules. The old chain is reachable only through external proving services.

---

## 3. Non-goals

- Backwards-compatible execution of pre-fork Plutus scripts on the new chain. Scripts must be *re-deployed* (their hashes can be re-anchored, but their semantics may need porting if the ISA changes).
- Preserving the live old chain forever. The old chain follows a governance-gated sunset trajectory.
- Bridging to non-Cardano L1s in this spec. (Out of scope; addressed in follow-up specs.)
- Maintaining classical (Ed25519/secp256k1/BLS12-381) signature support anywhere on the new chain. PQ-only from day one.

---

## 4. Locked decisions

| # | Decision | Choice | Rationale |
|---|---|---|---|
| 1 | Migration model | **Lazy / pull-based ZK resurrection** | Maximizes priority C (storage) and B (no eager-migration code paths) |
| 2 | Provable scope | **Everything**: UTXOs, native tokens, tx history, scripts, stake, governance | Maximum legitimacy continuity; richest backwards window |
| 3 | Trust stack for genesis commitment | **Belt-and-braces**: Mithril-PQ + recursive STARK proof + CIP-1694 ratification | Cryptographic + economic + constitutional legitimacy |
| 4 | Old-chain sunset | **D → C → A** phased: partner-chain → read-only → hard sunset, each phase gated by CIP-1694 | Respects optionality; ratchets community at its own pace |
| 5 | Post-quantum | **Yes, throughout — PQ-native from genesis** | Quantum threat is the long-tail risk a fork must address now or never |
| 6 | Hybrid sigs during transition | **None — PQ-only from day one** | Bridge-burning; no compat code in trust base |
| 7 | ZK proof system | **Plonky3** (FRI-based, hash-only, mature recursion) | Production-grade, PQ-secure, recursion-friendly, fits Verkle/Merkle inputs naturally |

---

## 5. Architecture overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      OLD CARDANO (frozen at H)                  │
│   Byron · Shelley · Allegra · Mary · Alonzo · Vasil ·           │
│   Chang · Plomin · Van Rossem                                   │
│                                                                 │
│   Final state at fork height H is committed into 7 sub-trees:   │
│   ├─ UTXO set          (Verkle root)                            │
│   ├─ block header chain (FRI accumulator)                       │
│   ├─ tx-id index       (Merkle root)                            │
│   ├─ token policies    (Merkle root)                            │
│   ├─ script registry   (Merkle root)                            │
│   ├─ stake state       (Merkle root)                            │
│   └─ governance state  (Merkle root)                            │
│                                                                 │
│   Bundle = root_of(7 roots) → "Ω-Commitment"                    │
└──────────────────────────┬──────────────────────────────────────┘
                           │
              ┌────────────┼────────────┬────────────┐
              ▼            ▼            ▼            ▼
        Mithril-PQ   recursive       CIP-1694    archived
        attestation  STARK proof     ratification raw data
        (≥80% stake) of Praos exec   (DRep+CC+SPO) (forever)
                           │
                           ▼
              ┌────────────────────────────┐
              │   GENESIS BLOCK of NEW     │
              │   CARDANO (Ω₀)             │
              │   - Ω-Commitment baked in  │
              │   - Empty UTXO set         │
              │   - PQ-native protocol     │
              └─────────────┬──────────────┘
                            │
                            ▼
        ┌──────────────────────────────────┐
        │   OUROBOROS OMEGA (live)         │
        │   - ML-DSA signatures            │
        │   - Lattice/hash VRF             │
        │   - Hash-based KES               │
        │   - Mithril-PQ aggregation       │
        │   - STARK-friendly script ISA    │
        │   - Native Leios pipelining      │
        │   - Account-abstracted addresses │
        │   - Encrypted mempool / PQ TLE   │
        └─────────────┬────────────────────┘
                      │
                      ▼
        ┌──────────────────────────────────┐
        │   claim_* transactions           │
        │   carry Plonky3 STARK proofs     │
        │   against the Ω-Commitment.      │
        │   Six claim types — see §8.      │
        └──────────────────────────────────┘
```

---

## 6. Cryptographic primitives — full audit

| Layer | Old Cardano | Ouroboros Omega | Status |
|---|---|---|---|
| User signatures | Ed25519 | **ML-DSA-65** (default), **SLH-DSA-256s** (cold/recovery), **Falcon-1024** (compact opt-in) | NIST-standardized |
| Hash function | Blake2b-256 | **Per-leaf and per-sub-tree:** Blake2b-256. **Bundle root:** dual-track tuple `(Blake2b-256, SHA3-256)`. Verifiers of the canonical Ω-Commitment must check both. See `docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md`. | Mature |
| VRF | Ed25519-VRF | **Hash-VRF over SHA3** (conservative pick over lattice-VRF) | New construction; designable from established hash assumptions |
| KES | Sum-comp Ed25519 | **Hash-based KES** (Merkle-tree of one-time-sig leaves over SLH-DSA-128f) | New construction |
| Stake aggregation | BLS12-381 STM (Mithril) | **Mithril-PQ**: hash-based STM over SLH-DSA leaves + VRF eligibility (existing Mithril paper structure preserved) | Active research → engineering |
| ZK proof system | n/a (or Halo2 inside dApps) | **Plonky3** (FRI / Goldilocks / KoalaBear) for all in-protocol verification | Production |
| Recursive aggregation | n/a | **Plonky3 recursion** for Ω-Commitment STARK | Production |
| Encryption | X25519 | **ML-KEM-768** | NIST-standardized |
| Address format | Bech32 of Ed25519 hash | Bech32 of ML-DSA pubkey hash; **account-abstracted** (address can specify which scheme) | New format |

**Critical property:** the old chain's classical crypto (Ed25519, BLS, secp256k1) becomes a **witness inside a PQ-secure proof system**, never a part of the new chain's trust base. A user proving "I controlled UTXO X on the old chain" produces a Plonky3 proof of knowledge of an Ed25519 secret key matching the public key in UTXO X. The proof itself is hash-only PQ-secure.

---

## 7. Genesis commitment bundle (Ω-Commitment)

The Ω-Commitment is the only thing the new chain knows about the old chain. It is a single 32-byte hash that roots seven independent commitments:

1. **UTXO set tree** — every unspent output at fork height H, keyed by (tx_id, index), valued by (address, value, datum_hash). Verkle tree for short proofs.
2. **Block header accumulator** — FRI-friendly accumulator over all old-chain block headers from genesis to H. Lets users prove "block at slot S had hash B."
3. **Transaction index** — Merkle tree of all tx hashes ever included on the old chain, mapping tx_id → (slot, block_hash, tx_position).
4. **Native token policy registry** — Merkle tree of all minting policy hashes that ever issued a token on the old chain, with their first-issuance slot and total supply at H.
5. **Script registry** — Merkle tree of all unique Plutus validator hashes deployed via reference scripts or used as outputs, with deployment slot.
6. **Stake state** — Merkle tree of (delegation, pool registration, DRep registration) tuples at H.
7. **Governance state** — Merkle tree of (treasury balance, DRep IDs, CC member positions, past gov action history, ratified proposals) at H.

All seven commitments are computed by a deterministic procedure executed by anyone running an old-chain node at height H. The procedure spec is part of this document's follow-up; reference implementation lives in `omega-prover` (new repo).

**Dual-track at the bundle layer:** Per the dual-hash decision (`docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md`), each sub-tree root is computed twice — once with Blake2b-256 and once with SHA3-256 — using the `hash::dual_hash` primitive. The Ω-Commitment is the **tuple** `(root_of(7 blake2b sub-tree roots), root_of(7 sha3 sub-tree roots))`. Per-sub-tree leaves and per-sub-tree Merkle roots remain Blake2b-only; only the bundle layer is dual-track.

---

## 8. Trust stack — belt-and-braces

The Ω-Commitment is canonicalized by **three independent attestations**, all of which must succeed:

### 8.1 Mithril-PQ stake attestation
Old-chain SPOs sign the Ω-Commitment using **dual signatures** (BLS12-381 STM + hash-based STM over SLH-DSA). Pre-fork software upgrade requires SPOs to register a PQ Mithril key alongside their BLS key. At fork height, only the PQ signature is canonical; BLS is retained as belt-and-braces only. A threshold of ≥80% of total stake must sign.

### 8.2 Recursive STARK proof of Ouroboros execution
A Plonky3 recursive proof attests: *"I started from the Cardano genesis state, executed Ouroboros Praos / Genesis from slot 0 through slot S_H, and the resulting state matches the seven sub-trees committed in the Ω-Commitment."*

This is generated by a **proving cluster** funded as part of the fork program (estimated 6–12 months of GPU-time, parallelizable, decreasing as Plonky3 tooling improves). Proof is published with the Ω-Commitment.

### 8.3 CIP-1694 governance ratification
A governance action of type **"Hard Fork Initiation"** is submitted with the Ω-Commitment as its payload. Standard Plomin-era thresholds: DReps + CC + SPOs all vote. Ratification is the *political* legitimization — the chain ratifies its own genesis.

**All three must pass.** No two are sufficient.

---

## 9. Claim transactions — six types

The new chain's ledger introduces a single new transaction class: `claim`. Each `claim` carries a STARK proof against the Ω-Commitment plus a PQ signature binding the claim to a new-chain account.

**Note on dual-track scope:** Claim circuits verify against the **Blake2b-half** of the Ω-Commitment tuple. The SHA3-half is consumed only at the bundle attestation layer (§8); per-claim circuit cost remains single-track. See `docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md`.

### 9.1 `claim_utxo`
Resurrect ADA + native tokens.
- **Witness:** Plonky3 proof of `(utxo_in_set ∧ knowledge_of_old_secret_key) ∧ (nullifier = H(utxo_id, old_sk))`
- **Effect:** mints corresponding ADA + tokens into a new-chain output bound to the user's PQ pubkey. Adds nullifier to spent-set.
- **Replay protection:** nullifier set on new chain.

### 9.2 `claim_token_policy`
Rebind a native-token minting policy on the new chain.
- **Witness:** Plonky3 proof of `(policy_in_registry ∧ knowledge_of_old_policy_keys)`
- **Effect:** new chain registers an alias `old_policy_hash → new_policy_hash` so legacy tokens proven under `claim_utxo` show consistent identity.

### 9.3 `claim_tx`
Emit a verifiable receipt that historical tx H existed at slot S.
- **Witness:** Plonky3 proof against transaction index and block header accumulator.
- **Effect:** new chain records an attestation that any future verifier can cite. No state change beyond the attestation event log.
- **Use cases:** legal compliance, RWA provenance, audit trails, dispute resolution.

### 9.4 `claim_script`
Re-anchor a Plutus validator hash.
- **Witness:** Plonky3 proof that script hash V was in the script registry at H.
- **Effect:** new chain records "script V was canonical on old chain at H" in a provenance registry. Does *not* re-deploy or re-execute the script. If the new ISA differs, the developer must port the validator's semantics and deploy a new script; the `claim_script` record then provides a verifiable lineage link from the old hash to the new one. Pure provenance/identity continuity — e.g., "this is the same multisig wallet."

### 9.5 `claim_stake`
Port over delegation, DRep registration, voting history.
- **Witness:** Plonky3 proof against stake state tree.
- **Effect:** new chain creates the corresponding new-chain stake credential, DRep, or pool registration with proven history. Required for legitimacy continuity in governance.

### 9.6 `claim_governance`
Reattach treasury accounts, DRep IDs, CC member positions, past governance action records.
- **Witness:** Plonky3 proof against governance state tree.
- **Effect:** new chain inherits the political/economic state — treasury balance materializes, current CC members retain their seats, in-flight governance actions resume on new chain.

---

## 10. New-chain redesign axes (use the velocity)

Because protocol velocity is priority #1 and we have no compat baggage, the fork ships *new* primitives that would be impossible to retrofit:

1. **PQ-native account abstraction.** Each account specifies which signature scheme it uses (ML-DSA / SLH-DSA / Falcon / multi-sig hybrid) at the ledger level. No fixed scheme assumption.
2. **STARK-friendly script ISA.** Replace UPLC with a VM whose execution traces are cheap to STARK (Cairo-style or Plonky3-friendly RISC subset). Every on-chain action becomes provably-correct cheaply — useful for L2s, light clients, and recursive composition.
3. **Native Leios from genesis.** Pipelined IB/EB/RB block production from block 0. No "Leios upgrade" needed later.
4. **Mithril-PQ as a chain primitive.** Every block produces a stake-attested certificate consumable by any verifier (light client, L2, partner-chain, AI agent). Cardano becomes the only PQ chain with cheap stake-attested light clients on day one.
5. **Encrypted mempool with PQ time-lock encryption.** MEV-resistant ordering: transactions are encrypted with lattice-based time-lock; decryption deterministic at block-inclusion time. No frontrunning.
6. **Reference inputs by default + script-bytecode addressed by hash globally.** No re-embedding scripts in transactions.
7. **Stake credentials are first-class objects.** Delegation, DRep registration, governance participation all unified under one credential type — fixes the present bifurcation between payment and stake addresses.
8. **Native rollup framework.** Built-in primitives for STARK rollups settling to L1 — no dApp bootstrapping needed.

---

## 11. Old-chain sunset trajectory

Three governance-gated phases. Each transition is a CIP-1694 hard-fork action.

### Phase 1: Partner-chain (immediately after fork, indefinite)
Old chain becomes a Cardano partner-chain of Omega. Its blocks produce Mithril certificates settling to Omega. SPOs continue to operate; users who have not claimed can keep transacting on the old chain. Omega ignores the live old-chain state — it only references the frozen Ω-Commitment for claim validation.

### Phase 2: Read-only (governance-gated, expected ~2-3 years post-fork)
A CIP-1694 action triggers old-chain transition to read-only mode. Old-chain SPOs stop accepting transactions; nodes serve historical queries and Mithril certificates of the now-frozen tip. ZK provers continue to read from it. Block production halts.

### Phase 3: Hard sunset (governance-gated, expected ~5+ years post-fork)
A CIP-1694 action terminates old-chain operations entirely. Foundation/Intersect/IOG cease maintaining nodes. Anyone wanting historical access runs their own archive node from data published to IPFS / Arweave / Filecoin. The Ω-Commitment + ZK proofs remain the only canonical reference.

**Key invariant:** Omega's protocol does *not* depend on which phase the old chain is in. It only ever reads the frozen Ω-Commitment.

---

## 12. Risk surface

| Risk | Severity | Mitigation |
|---|---|---|
| Mithril-PQ research is immature | 🔴 High | Run BLS+PQ Mithril in parallel for ≥1 year of mainnet shadow-test before fork. Publish formal proofs of security reduction. |
| Hash-VRF correctness / efficiency | 🟡 Med | Conservative pick over lattice-VRF; needs new security analysis published before fork. |
| Recursive STARK over years of old-chain history is expensive | 🟡 Med | Funded proving cluster; parallelize per-epoch; iterate Plonky3 perf. Estimated $5-15M one-time proving cost. |
| Hardware wallets need ML-DSA/SLH-DSA support | 🟡 Med | 2-year coordination with Ledger / Trezor / Tangem pre-fork. Provide reference HW wallet firmware. |
| Pre-fork quantum adversary (worst case) | 🟡 Med | Tight claim deadlines for UTXOs whose Ed25519 keys haven't moved in N years; force-resurrect-or-lock under CIP-1694 vote. |
| Address format break confuses users | 🟢 Low | New wallets ONLY speak new format. Old addresses appear only inside ZK claims, never user-facing. |
| Political coordination crisis (people refuse to fork) | 🔴 High | CIP-1694 governance is the legitimizing path. Belt-and-braces commitment provides cryptographic & economic legitimacy alongside political. Sunset is graceful (D→C→A). |
| Day-one TVL / liquidity collapse during migration | 🟡 Med | Phase 1 (partner-chain coexistence) means liquidity migrates voluntarily over time. Stablecoins can choose to mint on Omega via `claim_token_policy`. |
| ZK prover availability for poor users | 🟡 Med | Ship reference open-source prover; subsidize public proving service for small claims (<10 ADA) from treasury. |
| Plonky3 vulnerability discovered post-fork | 🟡 Med | Verifier code is a governance-upgradable slot; soundness vulns can be hotfixed via CIP-1694 vote within an epoch. |

---

## 13. Open questions

1. **Hash-VRF specifics** — exact construction, security parameter, performance budget per slot. Needs a working group.
2. **Proving cluster operations** — funded by treasury action? IOG-contracted? distributed across community provers?
3. **Verkle vs. Merkle for UTXO tree** — Verkle gives smaller proofs but needs PQ-secure polynomial commitments; Merkle is conservative and proven. Likely pick Merkle for safety, optimize later.
4. **Script ISA selection** — Cairo, Plonky3-RISC, or a custom VM? Deserves its own design doc.
5. **Stablecoin migration coordination** — USDM, Djed, USDC bridges all need migration plans before fork.
6. **dApp coordination window** — how long do major dApps (Minswap, Liqwid, Sundae, etc.) get to plan re-deployment?
7. **Treasury during transition** — is the treasury value committed in Ω-Commitment and resurrected via `claim_governance`, or does it require its own special sub-tree?
8. **Block-explorer continuity** — explorers need to query both old (read via partner-chain or archive) and new chain seamlessly. Standard interface?
9. **Pre-fork test fork** — at what testnet scale do we validate Mithril-PQ + recursive STARK? Existing testnets, or a dedicated "Ω-preview" network?
10. **Quantum-adversary scenario planning** — what's the response if a credible quantum compromise of Ed25519 is announced *during* Phase 1?

---

## 14. Implementation phasing (high-level)

Detailed plan is the next artifact (writing-plans skill). This is just a sketch:

| Phase | Duration | Gates |
|---|---|---|
| **0. Research & specification** | 12-18 months | This doc → CIP draft → peer-reviewed papers on Mithril-PQ, hash-VRF, Ω-Commitment construction |
| **1. Reference implementations** | 18-24 months | omega-prover, omega-node alpha, mithril-pq alpha, plonky3 verifier integration |
| **2. Public testnet ("Ω-preview")** | 6-12 months | Full claim flow exercised; recursive STARK over a synthetic old-chain history |
| **3. Mithril-PQ shadow on mainnet** | 12 months | BLS+PQ dual mainnet for confidence |
| **4. Ω-Commitment generation** | 1 month | Final Cardano block at H frozen; commitment bundle generated; recursive STARK proof produced |
| **5. CIP-1694 hard-fork ratification** | 1-3 months | DReps + CC + SPOs vote |
| **6. Mainnet activation** | 1 day | Old chain enters Phase 1 partner-chain mode; new chain produces block 0 |
| **7. Steady state** | indefinite | Claim transactions roll in; governance-gated sunset begins later |

**Total estimated runway from spec freeze to mainnet:** **~5 years**, parallelizable in places.

---

## 15. Naming, branding, framing

- **Working name:** Ouroboros Omega
- **Marketing arc:** "The first quantum-safe genesis." Cardano sheds its history not because it has to, but because it *can* — using the same peer-reviewed cryptography rigor that defined every prior era.
- **Constitutional framing:** This is **the most important governance action in Cardano's history** and only possible because of CIP-1694. The chain ratifies its own rebirth.
- **Risk framing for community:** This is not a "ship of Theseus" — old Cardano continues to exist in Phase 1 / Phase 2 / archives. No one is forced to claim. ZK proofs preserve every property forever.

---

## 16. References

- Cardano wiki: `/home/hoskinson/cardano-wiki/wiki/`
- Existing Cardano repos: `[[cardano-repos]]`
- Mithril paper: <https://eprint.iacr.org/2021/916>
- Plonky3: <https://github.com/Plonky3/Plonky3>
- NIST PQ standards: ML-DSA (FIPS 204), ML-KEM (FIPS 203), SLH-DSA (FIPS 205)
- CIP-1694: <https://cips.cardano.org/cip/CIP-1694>
- Ouroboros Leios: <https://leios.cardano-scaling.org/>

---

## 17. Decision log (this brainstorm session)

- 2026-05-01: B>C>A goal ranking confirmed
- 2026-05-01: Lazy/pull-based migration model picked
- 2026-05-01: Everything-provable scope picked
- 2026-05-01: Belt-and-braces trust stack picked
- 2026-05-01: D→C→A phased sunset picked
- 2026-05-01: Post-quantum throughout, no exceptions
- 2026-05-01: PQ-only signatures from day one (no dual-sig transition)
- 2026-05-01: Plonky3 selected as in-protocol ZK system
- 2026-05-01: Dual-hash question resolved as **Option 3 (selective dual-track)** — bundle root is the tuple `(blake2b_bundle, sha3_bundle)`; per-leaf and per-sub-tree remain Blake2b-only. See `docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md`.
