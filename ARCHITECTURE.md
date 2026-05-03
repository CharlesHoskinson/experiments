# Architecture

This document explains the design of the Ω-Commitment: what it captures, how it is structured, and how the v1.0 mainnet ingestion pipeline produces one. It is the deep-dive companion to [`README.md`](./README.md), which frames the program at a higher level, and to [`GOALS.md`](./GOALS.md), which lists what we are trying to accomplish across all twelve tracks.

## Cryptographic primitives

Every primitive used in the Ω-Commitment construction is post-quantum. No curve operations anywhere in the build, the verify, or the leaf encoding. This is the load-bearing constraint of the whole program. If a single curve hash, signature, or commitment crept into the construction, an attacker with a sufficiently large quantum computer could forge a Merkle membership proof against the published root, and the bridge from Cardano would no longer be sound. The design starts from the constraint and works backward.

The hash functions are Blake3-256 and SHA3-256. Blake3 is the workhorse: it computes every leaf hash and every per-sub-tree root, and is broadly faster than SHA-2 on 64-bit hardware while remaining trivial to constrain inside a STARK circuit. SHA3 is paired with Blake3 only at the bundle layer, the single hash above the seven sub-tree roots. As of the 2026-05-03 audit reframing (finding A1/F004 in `audit/SUMMARY.md`), the SHA3 root is documented as drift detection over the same seven Blake3 sub-tree roots — not as a hedge against a Blake3 break. Both bundle roots aggregate identical Blake3 leaf hashes, so a leaf-level Blake3 break would defeat both; a divergence between the two bundle roots therefore signals an aggregation-step bug, and the truly-independent SHA3 tree (separate per-leaf SHA3 hashing) is tracked as a v2.0 follow-up.

The Merkle tree is binary, fixed-arity, and padded to the next power of two with a domain-separated empty leaf. Arity matters because variable-arity trees (for instance, the Patricia trie used by Ethereum) require the prover to handle a much wider set of node shapes inside the verifier circuit. Fixed arity collapses the verifier into a tight loop. Every leaf hash binds the sub-tree id and the canonical sorted index into its preimage via `H("omega:v2:leaf" || sub_tree_id || canonical_index_be || payload_len_be || payload)`, and every internal node carries a distinct `omega:v1:node` tag. Padding leaves use a reserved `EMPTY_INDEX_SENTINEL = u64::MAX`, so a verifier that knows the published `item_count` can reject any inclusion proof whose canonical index is `>= item_count` as a padding-leaf forgery. The cost of the domain tags is a few bytes per hash, paid once at construction time, in exchange for closing the second-preimage swap and zero-padding membership attacks the audit flagged (A1/F001-F003).

Leaf ordering is deterministic and lexicographic. Inside each sub-tree, leaves are sorted by the natural identifier of the entity (transaction id for UTxOs, slot number for headers, policy id for native tokens, and so on). This makes the sub-tree root a function of the input set alone, independent of the order in which the parser produced entries. Without this property, two byte-different snapshots that represent the same logical state would produce different roots, and the cross-implementation reproducibility check that gates the genesis ceremony would be impossible.

The whole construction is plonky3-friendly. That phrase has a precise meaning: every operation needed to verify a Merkle membership proof against the published root must be expressible as STARK constraints without specialized cryptographic gadgets. Hash-only constructions satisfy this naturally; curve-based constructions do not. The v1.0 verifier circuit, which is track T6 of the program and lives downstream of this repository, leans on this property heavily. Without it, the per-claim verification cost on Omega's ledger would be high enough to make lazy resurrection economically painful, which would defeat the purpose of the design.

## The 7 sub-trees

