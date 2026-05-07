# Raftlet Post-Quantum Cryptography Stack — Design Spec

**Date:** 2026-05-06
**Branch:** `feat/raftlet-fizzbee` (model and verification done; this spec covers the implementation crypto)
**Status:** design synthesized from 5 parallel research streams + 1 QA synthesis pass + 1 user augmentation (TPM-assisted KES)
**Predecessor work:** `models/raftlet/raftlet.fizz` (M3 — all safety + liveness invariants PASS). Implementation crypto for the M4 implementation phase.

## Scope

The Raftlet implementation needs an end-to-end set of cryptographic primitives. This spec resolves: hot-path signature scheme, certificate composition, hash function (protocol + STARK-circuit), block-id construction, **KES with TPM-anchored epoch protection**, governance threshold, HSM strategy, and ceremony structure. It does NOT cover wire format, network transport, persistence, or pacemaker — those are separate M4 design layers.

## Resolved end-to-end stack

| Slot | Decision | Rationale |
|---|---|---|
| **Hot-path signature** | **ML-DSA-65** via `libcrux-ml-dsa ≥0.0.4`, exposed behind a `RaftletSig` trait in `raftlet-crypto` | NIST FIPS 204 finalized. Level 3 (~AES-192) is the only defensible choice for a chain that aspires to multi-decade forge resistance. Sig 3,309 B; pk 1,952 B. ~3,000–10,000 signs/sec/core in portable Rust. |
| **Cert composition** | **Concatenated ML-DSA-65 sigs** with a Merkle root commitment over `(voter_id, sig_bytes)` leaves. Full sigs gossiped during cert formation; canonical chain stores root + voter set. | Same scheme as hot path — primitive monogamy. ~16 KB cert at n=4 (worst case 7 voters → ~23 KB). The 12 KB savings from Falcon-512 isn't worth doubling the audit surface. |
| **Hash (protocol layer)** | **BLAKE3** (`blake3` v1.x), keyed mode for MACs and transcript binding, derive-key mode for KDFs | 6–8 GB/s on commodity x86_64. 119M downloads, 1,091 dependents (Solana, IPFS, OpenZFS). No formal audit, but the most-deployed Rust hash. |
| **Hash (STARK circuit)** | **Poseidon2 over BabyBear**, width 8 (compression) and width 12 (sponge), HALF_FULL_ROUNDS=4, PARTIAL_ROUNDS=22 | Matches existing omega-commitment parameters; reuses the same hash AIR. Active cryptanalysis on reduced-round instances (ePrint 2025/954, 937, 1916; 2026/150) — full-round unbroken. Pre-design Keccak-AIR fallback. |
| **Block-id construction** | `BLAKE3-XOF(domain_sep ‖ canonical_block_bytes)` → 512 bits | Hedges Grover's algorithm: 256-bit output gives only ~128-bit PQ pre-image resistance under conservative analyses. 512-bit hedges to 256-bit PQ for long-lived chain-internal references. |
| **KES (forward-security)** | **TPM-anchored software KES.** Software impl in `raftlet-kes` (HSS/LMS or XMSS-MT — see §"KES variant choice"); TPM provides epoch rollback protection via NV counter, sealed state, and epoch-key certification. See §"TPM-Assisted KES Architecture". | Resolves the v0.1 forward-security gap WITHOUT requiring an audited from-scratch Rust XMSS-MT (which is 6-12 months critical-path). TPM protects against the dominant statefulness hazard (leaf reuse after crash + restore). |
| **VRF** | **Punt entirely from Raftlet.** Raftlet's leader election is vote-based (per the FizzBee model). | A VRF is an Omega/Crypsinous concern, not a Raftlet concern. Designing now creates premature coupling. |
| **Cold-path / governance threshold** | **Coordinator-based MPC over ML-DSA-65** for v0.1; track Hermine (ePrint 2026/419) for v0.2 | Hermine is a 2-month-old paper. Acceptable as a tracked successor; not acceptable as a v0.1 governance dependency. |
| **HSM strategy (governance only)** | **AWS KMS ML-DSA** as the v0.1 primary. Add Thales Luna v7.9 once its FIPS 140-3 PQC cert lands. **YubiHSM is excluded** — no public PQ roadmap. | AWS KMS is FIPS 140-3 Level 3 certified for ML-DSA since June 2025. HSMs protect governance roots only — not the hot path (latency mismatch). |
| **TPM strategy (per-validator)** | **Required for KES rollback protection.** Validators run on hardware with a TPM 2.0 (Windows: CNG/Platform Crypto Provider + TBS; Linux: tpm2-tss). | TPM is per-validator hardware that anchors KES state. Distinct from HSM (which is a centralized cloud / network appliance for governance). |
| **Ceremony** | **No trusted-setup ceremony for v0.1.** Genesis = deterministic build of validator pubkey set + ML-DSA-signed bootstrap manifest, attested via independent reproducible builds + Sigsum-style transparency log. | Lattice signatures need no Powers-of-Tau SRS. The right pattern is reproducible-build attestation + transparency log, not Sapling-style PoT. |

