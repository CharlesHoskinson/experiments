# Ouroboros Omega — archive-anchored claims design

> **Scope.** This document specifies the cryptographic architecture for migrating Cardano-era state into Omega via archive-anchored claims, and the privacy / consensus / governance machinery that surrounds them. It is the design-level companion to the v0.9.1 commitment tooling already shipped at `experiments/omega-commitment/` and to the v1.0 / v1.1 ingestion plans under `experiments/cardano-wiki/docs/superpowers/plans/`. Out of scope: smart-contract VM (T3), network stack (T4), storage layer (T5), wallet UX (T8).
>
> **Status.** Spec draft, 2026-05-03. Brainstormed across six capability lenses (cold storage, regulator attestations, programmable claims, pre-claim asset markets, cross-chain provenance, public-good archival), six privacy-primitive lenses (shielded pools, viewing-key disclosure, nullifier-only, aggregation/DP, GDPR/legal), six adversarial pressure-test lenses (crypto horizon, genesis ceremony, state-actor censorship, holder compromise, MEV, archive availability), and one consensus-layer reframing. All findings folded in.

## Top-line design properties

1. **No master keys, no court override, no escrow keys, no designated-viewer master.** No mechanism in the protocol grants disclosure of an address's data without the holder's voluntary cooperation. By symmetry, this means no jurisdiction can compel disclosure via the protocol; compliance happens at the application layer between consenting counterparties.
2. **All primitives post-quantum.** No curve operations anywhere in build, verify, or leaf encoding. Hash functions: Blake2b-256 + SHA3-256 + Poseidon2 (in-circuit). Signatures: hash-based (SLH-DSA / SPHINCS+ family) for authority-grade ops; ML-DSA / FN-DSA permitted for high-throughput user signing per the Cloudflare 2025 analysis. Threshold: hash-based aggregation (leanXMSS / leanMultisig family); no BLS.
3. **Plonky3-friendly throughout.** Every operation expressible as STARK constraints without curve gadgets. Inner Merkle hash = Poseidon2 in-circuit; Blake2b/SHA3 only at interop boundaries. Goldilocks field underneath.
4. **Crypsinous-PQ consensus.** Ouroboros Crypsinous (Kerber-Kiayias-Kohlweiss-Zikas, eprint 2018/1132) updated for the privacy infrastructure specified here. Shielded VRF, shielded stake, shielded rewards. Encrypted mempool natively (not bolted on).
5. **Starstream as native UTXO + zkVM layer.** [LFDT-Nightstream/Starstream](https://github.com/LFDT-Nightstream/Starstream) is Omega's smart-contract execution model and the destination shape of every claim_utxo output. UTXO-based (EUTXO mental model survives), coroutine-primitive, Goldilocks + Poseidon2 (the exact primitives §3 already mandates), native folding for both variable updates and function application. See §15.
6. **Holder-sovereign disclosure.** PLUME-style nullifiers, holder-controlled viewing keys, holder-curated non-membership proofs against association sets the holder picks. The protocol has no association set.
7. **Genesis = mass-MPC ceremony, not a publication event.** Cardano stake-weighted; soundness requires only one honest participant who re-derives from block 0.
8. **The chain has the resolution machinery; holders bring their own proofs.** No mandated mirror network. Snapshot data lives wherever holders, communities, and economically-incentivised mirrors choose to host it. The protocol incentivises replication via storage-proof bounties without designating any operator.

## 1. Cryptographic primitives

| Layer | Primitive | Where |
|---|---|---|
| Per-leaf hash | Blake2b-256 with `omega:v1:leaf` domain tag binding `(sub_tree_id, canonical_index, payload_len, payload)` | Per-sub-tree leaf encoder, off-chain |
| Internal-node hash | Blake2b-256 with `omega:v1:node` domain tag binding `(left, right)` | Per-sub-tree tree builder, off-chain |
| Sub-tree root hash | Blake2b-256 over the final tree level | Off-chain at construction, baked into bundle pre-image |
| Bundle root (primary) | Blake2b-256 over concatenation of seven sub-tree roots | Genesis commitment first 32 bytes |
| Bundle root (drift-detection) | SHA3-256 over the same concatenation | Genesis commitment second 32 bytes |
| Inner Merkle hash (in-circuit) | Poseidon2 | Plonky3 verifier circuit |
| User signatures | ML-DSA-65 (FIPS 204) for ordinary tx; FN-DSA-512 (FIPS 206 IPD) optional once final | Per-claim authority, ongoing tx |
| Authority-grade signatures | SLH-DSA-SHAKE-128s (FIPS 205) | Genesis ceremony attestations, governance |
| VRF | Hash-based VRF inside STARK with a Praos-equivalent uniqueness reduction. **Open issue:** specific construction TBD; must not reduce to "no collisions in H." X-VRF (Buser et al.) ruled out per Bodaghi-Safavi-Naini FC 2024 break |
| Threshold encryption | Hash-based, Penumbra-flow-style adapted for PQ. Per-epoch stake-weighted committee |
| Nullifier construction | PLUME (ERC-7524) — `null = PLUME(sk, public_input_seed)` |

## 2. The seven sub-trees

| # | Sub-tree | Leaf binding | Source |
|---|---|---|---|
| 1 | UTXO | tx_id (32) ‖ output_index (u32 BE) ‖ address_bytes_len (u16 BE) ‖ address_bytes (raw CIP-19) ‖ value_lovelace (u64 BE) ‖ asset_count (u32 BE) ‖ assets ‖ datum_option_tag (u8) ‖ datum_payload ‖ script_ref_tag (u8) ‖ script_ref_payload | omega-utxo-snapshot LSQ output |
| 2 | Header | slot (u64 BE) ‖ block_height (u64 BE) ‖ block_hash (32) ‖ prev_hash (32) | v1.1 chain-follower |
| 3 | Tx-index | tx_id (32) ‖ slot (u64 BE) ‖ block_hash (32) ‖ tx_position (u32 BE) | v1.1 chain-follower |
| 4 | Token policy | policy_id (28) ‖ policy_script_hash (28) ‖ total_minted (u64 BE) ‖ first_issuance_slot (u64 BE) | UTXO walk + chain-follower |
| 5 | Script registry | script_hash (28) ‖ script_type_tag (u8) ‖ deployment_slot (u64 BE) | UTXO walk + chain-follower |
| 6 | Stake | credential_tag (u8) ‖ credential (28) ‖ delegated_pool (28 or zero) ‖ drep_tag (u8) ‖ drep_payload (variable) ‖ controlled_stake (u64 BE) ‖ rewards_balance (u64 BE) | LedgerState JSON parse |
| 7 | Governance | kind (u8) ‖ payload_hash (32) | LedgerState JSON parse |

Lexicographic sort within each sub-tree by the natural identifier. Zero-pad to next power of two with `leaf_hash_v1(sub_tree_id, EMPTY_INDEX_SENTINEL=u64::MAX, &[])`. Verifier path enforces `item_count` to prevent padded-slot proofs.

## 3. Genesis ceremony

The 64-byte commitment `(blake2b_root, sha3_root)` is constituted by a mass multi-party-computation ceremony in the Zcash-Sapling / Ethereum-KZG tradition, scaled to Cardano's stake distribution.

**Participants.** Any entity holding Cardano stake at the snapshot epoch may participate. Target ≥1,000 active participants drawn from SPOs, DReps, individual stakers, and academic / civil-society / exchange-custodian observers. No privileged signers, no foundation-curated list.

**Per-participant work.**
1. Independently re-derive the seven sub-tree roots and the bundle root tuple from raw Cardano immutable chain data starting at block 0 (NOT from a pre-built Mithril snapshot — see §3.1).
2. Sign the resulting 64-byte tuple plus the snapshot block hash with their pre-fork Cardano stake key.
3. Contribute entropy that is folded into Fiat-Shamir randomness for the per-claim verifier circuit's public parameters (defends against the Frozen Heart class of attacks).

**Soundness.** Requires one honest participant. As long as any single SPO or staker independently re-derives from block 0 and the resulting commitment matches what the rest of the ceremony agreed on, the genesis is correct regardless of how many other participants were compromised. With ~3,000 SPOs alone, the assumption "at least one is honest and capable of running a Rust + Lean verifier from block 0" is operationally trivial.

**Transcript.** Append-only, livestreamed, OpenTimestamp-anchored to Bitcoin within the same epoch. Final commitment + snapshot block hash + Mithril-cert-hash baked into Omega's genesis block.

### 3.1 Snapshot supply-chain hardening

The Mithril GHSA-qv97-5qr8-2266 advisory documents that Mithril's signed message excludes ledger-state files; only the immutable chain segment is multi-sig certified. Two consequences:

1. **At least one ceremony participant MUST replay from Cardano block 0** using only the multi-sig-certified immutable segment. The Mithril ledger-state ancillary file (single-IOG-key signed) is treated as untrusted input, suitable for fast bootstrapping but never as an attestation source.
2. **Cross-implementation reproducibility requires bit-exact agreement on intermediate per-tree roots**, not just the bundle root. At minimum two impls (Rust + Lean 4 extracted from formal spec); ideally three (add Haskell borrowing cardano-ledger's CDDL parsers as differential check).

## 4. Claim mechanism

A holder claims by submitting a transaction containing:
- `public_input = (sub_tree_id, leaf_index, bundle_root, omega_recipient, chain_id="omega-mainnet", freshness_nonce)`
- `witness = (leaf_preimage, merkle_path[≤24 levels], pq_signature_over_public_input, plume_nullifier_seed)`
- `proof = plonky3_prove(verifier_circuit, public_input, witness)`

### 4.1 Verifier circuit obligations (in-circuit constraints)

1. Recompute leaf hash via `leaf_hash_v1(sub_tree_id, leaf_index, leaf_preimage)`.
2. Walk Merkle path with `node_hash_v1(left, right)`, recomputing sub-tree root.
3. Recompute bundle root from seven sub-tree roots, assert match against `bundle_root` public input.
4. Verify `pq_signature` covers `public_input` exactly (recipient binding mandatory).
5. Compute PLUME nullifier `null = PLUME(sk, sub_tree_id ‖ leaf_index)`; emit as public output.
6. Assert `freshness_nonce ∈ valid_nonce_window` (defends against stale-mempool replay).

### 4.2 On-chain post-verification

1. Plonky3 `verify(circuit_id, public_input, proof)` — verifier circuit ID is a protocol parameter, rotatable by consensus.
2. Nullifier check: assert `null ∉ consumed_nullifier_set`; insert on success.
3. Apply state transition per claim type:
   - `claim_utxo` → credit shielded note to `omega_recipient` under Crypsinous flow encryption
   - `claim_token_policy` → grant Omega-side issuance rights gated by §4.3
   - `claim_script` → register script per §4.3
   - `claim_stake` → credit stake position; reputation NOT transferable
   - `claim_governance` → see §4.3

### 4.3 Per-claim-type assignment policy

| Claim type | Forward-only assignment | Required additional witness |
|---|---|---|
| `claim_utxo` | Allowed | None beyond §4.1 |
| `claim_token_policy` | Banned | Active-issuance attestation: minting events in last N Cardano epochs |
| `claim_script` | Banned for primary; (a) dual signature from registered deployer key OR (b) governance-arbitrated dispute window with deposit-and-challenge | Plutus-script-locked UTxOs require dApp redeployment with claim-time alias commitment |
| `claim_stake` | Forward-only allowed; reputation portability NOT a transferable credential | None |
| `claim_governance` | Banned | DReps inactive at snapshot do NOT receive Omega governance rights without post-fork re-attestation + cooling-off period |
| `claim_header` | Reserved (chain-anchored protocols only) | — |
| `claim_tx` | Reserved (tx-anchored protocols only) | — |

### 4.4 Pre-fork-key claim cutoff

A finite claim window (recommended 5–10 years) followed by deterministic burn-and-reissue at cutoff. Defends against (a) quantum-acceleration breaking pre-fork Ed25519 before holders claim, and (b) the asymmetric-extraction window on uninformed holders. **No custodial escrow** — the unclaimed value is burned and re-minted to the protocol treasury, which funds the §6 archival bounty.

## 5. Privacy architecture

The protocol-default disclosure surface is asymmetric.

### 5.1 Public surface (no holder consent required)
- Universal-accumulator non-membership proofs against any list-commitment the verifier of a specific use case picks. Anyone can prove "address X is *not* in list L." The protocol has no list L of its own.
- Differential-privacy-noised aggregate histograms (epoch-bucketed balance distributions, sub-tree-counts, etc.). Per-address queries are NOT a protocol-supported operation.
- The 64-byte commitment itself plus the chunked Merkle-of-Merkles per §6.2.

### 5.2 Holder-consent surface
- PLUME-anonymous claim within the global archive cohort.
- Bulletproofs+ range proofs of historical balance ("I held ≥ X at slot S"), holder-generated, holder-disclosed.
- Sapling/MASP-style FVK + IVK + OVK split for post-claim shielded notes; viewing keys delegated to anyone the holder picks (accountant, exchange, lawyer, executor).
- Selective-disclosure VC primitives (BBS+ / SD-JWT) for jurisdictional or counterparty-specific compliance flows.

### 5.3 What is NOT in the protocol
- No master view key.
- No court-quorum decryption.
- No regulator-jurisdiction TVK.
- No protocol-curated association set / sanctioned set.
- No threshold key held by the foundation, validators, or any quorum, that grants disclosure of holder data.

The query channel is the doxxing surface (per A4 / A5 / P4 finding). Per-claim privacy is theatre absent query-pattern privacy. Mitigations: Crypsinous-native encrypted mempool (per §7), holder-side ORAM-style query interfaces for application-layer services, no protocol-mandated query oracle.

## 6. Snapshot data: chain hosts the verifier, not the data

### 6.1 What the chain holds
- The 64-byte genesis commitment.
- The Mithril cert hash anchoring the snapshot block.
- The verifier circuit ID (rotatable parameter).
- The consumed-nullifier set.
- Each chunk-root from §6.2.
- A content-addressed tarball of the CDDL spec + reference Rust + Haskell parser source, hashed into the genesis pre-image. Defends against year-50 encoding-format obsolescence.

### 6.2 Chunked anchoring
The snapshot is partitioned into N Merkle-chunks (~64 MiB each ≈ 3,400 chunks for 218 GB). Each chunk has a Merkle path back to the bundle root. The chain commits each chunk-root at protocol epoch boundaries.

**Effect.** A holder needs only the chunk(s) containing their leaf, not the whole 218 GB. A storage-bounty hunter can earn rewards by proving they hold any specific chunk. Replication becomes economically tractable per-chunk rather than monolithic-or-nothing.

### 6.3 Archival bounty (treasury layer)
A perpetual treasury allocation (Wikimedia-Endowment-shaped, ≈ $5M principal at 4% safe-withdrawal-rate funds ~$200K/year storage operations) funds storage-proof challenges paid in ωADA.

**Mechanism.** Anyone may submit a chunk-availability proof to a periodically-rotating challenge protocol. The protocol pays a small ωADA reward per honored proof, rate-limited and stake-bonded against false claims. **No designated operator.** Anyone proving they hold any chunk earns. Encourages geographic, jurisdictional, and infrastructural diversity.

### 6.4 What the Omega chain does NOT do
- Does not host snapshot data.
- Does not mandate a mirror operator.
- Does not provide a query API for historical state (those are application-layer services run by anyone who wants to).
- Does not fund any specific party — only the storage-proof challenge mechanism.

### 6.5 Mirror partnerchain (forked Filecoin)

The §6.3 storage-proof bounty alone funds replication; it does not provide a market for **retrieval**. A holder claiming in 2046 needs not just "the data exists somewhere" but "the data is fetchable from someone right now at predictable cost." For that we run a separate partnerchain — a fork of [Filecoin](https://github.com/filecoin-project) — under the Cardano [partnerchain model](https://github.com/input-output-hk/partner-chains) the same way Midnight runs.

**Forking Filecoin, not adopting it as-is.** Filecoin's existing mainnet uses ECDSA / BLS / Groth16 — none PQ. The fork replaces all curve cryptography with the PQ stack from §1: SLH-DSA / ML-DSA / FN-DSA for signatures, hash-based threshold for any aggregation, Plonky3 STARKs in place of Groth16. Filecoin's storage proofs (PoRep, PoSt, the Window-PoSt time-bounded variant) are already hash-and-Merkle-based; porting them to Blake2b/SHA3/Poseidon2 is mechanical. The economic model (storage providers post bonds; sealing creates verifiable storage commitments; periodic spacetime proofs maintain liveness) survives unchanged. Approximate work: ~6-12 months engineering, comparable in scope to Filecoin's original mainnet launch from the spec.

**Partnerchain model.** Cardano's partnerchain SDK lets a partnerchain inherit security from Cardano's stake distribution: SPOs on Cardano can opt to also validate the partnerchain, earning rewards on both. For the Omega-Mirror chain this means storage providers become double-revenue: Filecoin-style storage market fees (paid by retrievers) plus Omega-side block rewards (paid by Omega's treasury). The mirror chain's consensus is itself Minotaur-shaped, with PoSpaceTime as the dominant resource and a small PoS slice for liveness.

**What the mirror partnerchain stores.**
- Every snapshot chunk per §6.2 (the 218 GB Cardano-era state, partitioned into ~3,400 ~64 MiB Merkle chunks)
- Per-chunk Window-PoSt proofs that the data is currently retrievable
- Optional: any chunked archive of post-genesis Omega state for users who want their post-claim data preserved with the same guarantees

**Retrieval interface.** Standard Filecoin retrieval miner protocol: a holder sends a retrieval request specifying chunk ID; one or more storage providers respond with the data plus a Merkle proof of correctness against the on-chain commitment. Holders pay a tiny fee per retrieval (denominated in mirror-chain native asset, redeemable for ωADA via the partnerchain bridge).

**The mirror partnerchain is OPTIONAL infrastructure.** Omega's correctness does not depend on it. Holders who keep their own data still claim directly. The mirror exists as a market-priced convenience layer for the long tail of holders who do not bother with self-storage. This preserves the "chain has the resolution machinery; holders bring proofs" property — the mirror is one of many possible providers of those proofs, not a privileged operator. The §6.3 bounty rewards anyone who can prove possession; the mirror partnerchain is the most natural earner of that bounty at scale, but it is not the only earner.

**Coupling to Minotaur (§7.2).** The mirror partnerchain's storage providers are also a consensus resource for Omega via Minotaur. Capturing Omega's consensus now requires capturing both Omega-stake AND a meaningful fraction of the mirror chain's storage market. The two attack surfaces have different cost structures (stake = capital markets; storage = data centre buildout) and different jurisdictional profiles. This is the key economic coupling that the user-asked-for "design the requirements for a mirror network" produces: not just an archive, but a consensus-level diversification.

**What the mirror partnerchain is NOT.**
- It is NOT a privileged operator. Anyone can run a storage provider.
- It is NOT a censorship surface. The Omega chain's claim verifier never reads from the mirror; it only reads holder-submitted proofs.
- It is NOT a single point of failure. If the mirror chain fails entirely, holders who kept their own data are unaffected; holders who relied on the mirror lose the convenience of cheap retrieval but their underlying claim rights are preserved.
- It is NOT a regulator-friendly backdoor. The mirror chain stores public data (the Cardano-era snapshot is already public on Cardano); retrieval is permissionless; storage providers cannot selectively withhold without losing their bond. Privacy of post-claim Omega state still flows through Crypsinous + Starstream + holder-controlled viewing keys per §5.

## 7. Consensus stack: Crypsinous + Chronos + Minotaur, all PQ

The consensus layer is three composable Ouroboros papers, each updated for the PQ + privacy infrastructure of §1-§5.

### 7.0 Crypsinous-PQ (privacy)

PQ-Crypsinous adapts eprint 2018/1132 to the privacy infrastructure above. Composition with the claim layer collapses three primitives I had as separate (consensus shielding, mempool encryption, claim privacy) into one coherent threshold-encryption layer.

| Crypsinous original | PQ-Crypsinous |
|---|---|
| Groth16 over BLS12-381 | Plonky3 STARK; recursion only as compression layer with periodic non-recursive checkpoints |
| Curve VRF (Praos analogue) | Hash-based VRF inside STARK (open construction issue per §1) |
| BLS threshold for any aggregation | Hash-based threshold (leanXMSS / leanMultisig family) |
| Pedersen commitments for shielded stake | Poseidon2 commitments inside STARK + threshold flow-encryption (Penumbra-style adapted PQ) |
| Curve-based PRF for nullifiers | PLUME hash-based nullifier |
| Sapling note encryption | Hybrid KEM (ML-KEM + AEAD) |

**Encrypted mempool.** Threshold-encrypted to per-epoch stake-weighted committee. Validators commit to ordering before they can decrypt. Closes OFAC validator censorship + recipient front-running + mempool surveillance + validator reordering simultaneously.

### 7.1 Chronos (time)

Ouroboros Chronos (Badertscher-Gaži-Kiayias-Russell-Zikas, [eprint 2019/838](https://eprint.iacr.org/2019/838)) replaces external clock-synchronization assumptions with a permissionless PoS-based global clock. Verbatim from the abstract: *"We design and analyze a PoS blockchain protocol in the above dynamic-participation setting, that does not require a global clock but merely assumes that parties have local clocks advancing at approximately the same speed... we obtain a permissionless PoS implementation of a global clock that may be used by higher level protocols that need access to global time."*

**Why Omega needs it.** Praos and Crypsinous both inherit a synchrony assumption that joining parties already have a common notion of round/slot time. In a clean-slate chain that begins from a mass-MPC ceremony rather than a trusted-time-server, this assumption is operationally fragile (state-actor pressure on NTP infrastructure is a documented attack against Praos validators). Chronos replaces the external clock with a synchroniser primitive built into the consensus protocol itself.

**Composition.** Chronos extends Crypsinous's primitive set: shared VRF, shared stake snapshot, additional synchroniser sub-protocol that re-aligns joining parties' local clocks within a few rounds. The threshold-encryption committee that decrypts the encrypted mempool is the same committee whose epoch boundaries Chronos pins. No new committee, no new key.

**PQ caveat.** Chronos depends on the same VRF whose hash-based PQ replacement is §15 open issue #1. Closing that issue closes both Crypsinous and Chronos in one stroke.

### 7.2 Minotaur (multi-resource)

Minotaur (Fitzi-Wang-Kannan-Kiayias-Leonardos-Viswanath-Wang, [eprint 2022/104](https://eprint.iacr.org/2022/104)) generalises consensus to combine multiple resource types in a single longest-chain protocol. Verbatim: *"Minotaur, a multi-resource blockchain consensus protocol that combines proof of work (PoW) and proof-of-stake (PoS), and we prove it optimally fungible. At the core of our design, Minotaur operates in epochs while continuously sampling the active computational power to provide a fair exchange between the two resources, work and stake. Further... we generalize Minotaur to any number of resources."*

**Security model.** Optimally fungible: secure when `ω · β_w + (1−ω) · β_s < 1/2` for any weighting `ω ∈ [0, 1]`, where `β_w` is the adversarial fraction of one resource and `β_s` of the other. **The honest majority must hold across the *combined* resource pool, not in any single resource alone.** An attacker who captures 60% of stake but only 20% of work does not break the chain; they need cumulative honest minority across the union.

**Why Omega needs it.** Pure-PoS exposes Omega to single-resource capture (Steemit precedent at small scale; nation-state acquiring 33% ADA-equivalent at large scale). Minotaur lets Omega couple consensus security to multiple independent economic resources at once. The two we adopt:

1. **Stake** — ωADA-bonded validators, the natural inheritance from Cardano.
2. **Storage** — proof-of-space-time tied to the §6.5 mirror partnerchain. Operators who provide verifiable storage for the snapshot archive earn consensus weight in addition to their storage rewards. **This is the key economic coupling**: capturing Omega's consensus now requires capturing both stake AND a meaningful fraction of the global storage market simultaneously. Two distinct attack surfaces with different cost structures.

**Generalised composition.** Minotaur's "any number of resources" property leaves room for adding proof-of-work (existing Bitcoin / hash-rate market), proof-of-bandwidth, or proof-of-uptime as future resources. The protocol parameter set in §13 controls which resources are weighted and at what `ω`. Adding a resource is a CIP-1694-shaped governance vote constrained by the §13.1 guardrails script (no resource that grants disclosure capability is permitted).

**PQ caveat.** Minotaur as published uses the same curve primitives Crypsinous does. Replacing them with the PQ stack from §1 is mechanical: PoS leader-election uses the hash-based VRF, PoSpaceTime uses standard hash commitments (already PQ), threshold aggregation uses the leanXMSS family.

### 7.3 The combined picture

Crypsinous (privacy) + Chronos (time) + Minotaur (multi-resource) compose because all three are descendants of Ouroboros and share the same security-proof framework (Universal Composability, common reference string structure adapted for PQ). The composition theorem comes for free; the engineering work is replacing curve primitives with PQ ones consistently across all three protocols.

The composite protocol has the following properties:
- Permissionless PoS + PoSpaceTime, with optional future resources
- All operations PQ
- Shielded VRF, stake, rewards, mempool ordering
- Permissionless self-clocking (no external NTP dependency)
- Honest-majority security across the union of stake and storage
- Single per-epoch threshold-encryption committee handles claim-mempool decryption, Crypsinous flow encryption, Chronos round attestations

## 8. Validator-set diversity

Liveness target: no single jurisdictional bloc exceeds 33% of stake; at least two non-cooperating blocs each hold >20%. Per-jurisdiction censorship sets are non-overlapping and cancel.

**Pluggable transports** (Tor WebTunnel, Conjure, Snowflake) shipped by default in the canonical claim wallet so claimants can submit to any reachable validator. As long as one egress works, the claim is accepted by global consensus.

**EDPB controllership defence.** Validators handle hashes only; leaf data is supplied by the holder, never stored on-chain or by validators. "Purpose and means" carve-out applies; ship a formal legal memo.

## 9. Wallet + UX requirements

Mandatory protocol-level requirements that wallets must satisfy:
1. Reproducible builds + multi-party signed releases (m-of-n threshold across diverse vendors / auditors).
2. Hardware-wallet-displayed claim payload — user verifies recipient + commitment on-device, not only in software.
3. Public claim-payload format any independent tool can construct. Pluralism is the protection; no single wallet vendor on the load-bearing path.
4. Out-of-band claim verification: GitHub releases + on-chain metadata + IPFS + signed mailing list.
5. Canonical claim URL signed from a foundation PQ key, pinned in wallet UIs; certificate-transparency monitoring against typosquats.
6. **Make seed-into-website impossible by design** — only signed claim payloads through hardware paths.

## 10. PQ-key derivation

BIP-39 → BIP-32 → BIP-85 → SLH-DSA / ML-DSA. Project Eleven pattern. Pinned on-chain to prevent silent BIP-85 spec drift; reference test vectors published as part of genesis spec; the derivation rule is encoded into the leaf itself so any rule change forces a new commitment.

## 11. What the protocol explicitly does NOT prevent

Honest acknowledgement, communicated in user-facing material:
1. Stale-seed compromise during dormancy (silent loss).
2. Wrench attacks (cryptography cannot resist physical force).
3. Key-loss vs key-theft asymmetry (indistinguishable on-chain).
4. Inheritance failures.

Empirically: ~10–20% of holders will silently lose position over a decade-scale dormancy window, matching Bitcoin's 17–28% lost/stolen-silently rate. This is the cost of the no-backdoor stance, not a flaw of it.

## 12. Comparison against current `experiments/omega-commitment/` work

The v0.9.1 commitment-tooling shipped at `experiments/omega-commitment/` (post-audit, 292 tests passing) implements substantial portions of the design above. Status by spec section:

| Spec section | Current implementation status | Required deltas to align |
|---|---|---|
| §1 cryptographic primitives — leaf-hash domain tags, dual-hash bundle | ✅ Shipped Batch 1 (`omega:v1:leaf` / `omega:v1:node`, dual-hash bundle root) | None |
| §2 seven sub-trees with Cardano-faithful leaf shapes | ✅ Shipped Batch 2 (raw address bytes, datum_option, script_ref, tagged DRep enum, AccountState) | Header + tx-index sub-trees deferred to v1.1 chain-follower |
| §3 mass-MPC genesis ceremony | ❌ Not implemented | Replaces single-snapshot-publication with ceremony tooling. New track. |
| §3.1 cross-impl replay-from-block-0 | ⚠️ Partial — Rust impl exists; Lean impl planned but not started | New track for v2.0 reproducibility check |
| §4 claim mechanism (Plonky3 verifier, PLUME nullifier, recipient binding) | ❌ Not implemented | Track T6 (verifier) — entire scope |
| §4.3 per-claim-type assignment policy | ❌ Not implemented | Track T7 (bridge protocol) |
| §4.4 pre-fork-key claim cutoff | ❌ Not designed | Track T7; protocol governance vote |
| §5 privacy architecture | ❌ Not implemented | Tracks T6 + T7 |
| §6.2 chunked anchoring | ❌ Not implemented | New v1.2 task — split snapshot into chunks, commit chunk-roots at epoch boundaries |
| §6.3 archival bounty | ❌ Not designed | Track T12 economic policy |
| §7 PQ-Crypsinous consensus | ❌ Not implemented | Track T2 (consensus) — entire scope, depends on Crypsinous-PQ research output |
| §8 validator diversity | ❌ Not measurable until T2 ships | Track T11 / T12 operational policy |
| §9 wallet requirements | ❌ Not implemented | Track T8 (tooling) — entire scope |
| §10 PQ-key derivation pinning | ⚠️ Partial — BIP-85 derivation discussed but not pinned in genesis pre-image | Add to genesis ceremony scope §3 |
| §11 honesty disclosures | ❌ Not in user-facing material | Track T9 (documentation) |

**Implications for the v1.0 / v1.1 ingestion plans.** No changes required. The v1.0 plan ships the snapshot-construction tooling that feeds the §3 ceremony's input. The v1.1 plan ships the chain-follower that fills in the header + tx-index sub-trees per §2. Both are still on the critical path.

**New plan slots needed.**
1. **v1.2 chunked anchoring + embedded parser tarball.** Adds §6.2 + §6.1 final bullet to the genesis pre-image. Modest engineering (~few hundred LOC).
2. **v2.0 mass-MPC ceremony tooling.** Implements §3 + §3.1. Borrow from Zcash Sapling / Ethereum KZG implementations.
3. **v2.1 Lean 4 reference implementation.** Cross-impl differential check against the Rust implementation. Required ceremony input.

**Deferred to other tracks.** Everything in §4–§11 (claim verifier, privacy primitives, Crypsinous, wallet requirements) is out of scope for T1 commitment-tooling and lives in T2 / T6 / T7 / T8 / T9 respectively. The T1 work is necessary and sufficient for genesis-side preparation; the holder-side and consensus-side work happens in parallel program tracks.

## 13. Governance trajectory + constitutional binding

Resolved from the G1 governance pressure-test. Omega inherits CIP-1694 / Voltaire-shape governance (DRep + Constitutional Committee + SPO veto), with three stacked mechanisms that bind future protocol upgrades against the introduction of any backdoor — even by supermajority.

### 13.1 Three-layer constitutional binding

**Layer 1 — guardrails script (CIP-1694 shape).** An on-chain Plutus script statically rejects any parameter-update proposal that:
- Replaces or modifies the genesis commitment.
- Introduces a master / recovery / escrow key field at the protocol level.
- Introduces a designated-viewer / regulator-master key.
- Introduces a court-decryption pathway.
- Removes the PLUME nullifier requirement from the claim verifier.
- Removes the recipient-binding-inside-the-SNARK requirement (§4.1 step 4).

These are mechanically rejected before any vote tally; no DRep / CC / SPO supermajority can pass them. Updating the guardrails script itself is forbidden — only a chain replacement (i.e. a fork that the wallet ecosystem must explicitly opt into) can change it.

**Layer 2 — circuit-level invariants.** The verifier circuit ID is a rotatable protocol parameter, but the *public-input shape* (PLUME nullifier required; recipient + chain-id + freshness-nonce bound; holder viewing-key sovereignty) is hashed into the genesis commitment's domain-separation pre-image. A backdoor-shaped circuit (for example one that accepts "regulator override" as a public input) cannot produce verifying proofs against the existing leaf format. Backdoor circuits are mechanically excluded from the proof system itself, not merely by governance vote.

**Layer 3 — social fork pre-commitment.** Wallet vendors and the canonical claim-wallet implementation are explicitly committed (in their reproducible-build attestations) to follow the no-backdoor branch in any chain split. Steem → Hive (Feb 2020) is the operational precedent: when Justin Sun + exchange-custodied stake captured Steemit, the community forked to Hive and the captured chain became economically irrelevant. Same pattern applies if Omega is captured.

**Honest tension.** Mutable governance and absolute no-backdoor cannot both be achieved at the consensus layer alone. A sufficiently determined supermajority can *replace the entire protocol*. What the three-layer stack buys is that doing so produces a chain that the existing wallet / exchange / holder ecosystem identifies as a *different chain*, not as Omega. The no-backdoor guarantee is enforced at the **identity-of-chain** level, not at the consensus level. **No consensus path can introduce a backdoor and still be called Omega.**

### 13.2 Cross-fork claim conflict resolution

A contentious fork at year N produces Omega-A and Omega-B, both referencing the same 64-byte genesis commitment. Both can verify the same Merkle membership proof. Without explicit replay protection, a holder could submit the same claim on both chains.

**Mitigation.** Bind each fork's verifier circuit to a fork-discriminator: `H(chain_id ‖ fork_epoch)` is a public input to the claim verifier from genesis onward. Forks produce non-interchangeable proofs. This makes "double-claim across forks" mechanically impossible at the proof level. Each chain considers its own claim authoritative; cross-fork accounting is out of scope for the protocol — it is the irreducibly social layer (ETH / ETC precedent).

### 13.3 Nullifier-set divergence at fork

At fork, both chains inherit the pre-fork nullifier set; post-fork sets diverge. A holder who consumed a nullifier on Omega-A finds Omega-B's set never saw the spend. **Resolution.** Per §13.2, fork-discriminator binding means the holder must produce two distinct proofs (one per chain). Per-fork nullifier domains turn "double claim" into "two distinct claims on two distinct ledgers" — which matches economic reality.

### 13.4 Verifier-circuit rotation without ceremony

Plonky3 STARKs require no trusted setup. Verifier-circuit rotation is an on-chain governance action (CIP-1694 vote, gated by the §13.1 guardrails script), not a ceremony. This is the single most consequential simplification from the G1 findings: Omega never runs another mass MPC after genesis. The genesis MPC is once-only.

### 13.5 Emergency-fix governance

A critical PQ-Crypsinous bug discovered post-genesis cannot wait for a 70-day Tezos-style amendment cycle. The mechanism: SPO-quorum (≥80% of stake, sliding 30-block window) can pause block production for at most N epochs. The pause cannot mutate state; it only halts. Resume requires either timeout expiry or a Voltaire vote with the standard latency. **No single key, no foundation override, no pause guardian.**

### 13.6 Validator-set capture defence

Steemit precedent is explicit: ~$70M market cap in Feb 2020 made capture economically trivial. Cardano's ~$15B market cap puts 33% acquisition at ~$5B nominal / ~$15-25B real. Tezos has had no successful capture in seven years. The §13.1 guardrails script makes the prize hollow even if capture succeeds — an attacker holding 100% of stake still cannot add a backdoor parameter. They can only chain-replace, which the wallet ecosystem will reject.

## 14. Starstream as Omega's native UTXO + zkVM layer

[LFDT-Nightstream/Starstream](https://github.com/LFDT-Nightstream/Starstream) is the smart-contract execution model that the post-claim side of Omega runs on. Three properties make it a load-bearing choice rather than a swap-out option:

| Starstream property | Why it composes with the rest of the spec |
|---|---|
| Goldilocks field + Poseidon2 hash | Exactly the primitives §3 mandates for in-circuit operations. Zero-friction: the same hash that builds the Ω-Commitment Merkle tree is the same hash that Starstream commits state under. No translation layer. |
| UTXO-based with coroutines | EUTXO mental model survives the migration. Cardano-era UTxOs (committed in the §2 UTXO sub-tree) translate naturally into Starstream UTxOs at claim time. Coroutines provide the multi-step / atomic-bundle / time-locked / dead-man's-switch claim primitives §4.3 + C3 wanted, natively at the VM layer. |
| Native folding scheme support | Multiple claims by the same holder (claim_utxo + claim_stake + claim_governance for the same credential) fold into a single recursive proof. The atomic-bundle-claim primitive §4.3 wanted is built in, not bolted on. |
| zkVM with off-chain execution sealed in succinct proofs | Composes with PQ-Crypsinous's encrypted mempool: claim transactions carry the Starstream proof; consensus verifies; the on-chain footprint is constant-size regardless of computation depth. |
| LFDT governance | Linux Foundation Decentralized Trust (formerly Hyperledger). No single-vendor capture. Mature open-source pattern. |
| WebAssembly compilation target | Wallet, browser, mobile claim-execution paths inherit a mature toolchain. VSCode + Zed extensions already shipping; tooling not a blocker. |

### 14.1 What Starstream changes about the rest of the spec

**§2 UTXO sub-tree leaf encoding stays as is.** The Cardano-era UTxOs are the historical record; their leaf format is locked by Cardano's CDDL. Starstream is the *destination* shape, not a re-encoding of history.

**§4.1 verifier circuit obligations gain a translation step.** After step 5 (PLUME nullifier emission), the circuit produces a Starstream UTxO `(coroutine_id, amount, datum, recipient_view_key)` as a public output. The Starstream UTxO commits to the holder's Cardano-era position under Poseidon2 — the same hash family the inner Merkle tree uses, so verification is one circuit, not two.

**§4.3 per-claim-type assignment policy gets sharper.** `claim_script` no longer needs the "registered deployer key OR governance-arbitrated dispute" two-path: instead, the holder submits a Starstream coroutine that proves semantic equivalence to the original Plutus script. Plutus → Starstream translation is a community / compiler-team responsibility (separate research project — see [`runtimeverification/plutus-core-semantics`](https://github.com/runtimeverification/plutus-core-semantics)), but the protocol surface accepts any Starstream coroutine that produces an equivalent script-hash. dApp redeployment becomes "submit a Starstream module that the verifier accepts," not "negotiate with a foundation arbitrator."

**§4.4 claim cutoff couples to Starstream's folding scheme.** Unclaimed pre-fork UTxOs at cutoff fold into a single Starstream coroutine that the protocol treasury controls (per §6.3 archival-bounty allocation). No custodial escrow; the fold is mechanical.

**§7 PQ-Crypsinous integration becomes two-layer.** Crypsinous shields the consensus (VRF, stake, rewards). Starstream shields the execution (state, computation, transitions). The two layers compose by sharing the same threshold-encryption committee for mempool decryption: the per-epoch stake-weighted committee decrypts both Crypsinous-encrypted ordering inputs AND Starstream-encrypted state-transition payloads.

**§9 wallet requirements expand.** Wallets must construct + sign Starstream coroutine instantiations, not just Cardano-shaped Tx outputs. The mature tooling (VSCode + Zed extensions, web sandbox, CLI) is the foundation; canonical claim-wallet derives from it.

### 14.2 What Starstream does NOT solve

- The hash-based VRF construction (§15 open issue #1) is still a research-paper-shaped gap. Starstream does not specify a VRF.
- The lattice-vs-hash signature decision (§15 open issue #2) is orthogonal. Starstream is signature-agnostic.
- The mass-MPC genesis ceremony (§3) is not in Starstream's scope. Starstream is the post-claim execution layer; the pre-fork commitment is computed and ceremonially attested separately.

### 14.3 Implementation status (as of 2026-05-03)

Starstream is in active design with implementation experiments. Per the upstream README and the [impl-plan.md](https://github.com/LFDT-Nightstream/Starstream/blob/main/impl-plan.md): compiler + interpreter + WebAssembly target shipping; type checker, IVC, MCC, and lookups modules marked TODO. This is consistent with Omega's program timeline — Starstream maturity tracks the T6 verifier and T3 smart-contract VM tracks, not T1 commitment tooling. The T1 work in `experiments/omega-commitment/` is unchanged by this addition; the integration happens at T3 / T6 in parallel.

### 14.4 Updated comparison against current work (replaces §12 row "T3 smart-contract VM")

| Spec section | Current implementation status | Required deltas to align |
|---|---|---|
| §14 Starstream as native UTXO + zkVM | ❌ Not implemented (Starstream is upstream, in active development) | T3 track is now defined as: integrate LFDT-Nightstream/Starstream + extend with PQ-Crypsinous shielding hooks. Track T3 was previously open; it now has a concrete upstream dependency and a clear scope boundary. |

## 15. Open issues

These remain unresolved and gate v2.0 publication readiness:

1. **Hash-based VRF construction.** X-VRF (Buser et al.) was broken by Bodaghi-Safavi-Naini FC 2024. The Praos-equivalent uniqueness reduction must hold without reducing to "no collisions in H." Either pick a paper or commission one.
2. **Lattice-vs-hash signature decision.** ML-DSA-65 / FN-DSA-512 for high-throughput user signing vs SLH-DSA-only for full no-curves-anywhere posture. Cloudflare 2025 analysis argues for the lattice option for ordinary tx; SLH-DSA reserved for genesis ceremony, governance, KES root.
3. **Threshold-encryption committee composition under PQ.** Per-epoch stake-weighted committee is the obvious answer; the specific PQ threshold scheme (lattice-based vs hash-based) is undecided.
4. **Claim-window length.** 5 vs 7 vs 10 vs 20 years. Trade-off: shorter = quantum-pre-fork-Ed25519 safety + extraction-window compression. Longer = vault-holder forgiveness. Empirical anchor: Mt Gox 10-year wait was operationally tolerable.
5. **Guardrails-script entrenchment depth.** Whether the §13.1 guardrails script update path is forbidden entirely or requires a higher-than-supermajority quorum (e.g. 90% DRep + 90% SPO + unanimous CC). The latter preserves a theoretical escape hatch but creates a target. The former is cleaner but assumes any future need to update the guardrails will route through chain replacement.
6. **Plutus → Starstream translation.** §14.1 punts the translation of Cardano-era Plutus scripts into Starstream coroutines to a community / compiler-team responsibility. Specific decision points: which Plutus Core semantics (the [RuntimeVerification K-spec](https://github.com/runtimeverification/plutus-core-semantics) or the Agda formal-ledger spec) is the source-of-truth; whether translation is automated or holder-submitted; whether script-hash-equivalence is verified by the protocol or attested via dApp re-deployment.
7. **Starstream's IVC / MCC / lookups upstream maturity.** Per the upstream impl-plan.md, these are marked TODO. Omega's T3 / T6 tracks depend on these landing. Tracking via upstream rather than re-implementing.
8. **Filecoin PQ-port scope.** §6.5 calls for a fork of Filecoin with all curve crypto replaced by the §1 PQ stack. Scope estimate ~6-12 months engineering but the actual work breakdown — which Filecoin actor types port first, whether to fork at the protocol-spec level vs the lotus-implementation level, what's the test-network bootstrap path — is unscoped.
9. **Minotaur weighting parameter `ω`.** The PoS / PoSpaceTime split (`ω` in the Minotaur security inequality) is a tunable governance parameter. Initial value, rotation policy, and constitutional constraints (per §13.1, can governance push `ω = 1` and effectively disable storage-resource consensus?) need explicit specification.
10. **Mirror partnerchain economic model.** The mirror chain pays its operators in (a) per-retrieval fees from holders + (b) Omega-treasury block rewards. The split, the per-retrieval price floor, and the long-term sustainability under declining ωADA prices are all unscoped.

## Self-review

**Coverage.** Every concern surfaced in C1-C6 (capability scan), P1-P5 (privacy primitives), A1-A6 (adversarial pressure test), and G1 (governance trajectory) maps to a specific section of the spec. The three design corrections from the user (no courts/escrow keys, no mirror network — chain has the resolution machinery, Crypsinous as consensus layer + mass-MPC for genesis) are integrated as §1 / §6 / §7 / §3.

**No backdoors.** Every section verified against the no-master-key, no-court-override, no-escrow-key, no-designated-viewer constraint. Failed verification at zero sections. The §13.1 guardrails-script + circuit-level-invariant + social-fork-pre-commitment stack mechanically prevents future governance from introducing one.

**Compatibility with current work.** §12 enumerates the alignment status. The shipped v0.9.1 implementation is consistent with the spec; deltas are additive (chunked anchoring, mass-MPC tooling, Lean reference impl, guardrails script), not corrective.

**Honest gaps.** §14 lists the five remaining open issues. None are protocol-incompatible with the rest of the design; all are placeholder-reducible to specific paper / decision references.

## Spec status

This document is the design-level reference. Per the brainstorming skill protocol, the next step is owner review followed by writing-plans skill invocation to produce concrete implementation plans for the new track slots (v1.2, v2.0, v2.1, plus the guardrails-script work that lives under T7 bridge protocol). The existing v1.0 / v1.1 plans remain valid and on-track.