| # | Sub-tree | What it captures | Leaf encoding |
|---|---|---|---|
| 1 | UTXO | Every unspent output: tx_id, output_index, address_hash, value (lovelace + native-asset bundle), optional datum hash | variable, ~81 bytes minimum (locked from v0.9.1) |
| 2 | Header chain | Every block header: slot, block_height, block_hash, prev_hash | 80 bytes (locked from v0.2.0) |
| 3 | Transaction index | Every transaction: tx_id, slot, block_hash, position-within-block | 76 bytes (locked from v0.3.0) |
| 4 | Native token policies | Each policy: policy_id, first_issuance_slot, total_supply | 52 bytes (locked from v0.4.0) |
| 5 | Script registry | Each on-chain script: script_hash, deployment_slot, script_size, language byte (Native / PlutusV1 / V2 / V3) | 41 bytes (locked from v0.5.0) |
| 6 | Stake state | Each stake credential: credential, delegated_pool, DRep delegation, rewards_balance, is_pool_operator | 93 bytes (locked from v0.6.0) |
| 7 | Governance state | Each governance fact: kind, key, value (u128), slot. Kinds: treasury / CC seat / ratified gov action / in-flight gov action | 57 bytes (locked from v0.6.0) |

The seven sub-trees cover the seven categories of state a holder might want to claim on Omega and map cleanly onto Cardano's existing on-chain types. The decomposition is not the obvious one. An earlier draft had a single flat tree over a heterogeneous union of all entity types, which would have been simpler to construct but harder to prove against. With seven separate trees, a `claim_utxo` proof verifier only needs to know about the UTXO sub-tree's structure and its root; it does not need to reason about how stake or governance entries are encoded. This containment is what keeps the per-claim verifier circuits small.

UTXO is the largest sub-tree and the only one with a variable-size leaf encoding. The variability comes from native-asset bundles: a UTxO holding only ADA with no datum encodes in roughly 81 bytes, while a UTxO carrying many native assets grows linearly because each `(asset_id, quantity)` pair consumes an additional 2 + variable + 8 bytes. The leaf format was extended in v0.9.0 to include the asset bundle, then patched in v0.9.1 to actually preserve the assets through the parser. The v0.9.0 implementation parsed and discarded them, an audit-found bug. The other six sub-trees have fixed-size leaves, which simplifies the construction loop.

Header chain and transaction index were the obvious early sub-trees because their leaf shapes are completely determined by block-level data and have no dependency on ledger state. Header is `(slot, block_height, block_hash, prev_hash)` packed into 80 bytes. Transaction index is `(tx_id, slot, block_hash, tx_position)` packed into 76. Both formats were locked at v0.2.0 and v0.3.0 respectively and have not changed since. Reproducing them on a second implementation is straightforward because the only ambiguity is endianness, which the spec pins to big-endian throughout.

Stake state and governance state are the trickiest because they depend on Cardano-era-specific ledger semantics. Stake is `(stake_credential_hash, delegated_pool, delegated_drep, rewards_lovelace, is_pool_operator)` for each credential, but "active stake" has a specific meaning that depends on which snapshot in the rolling Mark/Set/Go triplet you are reading. The v1.0 plan pins this to the `pstakeSet` snapshot, which is the snapshot used for reward calculation in the current epoch. Governance is harder because Conway-era governance has heterogeneous facts (treasury balance, CC seats, ratified and in-flight gov actions) that share no common record shape. The leaf encoding handles this with a `kind` discriminant byte and a fixed-width `(key, value, slot)` payload whose interpretation depends on the kind.

Native token policies and the script registry are derived sub-trees in the v1.0 architecture. They are not produced by querying separate endpoints; they are computed during the same UTXO walk that populates the UTXO sub-tree. A token policy entry is created the first time the walker sees a UTxO containing that policy's tokens; total_supply is summed across the walk; first_issuance_slot is pinned to zero for v1.0 (the real value requires chain history that comes from the v1.1 chain-follower). The script registry is populated similarly from the optional reference-script hash on each UTxO. This dependency pattern is why Tasks 4, 5, and 6 in the v1.0 plan are tightly coupled: implementing them out of order produces parsers that cannot share work.

## The dual-track bundle root