## TPM-Assisted KES Architecture

KES (Key-Evolving Signatures) provides forward-security: a key compromised at epoch `e` cannot retroactively forge signatures from epochs `< e`. Cardano's existing KES uses Ed25519 in a Malkin-Micciancio-Miner sum construction. The PQ replacement uses hash-based or lattice signatures inside the same sum-tree shape, but the dominant operational risk shifts from cryptography to **state management**: stateful hash-based schemes (XMSS-MT) require durable monotonic-counter persistence, and a leaf-reuse incident after crash recovery breaks forward security.

### Approach: TPM anchors KES state; software signs

The TPM does **not** sign Raftlet messages directly. It performs four roles:

1. **Anchor KES state.** Validator's KES private state is sealed to the TPM under a PCR policy + NV counter. The state cannot be unsealed on a different machine, after a TPM reset, or after rolling back the NV counter.
2. **Certify epoch public keys.** A non-exportable TPM key signs each epoch's KES public root, producing remote-attestation evidence that the epoch key was generated on a known validator device.
3. **Enforce monotonic epoch rollback.** A TPM NV counter is incremented at epoch boundaries (not per-vote). State refusing to unseal at `state.epoch < tpm_counter` prevents rolled-back states from signing.
4. **Quote PCRs and NV counter for remote verification.** Peers verify the quote chains to an accepted validator device identity.

### Component map

```
Raftlet message
  -> Rust KES signer (raftlet-kes)
    -> uses current epoch signing state
    -> state is sealed/unsealed by TPM policy   (raftlet-tpm)
    -> epoch monotonicity checked vs TPM NV counter
  -> outputs KesSignature + epoch certificate id
```

| Component | Crate | Responsibility |
|---|---|---|
| `KesEngine` | `raftlet-kes` | Implements the KES scheme. Owns transcript formatting + leaf/index allocation. |
| `TpmKesBackend` | `raftlet-tpm` | Abstracts TPM ops: create non-exportable identity key; seal/unseal state; read/increment NV counter; quote PCRs+NV; sign/certify epoch pubkeys. |
| `SealedKesState` | `raftlet-tpm` | On-disk artifact unusable without TPM. Fields: scheme id, validator node id, chain id, current epoch, leaf cursor, encrypted private state, public root, TPM policy digest, NV counter id, MAC, last epoch cert id. |
| `EpochCertificate` | `raftlet-crypto` | Published per-epoch (not per-vote). Fields: validator node id, epoch number, KES epoch public key/root, TPM attestation key id, TPM quote, TPM signature over the epoch pubkey, expiration / max message count. |
| `KesSignature` | `raftlet-crypto` | Attached to Raftlet messages. Fields: scheme id, epoch, leaf/index, transcript hash, hash-based signature, epoch certificate reference. |

### Signing flow

1. Load `SealedKesState`.
2. Read TPM NV counter.
3. **Reject** if `state.epoch < tpm_counter` — refuse to sign with a rolled-back state.
4. Unseal state via TPM policy.
5. Reserve a leaf (or small leaf range) in the local journal.
6. Sign the Raftlet transcript with the software KES.
7. Persist advanced state **before** releasing the signature.
8. Return `KesSignature`.

**Critical performance note:** the TPM NV counter is incremented **per epoch, not per vote**. Per-vote NV writes are too slow and wear NV storage. Within an epoch, crash safety relies on a software journal of reserved leaves — not on TPM operations.

### Epoch rotation

1. Decide next epoch (tied to Raftlet term, validator key period, or block range).
2. Increment TPM NV counter.
3. Derive or load next KES epoch state.
4. Seal new state to TPM policy.
5. TPM certifying key signs the new epoch public key.
6. Publish `EpochCertificate`.
7. Securely erase old epoch secret material.

**Crash rule.** If the TPM counter advances but the new state is not saved, the validator skips that epoch. Losing signing capacity is acceptable; reusing old signing state is not.

### Verifier behavior

Peers verify TPM-independently:

- Epoch certificate is well-formed and unexpired
- TPM quote/certification chains to an accepted validator device identity (configured at genesis or via on-chain registration)
- Epoch is not expired
- KES signature verifies under the certified epoch public key
- `(validator, epoch, leaf_index)` has not already been seen for a conflicting transcript (this enforces the FizzBee model's `HonestVoteConsistency` invariant at the wire layer)
- Transcript domain separator is correct (e.g., `raftlet:v1:notarization-vote`)

### Security properties claimed

- Rollback resistance across epochs via TPM NV counter
- Device binding via TPM sealed state
- Remote evidence that the epoch key was certified by a TPM-backed key
- Forward security if old KES state is erased correctly
- STARK-friendly path if the KES scheme is hash/Merkle based

### Security properties NOT claimed

- TPM-native hash-based signatures (the TPM doesn't sign with KES)
- Protection from a fully compromised live OS while the KES state is unsealed
- Perfect same-epoch rollback protection unless the local leaf journal is correct
- Production assurance for experimental Poseidon/WOTS-style KES schemes

### KES variant choice

Three candidate schemes for `KesEngine`:

| Variant | Standards | STARK-friendly | Production status |
|---|---|---|---|
| **HSS/LMS** (RFC 8554, NIST SP 800-208) | yes | no (SHA-256/SHAKE not cheap inside Plonky3) | Reference C; Rust crates exist but unaudited |
| **XMSS-MT** (RFC 8391, NIST SP 800-208) | yes | no | `xmss` crate exists but unaudited |
| **Poseidon-WOTS-Merkle** | no — research | yes (matches Plonky3 hash AIR) | Experimental — implement behind a feature gate |

**v0.1 default: HSS/LMS or XMSS-MT** (whichever has the more credible Rust crate at audit time — likely `xmss-rs` or a forked maintained variant). Standards-aligned matters for v0.1 procurement; STARK-friendliness is a v0.2 concern that can be added behind `--features stark-friendly-kes`.

## Crate shape

```
raftlet-crypto
  SignatureSuite trait
  KesSignature, EpochCertificate types
  transcript / domain separation strings (raftlet:v1:proposal, etc.)
  hash adapters (BLAKE3 protocol, Poseidon2 circuit)

raftlet-kes
  KES state machine
  HSS/LMS or XMSS-MT impl adapter
  experimental `stark-friendly-kes` feature gate (Poseidon-WOTS-Merkle)
  transcript / leaf-allocation logic

raftlet-tpm
  TpmKesBackend trait
  Windows backend (CNG / Platform Crypto Provider + TBS)
  Linux backend (tpm2-tss) — added in M4.5
  SealedKesState + NV counter wiring

raftlet-core
  consumes SignatureSuite
  does NOT depend directly on TPM
  verifier path is TPM-independent
```

**Boundary discipline.** `raftlet-core` (the protocol engine — the FizzBee model translated to Rust) verifies signatures and certificates but never talks to a TPM. `raftlet-tpm` only helps a local validator protect and attest its signing state. This makes the verifier code portable (light clients, browser verifiers, non-TPM hardware) while still letting validators leverage hardware anchoring.

## Trade-offs accepted

1. **Zero audited PQ Rust crates in the trusted base.** `libcrux-ml-dsa`, `blake3`, Plonky3 Poseidon2, and whatever KES crate we choose are all unaudited. See §"Audit plan".
2. **~16–23 KB notarization certs.** Falcon-512 would give ~3.3 KB but mixing two PQ primitives doubles audit surface. We pay the bytes for primitive monogamy.
3. **TPM hardware requirement** for validators. A no-TPM validator cannot run the v0.1 reference client. Documented as an operator requirement.
4. **HSS/LMS or XMSS-MT chosen pragmatically** rather than the STARK-friendly Poseidon-WOTS-Merkle. v0.2 can flip on `--features stark-friendly-kes` once the experimental scheme has analysis.
5. **No threshold signing for v0.1 governance.** Coordinator-MPC means the coordinator host is trusted not to selectively-abort. Hermine deployment in v0.2 fixes this.
6. **Block IDs are 512-bit on-the-wire.** Doubles the bytes per cert reference. The Grover-hedge cost.
7. **Reduced-round Poseidon2 cryptanalysis pressure.** Active research; full-round unbroken; Keccak-AIR fallback pre-designed.
8. **AWS KMS lock-in for v0.1 governance HSM.** Single vendor for cold-path is a SPOF. v0.2 adds Thales Luna multi-vendor.

## Questions the user must answer

These could not be resolved without protocol-level input:

1. **What is Raftlet's signature throughput target per validator?** ML-DSA-65's 1.5× signing-time penalty over ML-DSA-44 only matters above ~5,000 signs/sec/validator. Pin this to confirm Level 3 is comfortable.
2. **What is the v0.1 ship date and chain-lifetime ambition?** "Decades" of forge resistance forces Level 3+. "5 years until v1.0 re-cuts crypto" allows Level 2.
3. **What is the validator-set size at mainnet?** Modeled as `n=4`. If real deployment is `n=100`, the cert-size analysis changes by 25×.
4. **What slot length?** 1-second slots make 1.7 MB/sec gossip plausible on commodity hardware; 100 ms slots make it impossible.
5. **Is governance threshold a v0.1 requirement or v0.2?** If v0.1, coordinator-MPC is the answer; if v0.2, we wait for Hermine to mature.
6. **Is "pure Rust" a hard constraint or preference?** `pqcrypto-falcon` (FFI) is excluded under hard constraint; allowed under preference.
7. **Who owns the audit budget?** Audits in §"Audit plan" are six figures each.
8. **Is TPM 2.0 a workable operator requirement?** Excludes some bare-metal / cloud-VM deployments. Acceptable?

## M4 implementation order — re-validated

Original recommendation: **crypto trait → wire → persistence → network → pacemaker.**

The TPM-KES decision plus the QA agent's persistence-contract observation forces one revision:

1. **Crypto trait** (`RaftletSig`, `RaftletHash`, `RaftletKes`, `EpochCertificate` types — even if KES is a stub initially)
2. **Persistence** (must expose monotonic-counter primitive AND TPM sealed-state I/O before wire freezes)
3. **Wire** (now knows what KES + epoch-cert metadata to reserve room for)
4. **Network**
5. **Pacemaker**

Persistence moved up because:
- `SealedKesState` is a persistence concern with TPM dependencies
- The local leaf-reservation journal is a persistence primitive
- Wire format must reserve fields for `KesSignature.{epoch, leaf, scheme_id}` and `EpochCertificate` reference

Crypto trait at step 1 must be parametric so v0.1→v0.2 KES-variant upgrades don't refactor every consumer.

## Audit plan

Critical-path crates to audit, in priority order:

| # | Crate | Scope | Budget hint | Trigger |
|---|---|---|---|---|
| 1 | `libcrux-ml-dsa` | Constant-time guarantees on x86-64 + aarch64; SHAKE correctness post-0.0.4 fix; deterministic-randomization path | $80–120k | Before v0.1 mainnet |
| 2 | `raftlet-kes` (own crate) | KES state-machine correctness; leaf-allocation monotonicity; transcript formatting; crash-recovery semantics | $100–150k | Before v0.1 mainnet |
| 3 | `raftlet-tpm` (own crate) | Sealed-state binding; NV-counter monotonicity; quote chain validation; non-exportable key invariants | $80–120k | Before v0.1 mainnet |
| 4 | `blake3` (Rust) | Less critical (mature, deployed) but no formal audit. Scope: Rust-specific impl, not BLAKE3 spec | $40–60k targeted | Before v0.1; bundle with #1 |
| 5 | Raftlet's own `RaftletSig` adapter + transcript domain separation | Glue is where bugs hide. Domain-separation strings, canonical encoding, vote-replay guards, voter-id binding | $50–80k | Before v0.1 |
| 6 | Plonky3 Poseidon2 width-8/12 BabyBear | Constraint correctness vs reference impl; lookup-arg soundness | $60–100k | Before any STARK that touches consensus state |
| 7 | Hermine impl (when one exists) | Threshold ML-DSA correctness + abort-resilience | $150k+ | Before v0.2 governance |

**Total v0.1 audit envelope:** ~$350–530k (crypto + KES + TPM + glue + BLAKE3). **v0.2 envelope:** ~$200k+ (Poseidon2 + Hermine).

If the budget isn't there, the honest call is: **don't ship to mainnet at v0.1**. Run testnet only with a published "unaudited cryptography" warning, and use the testnet duration to fund the audit.

## Resolved sub-questions

**ML-DSA-44 vs 65.** Pick **65**. Level 2 (~AES-128) is fine for a 5-year horizon but inadequate for "decades." The 2× sig-size penalty (3,309 vs 2,420 B) is below the noise floor of network gossip. Default to 65; downgrade only if the throughput target (Q1) forces it.

**Algorand status.** Falcon-1024 is live for **user-account "Falcon logic signatures"** (Nov 3, 2025). Consensus block proposals + committee VRF still use Ed25519. Native Falcon consensus is on the 2026 roadmap, possibly slipping to 2027. Don't cite Algorand as proof PQ "works in consensus production."

**HSM viability.** **AWS KMS ML-DSA only** for v0.1 governance (FIPS 140-3 Level 3 certified, GA since June 2025). Thales Luna v7.9 has ML-DSA in firmware but FIPS 140-3 PQC cert is in progress. Add as backup once cert lands.

**Aggregation vs hot-path.** Resolved to ML-DSA-65 + concatenation. Mixing primitives doubles audit surface for a 12 KB savings.

**VRF scope creep.** Punt entirely. Raftlet's election is vote-based per the FizzBee model.

**TPM as KES anchor.** The user's augmentation. Resolves the QA agent's "punt KES to M5" by giving v0.1 forward-security WITHOUT requiring a from-scratch audited XMSS-MT crate as a critical-path item — software KES is the workable thing to audit, TPM is COTS hardware.

**No-audit reality.** §"Audit plan" is the honest plan. If unfunded: don't ship to mainnet.

## Hidden assumptions surfaced

- **"Thousands of sigs/sec" hot-path target is unverified.** Q1 above must answer.
- **Sapling-style PoT ceremony is wrong-shaped for this stack.** Lattice schemes have no SRS. Right pattern is reproducible-build attestation + transparency log.
- **Grover gives 128-bit PQ on a 256-bit hash — debated.** Bernstein (2009) and follow-ups argue weaker effective security. Hedge: BLAKE3-XOF→512 bits for chain-internal references.
- **"Decades-long forge resistance" was never converted to a key-rotation cadence.** Q2 above must answer.
- **TPM 2.0 hardware availability** for validator operators is implicit. Q8 must answer.

## Cross-cutting

- **Total sig footprint per slot.** ML-DSA-65 (3,309 B) at `n=4, f=1`: ~13 KB/slot. At `n=100`: ~330 KB/slot. At 1-second slots, the latter is real but tractable; at 100 ms slots, it isn't. User must pin (Q3, Q4).
- **KES persistence contract.** Persistence layer at M4 step 2 must expose: (a) monotonic counter primitive for KES leaf index, (b) TPM sealed-state I/O for `SealedKesState`, (c) transactional leaf-reservation journal.
- **HSM signing latency vs hot-path.** Resolved: hot-path keys live in process memory protected by TPM-anchored KES rollback; HSMs (cloud) protect governance roots only.
- **TPM is per-validator hardware**, not a centralized HSM. The two are complementary, not substitutable.

## Sources

Aggregated from 5 research streams + QA + user augmentation:

- [FIPS 204 (ML-DSA)](https://csrc.nist.gov/pubs/fips/204/final)
- [FIPS 205 (SLH-DSA)](https://csrc.nist.gov/pubs/fips/205/final)
- [FIPS 206 draft (FN-DSA / Falcon)](https://csrc.nist.gov/pubs/fips/206/ipd)
- [NIST SP 800-208 — Stateful Hash-Based Signatures](https://csrc.nist.gov/Projects/stateful-hash-based-signatures)
- [libcrux-ml-dsa crate](https://crates.io/crates/libcrux-ml-dsa)
- [RUSTSEC-2025-0144 — ml-dsa timing side-channel](https://rustsec.org/advisories/RUSTSEC-2025-0144.html)
- [BLAKE3 IETF draft](https://www.ietf.org/archive/id/draft-aumasson-blake3-00.html)
- [Poseidon Cryptanalysis Initiative](https://www.poseidon-initiative.info/)
- [Poseidon2 ePrint 2023/323](https://eprint.iacr.org/2023/323.pdf)
- [Key Updatable Hash-Based VRF — ePrint 2026/052](https://eprint.iacr.org/2026/052)
- [Hermine — ePrint 2026/419](https://eprint.iacr.org/2026/419)
- [Mithril threshold ML-DSA — ePrint 2026/013](https://eprint.iacr.org/2026/013)
- [State of PQC in Rust — Project Eleven](https://blog.projecteleven.com/posts/the-state-of-post-quantum-cryptography-in-rust-the-belt-is-vacant)
- [AWS KMS ML-DSA announcement](https://aws.amazon.com/blogs/security/how-to-create-post-quantum-signatures-using-aws-kms-and-ml-dsa/)
- [Thales Luna v7.9 PQC](https://cpl.thalesgroup.com/blog/encryption/luna-hsm-pqc-quantum-safe-encryption)
- [Microsoft TPM fundamentals](https://learn.microsoft.com/en-us/windows/security/hardware-security/tpm/tpm-fundamentals)
- [Microsoft TPM/Platform Crypto Provider](https://learn.microsoft.com/en-us/windows/security/hardware-security/tpm/how-windows-uses-the-tpm)
- [Algorand Falcon mainnet](https://algorand.co/blog/technical-brief-quantum-resistant-transactions-on-algorand-with-falcon-signatures)