```
   Per-sub-tree roots (Blake3 only)
         │
         ▼
  ┌──────────────────────────────────────────────┐
  │   bundle_root_blake3 = blake3(             │
  │     utxo_root || header_root || tx_root ||   │
  │     token_root || script_root || stake_root  │
  │     || gov_root)                             │
  │                                              │
  │   bundle_root_sha3 = sha3(same concatenation)│
  └──────────────────────────────────────────────┘
         │
         ▼
   Ω-Commitment = (bundle_root_blake3, bundle_root_sha3)
```

The dual-hash bundle root is the single most consequential design decision in the program after the choice to do a clean-slate fork at all. The naive single-hash design publishes one root, and any future preimage or collision attack on that hash function compromises the soundness of every claim proof ever submitted against the commitment. With ten million UTxOs and eight years of public exposure to whatever cryptanalysis gets developed in the next two decades, that is too much surface area to bet on a single hash function holding indefinitely.

The fully-symmetric dual-hash design hashes everything twice, including every leaf and every internal Merkle node. That works but doubles the storage cost of the published commitment and doubles the verifier circuit cost, both of which scale with the entire UTxO set rather than with the number of claims. The cost-benefit math came out badly: doubling verifier work to defend against a hash break that, if it ever happens, would more likely produce a global protocol pause than a stealthy individual attack.

The selective dual-hash compromise applies the second hash function only at the bundle layer. The seven per-sub-tree roots are computed with Blake3 only (under the v2 domain tags above). Then both Blake3 and SHA3 hash the seven-root concatenation, yielding two bundle roots published as a tuple. A claim-proof verifier checks the Blake3 membership path against `bundle_root_blake3` (cheap, plonky3-friendly). The SHA3 root is the drift-detection signal flagged in the 2026-05-03 audit reframing: divergence between the two bundle roots indicates the aggregation step computed two inconsistent answers, not that Blake3 broke. A truly independent SHA3 tree (rebuilding all seven sub-trees with SHA3 leaves and SHA3 internal nodes) is tracked as a v2.0 follow-up; if Blake3 is later broken, the protocol's path forward is a coordinated re-commitment over public Cardano chain history rather than a drop-in swap to today's SHA3 root.

The decision to put the dual-hash at the bundle layer rather than the leaf layer was contested during the spec discussion. The argument for leaf-level dual-hash: an attacker who only controls the bundle layer can swap one full sub-tree root for a colliding one, while leaf-level dual-hash forces the attacker to collide every individual leaf as well. The counterargument, which won: the seven sub-tree roots are fixed-size, well-distributed inputs to the bundle hash, and a collision attack against the bundle-layer hash on those specific inputs is harder to mount than a collision against arbitrary leaves. The full reasoning is in `cardano-wiki/docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md` (summarized in the 2026-05-01 wiki log entry).

The Ω-Commitment published in Omega's genesis block is a 64-byte tuple: 32 bytes of Blake3 output followed by 32 bytes of SHA3 output. Anybody with the published commitment, the public Cardano chain history, and one or both parsing implementations can independently reconstruct the per-sub-tree leaf sets and verify they hash to the published roots. Independent reproducibility is the audit precondition for using the commitment at all: at least one second implementation must exist and produce byte-identical roots before genesis. The requirement is why the v0.9.x test suite leans heavily on golden vectors at three layers (per-leaf encoding, per-sub-tree root, bundle root tuple).

## Consensus stack: Crypsinous + Chronos + Minotaur, all post-quantum

The consensus layer composes three Ouroboros papers, each updated for the post-quantum primitive set above. All three descend from the same Ouroboros family, share the universal-composability framework, and compose without new soundness proofs. The engineering work is consistent PQ-primitive substitution across all three, not new theorem-proving.

```
   Crypsinous           Chronos            Minotaur
   (privacy)            (time)             (multi-resource)
        │                   │                    │
   shielded VRF        permissionless      stake + storage
   shielded stake      PoS clock           + future resources
   shielded rewards    sub-protocol        ω-weighted security
        │                   │                    │
        └───────────────────┼────────────────────┘
                            ▼
                  Composite Ouroboros-Omega
                  consensus protocol, all PQ
```

### Crypsinous (privacy)

Ouroboros Crypsinous (Kerber, Kiayias, Kohlweiss, Zikas, [eprint 2018/1132](https://eprint.iacr.org/2018/1132)) is a privacy-preserving variant of Praos. It shields the VRF outputs that drive slot-leader election, the stake amounts that weight that election, and the reward flows that pay validators. An external observer cannot tell who is producing blocks, who controls how much stake, or who is receiving rewards.

The original construction uses a curve-based zk-SNARK (Groth16 over BLS12-381) and a curve VRF. Both are replaced. The PQ-Crypsinous variant uses Plonky3 STARKs in place of Groth16, a hash-based VRF proven inside a STARK in place of the curve VRF, hash-based threshold aggregation (the leanXMSS / leanMultisig family) in place of BLS, and Poseidon2 commitments inside circuits in place of Pedersen. The primitive set matches the §1 mandates exactly; the original paper's UC-framework security proof carries over by primitive substitution.

The encrypted mempool falls out of Crypsinous naturally rather than being bolted on. The same per-epoch stake-weighted threshold-encryption committee that Crypsinous needs to shield its consensus inputs also handles claim-transaction mempool decryption. Validators commit to ordering before they can decrypt the payload, which closes the OFAC-validator-censorship attack, the recipient-substitution front-running attack, the mempool-surveillance attack, and the validator-reordering MEV attack in one mechanism.

### Chronos (time)

Ouroboros Chronos (Badertscher, Gaži, Kiayias, Russell, Zikas, [eprint 2019/838](https://eprint.iacr.org/2019/838)) is a permissionless proof-of-stake clock-synchronization protocol. From the abstract: *"we obtain a permissionless PoS implementation of a global clock that may be used by higher level protocols that need access to global time."* Chronos requires only that joining parties have local clocks advancing at approximately the same speed; the chain itself synthesises the global clock that Praos and Crypsinous would otherwise have to assume.

Why the design needs it: Praos and Crypsinous both inherit a synchrony assumption that joining parties already share a common notion of round and slot time. State-actor pressure on NTP infrastructure is a documented attack against PoS validators that depend on external time sources. A clean-slate chain that begins from a mass multi-party-computation ceremony rather than a trusted-time-server cannot afford the external-clock dependency. Chronos replaces it with a synchroniser sub-protocol baked into consensus.

The composition with Crypsinous is clean: shared VRF, shared stake snapshot, additional sub-protocol that re-aligns joining parties' local clocks within a few rounds. The threshold-encryption committee that decrypts the encrypted mempool is the same committee whose epoch boundaries Chronos pins. No new committee, no new key. The hash-based VRF construction Chronos depends on is the same one Crypsinous depends on, which is also the same one tracked as the load-bearing open research question in `RESEARCH-QUESTIONS.md`.

### Minotaur (multi-resource)

Minotaur (Fitzi, Wang, Kannan, Kiayias, Leonardos, Viswanath, Wang, [eprint 2022/104](https://eprint.iacr.org/2022/104)) generalises consensus to combine multiple resource types in a single longest-chain protocol. From the abstract: *"a multi-resource blockchain consensus protocol that combines proof of work (PoW) and proof-of-stake (PoS), and we prove it optimally fungible... we generalize Minotaur to any number of resources."*

The security inequality is `ω · β_w + (1−ω) · β_s < 1/2` for any weighting `ω ∈ [0, 1]`, where `β_w` is the adversarial fraction of one resource and `β_s` the adversarial fraction of the other. The honest majority must hold across the *union* of resource pools, not in any single resource alone. An attacker who captures 60% of stake but only 20% of the second resource does not break the chain; they need cumulative honest minority across the combined input.

For Omega, the second resource is storage. The mirror partnerchain (next section) provides verifiable proof-of-space-time over the snapshot archive; its storage providers earn consensus weight on Omega via Minotaur's multi-resource composition. Capturing Omega's consensus now requires capturing both ωADA stake and a meaningful fraction of the global storage market simultaneously. Two distinct attack surfaces with different cost structures: stake capture goes through capital markets, storage capture goes through data-centre buildout. Minotaur's "any number of resources" property leaves room for adding proof-of-work, proof-of-bandwidth, or proof-of-uptime as future resources via a CIP-1694-shaped governance vote.

## Starstream as the native UTXO + zkVM layer

[LFDT-Nightstream/Starstream](https://github.com/LFDT-Nightstream/Starstream) is the smart-contract execution model for Omega and the destination shape of every `claim_utxo` output. UTXO-based with coroutines as the core primitive, native folding scheme, compiles to WebAssembly, off-chain execution sealed in succinct proofs verified on-chain. It is hosted under [LFDT-Nightstream](https://github.com/LFDT-Nightstream) (Linux Foundation Decentralized Trust, formerly Hyperledger), open-source, no single-vendor capture.

Three properties make Starstream a load-bearing choice rather than a swap-out option. First, the primitive set matches: Goldilocks field plus Poseidon2 hash, exactly what §1 mandates for in-circuit operations. The same hash that builds the Ω-Commitment Merkle tree commits Starstream state. No translation layer between layers, one circuit not two. Second, the UTXO-based model preserves the EUTXO mental continuity from Cardano. Holders and dApp developers do not need to relearn an account-based mental model; the leaf encoding of pre-fork Cardano UTxOs translates naturally into Starstream UTxOs at claim time. Third, coroutines provide the multi-step claim primitives natively. Atomic-bundle claims, time-locked claims, dead-man's-switch claims, m-of-n trustee resurrection, oracle-gated claims — all expressible as Starstream coroutines.

The verifier circuit emits a Starstream UTxO `(coroutine_id, amount, datum, recipient_view_key)` as its public output, replacing the generic shielded-note pattern. `claim_script` becomes "submit a Starstream coroutine that produces the equivalent script-hash" rather than the foundation-arbitrated dispute-window pattern. Multiple claims by the same holder (claim_utxo + claim_stake + claim_governance for the same credential) fold into a single recursive proof via Starstream's folding scheme. The on-chain footprint is constant regardless of how many sub-trees the holder is claiming from.

What Starstream does not solve is orthogonal. The hash-based VRF construction is in Crypsinous-Chronos's scope, not Starstream's. The lattice-vs-hash signature decision is at the ordinary-transaction layer, not the smart-contract layer. The mass-MPC genesis ceremony is the pre-fork commitment work, separate from the post-claim execution layer. Each open question lives in its own track.

Upstream maturity tracks Omega's program timeline rather than blocking it. Per the upstream [impl-plan.md](https://github.com/LFDT-Nightstream/Starstream/blob/main/impl-plan.md), the compiler, interpreter, and WebAssembly target are shipping; type checker, IVC, MCC, and lookups modules are marked TODO. Track T6 (Omega's verifier) and Track T3 (smart-contract VM) depend on these landing. The T1 commitment-tooling work in this repository is unchanged by Starstream; the integration happens at T3 and T6 in parallel.

## Mirror partnerchain (forked Filecoin)

The §6.3 storage-proof bounty alone funds replication; it does not provide a market for retrieval. A holder claiming in 2046 needs not just "the data exists somewhere" but "the data is fetchable from someone right now at predictable cost." For that the design runs a separate partnerchain — a fork of [Filecoin](https://github.com/filecoin-project) — under the [Cardano partnerchains SDK](https://github.com/input-output-hk/partner-chains). The mirror partnerchain is the same architectural pattern Midnight uses against Cardano today.

The fork is not adoption-as-is. Filecoin's existing mainnet uses ECDSA, BLS12-381, and Groth16 — none post-quantum. The Omega mirror replaces all curve cryptography with the §1 PQ stack: SLH-DSA / ML-DSA / FN-DSA for signatures, hash-based threshold aggregation in place of BLS, Plonky3 STARKs in place of Groth16, Blake3/SHA3/Poseidon2 in place of legacy hash choices. Filecoin's storage proofs (PoRep, PoSt, the Window-PoSt time-bounded variant) are already hash-and-Merkle-based; porting them to the new hash family is mechanical. The economic model survives unchanged: storage providers post bonds, sealing creates verifiable commitments, periodic spacetime proofs maintain liveness. Approximate engineering work is six to twelve months, comparable to Filecoin's original mainnet launch from spec.

The partnerchain coupling is what makes the mirror more than just an archive. Storage providers earn double revenue: Filecoin-style retrieval fees paid by holders who request data, plus Omega-side block rewards paid by the protocol treasury via the partnerchain SDK. The mirror chain's own consensus is itself Minotaur-shaped, with proof-of-space-time as the dominant resource and a small proof-of-stake slice for liveness. Storage providers on the mirror chain are also a Minotaur-input to Omega's main consensus, providing the storage-resource diversification that the previous section calls for.

The mirror partnerchain is optional infrastructure. Omega's correctness does not depend on it. Holders who keep their own data still claim directly without ever touching the mirror. The chain-level commitment plus claim-resolution machinery is exactly what was specified in the prior version of this document; the mirror is one of many possible providers of those proofs, not a privileged operator. The §6.3 bounty rewards anyone who proves possession of any chunk; the mirror partnerchain is the most natural earner of that bounty at scale, but anyone can earn it.

What the mirror partnerchain is not is as important as what it is. It is not a privileged operator. Anyone can run a storage provider, jurisdiction is by economic incentive not by designation. It is not a censorship surface — Omega's claim verifier never reads from the mirror, only from holder-submitted proofs. It is not a single point of failure — if the mirror chain fails entirely, holders who kept their own data are unaffected; holders who relied on the mirror lose convenience of cheap retrieval but their claim rights are preserved. It is not a regulator-friendly disclosure mechanism — it stores public data (the Cardano-era snapshot is already public on Cardano), retrieval is permissionless, and storage providers cannot selectively withhold without losing their bond. Privacy of post-claim Omega state still flows through Crypsinous and Starstream and holder-controlled viewing keys.

## Lazy / pull-based resurrection

Omega's genesis ledger does not pre-load eight years of Cardano state. Genesis is essentially empty: the block-zero state contains a small set of bootstrap parameters, the validator keys for the first epoch, and the published Ω-Commitment. Every UTxO, every stake position, every native token policy, every governance role from the Cardano era exists only as a Merkle leaf inside the published commitment until somebody claims it.

Each claim is a transaction with two parts: a Merkle membership proof against the Ω-Commitment, and a witness that the holder controls the credential being claimed. The verifier inside Omega's ledger checks both, then either accepts the claim and credits the resulting state, or rejects it and burns the fee. No trusted intermediary. No committee that decides which claims are valid. The only trust assumptions are cryptographic (the hash functions hold, the witness scheme is unforgeable) and snapshot correctness (the published commitment is what an honest second-implementation would have computed).

| Claim type | Proves | Resurrects |
|---|---|---|
| `claim_utxo` | UTxO existed at snapshot | ADA + native tokens at the same address |
| `claim_token_policy` | Policy was minted on Cardano | Same policy ID can mint on Omega |
| `claim_script` | Script was registered on Cardano | Same script hash callable on Omega |
| `claim_stake` | Credential was delegated to pool X | Migrate stake position to Omega's analogous pool |
| `claim_governance` | Held DRep / committee role | Same role on Omega's governance |
| `claim_header` | Block existed at slot S | (Reserved — chain-anchored protocols only) |
| `claim_tx` | Transaction existed | (Reserved — tx-anchored protocols only) |

The obvious upside is storage cost. Cardano's UTxO set has a long tail of dust addresses that nobody will ever claim because the gas to claim them exceeds their value. In an eager-loading design, every Omega validator carries those dust UTxOs forever. In the lazy design, they cost nothing on Omega until somebody claims them, and they will not be claimed because they are dust. The unclaimed long tail simply does not migrate. Acceptable outcome.

The less-obvious upside: the lazy model gives a natural cleanup mechanism for bridge-related disputes. If a UTxO turns out to have been compromised on Cardano (key theft, lost seed, exchange-custody dispute), the on-Omega resolution is whoever submits a valid claim first. There is no replay-after-the-fact risk because each credential can only be claimed once. The on-Cardano dispute plays out separately. Omega's state machine treats the claim as final regardless. This mirrors how Bitcoin treats UTxO ownership and avoids the chain-of-precedent problems that have made Ethereum's contract-account model expensive to migrate.

The downside is operational. Holders need wallets that know how to construct claim transactions, infrastructure to fetch Merkle paths from the published snapshot, and patience for the verifier circuit to verify each proof on-chain. Track T8 (tooling) and Track T6 (verifier) are gated by this. The plan is to ship reference wallets and a snapshot-API service alongside Omega's mainnet launch so the first wave of holders has something to use. That is downstream of the work in this repository; T1 only produces the commitment.

## v1.0 ingestion pipeline

The Ω-Commitment is computed once, at the snapshot boundary chosen by the program. Producing it requires real Cardano mainnet state. The original v1.0 plan (2026-05-01) assumed a single CBOR dump of the full LedgerState produced by `cardano-cli query ledger-state --output-cbor`. That command does not exist in cardano-cli 10.16; supported formats are JSON, text, and YAML. The 2026-05-03 architecture revision in [`cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`](./cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md) reworked the pipeline into the two-stream model now being implemented.

The source of all input data is a standard headless `cardano-node 10.7.1` synced to mainnet via Mithril fast-bootstrap. `mithril-client 2617.0` downloads a 217 GiB compressed snapshot of the chain DB from the mainnet aggregator, verifies the Mithril certificate against the published genesis verification keys, restores it to local disk (about 218 GB on disk after extraction), and the node finishes syncing within minutes of restore completion. The full runbook is at [`omega-commitment/scripts/setup_headless_node.md`](./omega-commitment/scripts/setup_headless_node.md). The host is a 122 GiB RAM box, comfortable for any of the in-memory operations downstream.

Stream one is a single JSON dump. `cardano-cli conway query ledger-state --mainnet --out-file ledger_state_<TS>.json` produces about 2 GiB of JSON. The cli scrubs the `utxoState.utxo` field to `{}` on mainnet and offers no way to recover it through this code path. Everything else is intact and verified live: 1,474,666 stake accounts, 2,940 stake pools, 1,016 DReps, 2,499,064 stake credentials, three rolling snapshots of roughly 1.32 million activeStake entries each, full governance state including all current proposals with attached vote tallies. Both the stake and governance sub-trees parse from this single file. The path map is in [`cardano-wiki/wiki/pages/ledger-state-json-layout.md`](./cardano-wiki/wiki/pages/ledger-state-json-layout.md).

Stream two is a custom binary called `omega-utxo-snapshot`, built 2026-05-03 to recover from the cardano-cli `--whole-utxo` failure. The cli's `query utxo --whole-utxo --output-cbor-bin` invocation is the natural choice for the UTxO sub-tree input, but it dies on mainnet after consuming roughly 978 MB of the response stream with `DeserialiseFailure "Decoding TxIx: More than 16bits was supplied"`. The root cause is in `Cardano/Ledger/Address.hs:847`: the decoder reads pointer-address transaction indices via `decodeVariableLengthWord16` while the encoder at line 348 writes variable-length Word64. Mainnet's historical record contains pointer-address TxOuts whose TxIx exceeds 16 bits. PR `IntersectMBO/cardano-cli#1350` carries the hotfix and has been open since March 2026 without merge.

The `omega-utxo-snapshot` binary is a 127-line Rust program that uses `pallas-network 0.30.2`'s local-state-query miniprotocol to issue the same `BlockQuery::GetUTxOWhole` query the cli would have issued. Pallas's CBOR decoder does not share Haskell's 16-bit TxIx asymmetry, so the response decodes cleanly. The wire-format match against ouroboros-consensus was verified layer-by-layer 2026-05-03: pallas's encoded bytes for the query are `82 03 82 00 82 00 82 06 81 07`, byte-identical to what the Haskell stack expects (LSQ MsgQuery wrapping QueryIfCurrent wrapping the era-NS wrapping the BlockQuery tag). The output file is bit-identical to what a fixed cardano-cli with PR #1350 applied would produce. UTXO, native token policies, and scripts all derive from this one file. Details are in [`cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md`](./cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md).

## Tracks beyond commitment-tooling

The seven-sub-tree commitment is track T1 of a twelve-track program. The other eleven tracks are scoped in [`cardano-wiki/wiki/pages/spec-ouroboros-omega.md`](./cardano-wiki/wiki/pages/spec-ouroboros-omega.md) and listed in [`GOALS.md`](./GOALS.md). The relationship between tracks matters because most have hard dependencies on T1. Without a stable Ω-Commitment format, the verifier circuit (T6) cannot be specified. Without the verifier, the bridge protocol (T7) is hand-waving. Without the bridge protocol, the wallet tooling (T8) has nothing to construct. T1 gates roughly half of the program even though it is the smallest track by line count.

Track T2 (consensus) is the [Crypsinous](https://eprint.iacr.org/2018/1132) + [Chronos](https://eprint.iacr.org/2019/838) + [Minotaur](https://eprint.iacr.org/2022/104) composite documented in the consensus-stack section above. All three papers descend from Ouroboros and share the universal-composability framework, so the composition is a primitive-substitution exercise rather than a new theorem-proving exercise. The hash-based VRF construction is the load-bearing open research question that gates all three.

Track T3 (smart-contract VM) is [LFDT-Nightstream/Starstream](https://github.com/LFDT-Nightstream/Starstream), adopted upstream and extended with PQ-Crypsinous shielding hooks. The Goldilocks field plus Poseidon2 primitive set lines up exactly with §1's mandates. Starstream's coroutine model provides the atomic-bundle and time-locked claim primitives natively. Track T3 was previously open scope; it now has a concrete upstream dependency and a clear scope boundary at the Plutus → Starstream translation question.

Track T4 (network stack) is the Ouroboros networking miniprotocols ported to Omega's primitive set, with PQ-handshake variants of the noise-style transport encryption. The interesting design questions are around backwards compatibility: do Omega nodes speak any Cardano-flavored protocol versions for migration tooling, or is the wire incompatibility total? Not started.

Track T5 (storage and mirror partnerchain) is the on-disk layout for Omega's UTxO state and block storage, plus the [Filecoin-fork mirror partnerchain](https://github.com/filecoin-project) under the [Cardano partnerchains SDK](https://github.com/input-output-hk/partner-chains). Storage providers earn double revenue (Filecoin retrieval fees plus Omega-side block rewards) and provide the storage-resource input to Minotaur consensus. The mirror chain is optional infrastructure: Omega's correctness does not depend on it.

Tracks T9 (documentation), T10 (audits), T11 (testnet operations), and T12 (mainnet operations) are the rollout machinery. T9 is the whitepaper plus the formal protocol spec at the level of detail auditors and second-implementation teams need. T10 is the audit pass over the cryptographic primitives, the bridge protocol, and the consensus protocol, plus machine-checked proofs of the most critical invariants in Lean or Coq. T11 is the lifecycle of devnet, internal testnet, and public testnet. T12 is the genesis ceremony, key rollout, validator onboarding, and the claim-window rollout schedule. Each has a long tail of operational work that does not block downstream tracks but does block the ultimate launch.

The scope is large. The repository you are reading is roughly 1% of the total work. What it does, and what it must do correctly before anything else can proceed, is produce a reproducibly-verifiable Ω-Commitment from real Cardano mainnet state. Everything else either sits downstream of that or runs in parallel under the assumption that this part lands correctly. The next milestone is the v1.0 real-data golden vector (5 of 7 sub-trees on real mainnet, 2 placeholders). After that is v1.1 (chain-follower fills in the placeholders, complete 7-of-7 root tuple). Then a second implementation, then audits, then everything else. The road is long. But it is now mapped, and the first kilometer is paved.
