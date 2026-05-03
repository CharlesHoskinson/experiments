# Architecture

This document explains the design of the Ω-Commitment: what it captures, how it is structured, and how the v1.0 mainnet ingestion pipeline produces one. It is the deep-dive companion to [`README.md`](./README.md), which frames the program at a higher level, and to [`GOALS.md`](./GOALS.md), which lists what we are trying to accomplish across all twelve tracks.

## Cryptographic primitives

Every primitive used in the Ω-Commitment construction is post-quantum. No curve operations anywhere in the build, the verify, or the leaf encoding. This is the load-bearing constraint of the whole program. If a single curve hash, signature, or commitment crept into the construction, an attacker with a sufficiently large quantum computer could forge a Merkle membership proof against the published root, and the bridge from Cardano would no longer be sound. The design starts from the constraint and works backward.

The hash functions are Blake2b-256 and SHA3-256. Blake2b is the workhorse: it computes every leaf hash and every per-sub-tree root, and is broadly faster than SHA-2 on 64-bit hardware while remaining trivial to constrain inside a STARK circuit. SHA3 is paired with Blake2b only at the bundle layer, the single hash above the seven sub-tree roots. The cost of two hashes there is 32 extra bytes of output and one extra hash over a 224-byte concatenation. The benefit is a hedge: if Blake2b falls to a future preimage attack, the SHA3 root still anchors the commitment, and vice versa. The tradeoff was adopted in the 2026-05-01 dual-hash decision (`cardano-wiki/docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md`, summarized in the wiki log).

The Merkle tree is binary, fixed-arity, and zero-padded to the next power of two. Arity matters because variable-arity trees (for instance, the Patricia trie used by Ethereum) require the prover to handle a much wider set of node shapes inside the verifier circuit. Fixed arity collapses the verifier into a tight loop. Zero-padding handles the case where the leaf set is not already a power of two without requiring the verifier to know the original leaf count, which would otherwise leak snapshot height. The cost is a small amount of extra hashing for non-power-of-two snapshots, paid once at construction time.

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
   Per-sub-tree roots (Blake2b only)
         │
         ▼
  ┌──────────────────────────────────────────────┐
  │   bundle_root_blake2b = blake2b(             │
  │     utxo_root || header_root || tx_root ||   │
  │     token_root || script_root || stake_root  │
  │     || gov_root)                             │
  │                                              │
  │   bundle_root_sha3 = sha3(same concatenation)│
  └──────────────────────────────────────────────┘
         │
         ▼
   Ω-Commitment = (bundle_root_blake2b, bundle_root_sha3)
```

The dual-hash bundle root is the single most consequential design decision in the program after the choice to do a clean-slate fork at all. The naive single-hash design publishes one root, and any future preimage or collision attack on that hash function compromises the soundness of every claim proof ever submitted against the commitment. With ten million UTxOs and eight years of public exposure to whatever cryptanalysis gets developed in the next two decades, that is too much surface area to bet on a single hash function holding indefinitely.

The fully-symmetric dual-hash design hashes everything twice, including every leaf and every internal Merkle node. That works but doubles the storage cost of the published commitment and doubles the verifier circuit cost, both of which scale with the entire UTxO set rather than with the number of claims. The cost-benefit math came out badly: doubling verifier work to defend against a hash break that, if it ever happens, would more likely produce a global protocol pause than a stealthy individual attack.

The selective dual-hash compromise applies the second hash function only at the bundle layer. The seven per-sub-tree roots are computed with Blake2b only. Then both Blake2b and SHA3 hash the seven-root concatenation, yielding two bundle roots published as a tuple. A claim-proof verifier checks the Blake2b membership path against `bundle_root_blake2b` (cheap, plonky3-friendly). The SHA3 root is the contingency: if Blake2b is later broken, the protocol can hard-fork to require Merkle paths matched against `bundle_root_sha3` instead, and re-derivable sub-tree roots can be regenerated from public Cardano chain history without re-running the whole snapshot ceremony.

The decision to put the dual-hash at the bundle layer rather than the leaf layer was contested during the spec discussion. The argument for leaf-level dual-hash: an attacker who only controls the bundle layer can swap one full sub-tree root for a colliding one, while leaf-level dual-hash forces the attacker to collide every individual leaf as well. The counterargument, which won: the seven sub-tree roots are fixed-size, well-distributed inputs to the bundle hash, and a collision attack against the bundle-layer hash on those specific inputs is harder to mount than a collision against arbitrary leaves. The full reasoning is in `cardano-wiki/docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md` (summarized in the 2026-05-01 wiki log entry).

The Ω-Commitment published in Omega's genesis block is a 64-byte tuple: 32 bytes of Blake2b output followed by 32 bytes of SHA3 output. Anybody with the published commitment, the public Cardano chain history, and one or both parsing implementations can independently reconstruct the per-sub-tree leaf sets and verify they hash to the published roots. Independent reproducibility is the audit precondition for using the commitment at all: at least one second implementation must exist and produce byte-identical roots before genesis. The requirement is why the v0.9.x test suite leans heavily on golden vectors at three layers (per-leaf encoding, per-sub-tree root, bundle root tuple).

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

Tracks T2 (consensus), T3 (smart-contract VM), and T4 (network stack) define what Omega looks like as a chain. None have hard dependencies on T1 other than the choice of cryptographic primitives, which is a policy decision the spec already encodes. They can in principle proceed in parallel. The risk is the integration phase: a consensus protocol that assumes one set of primitives and a smart-contract VM that assumes another will not compose without rework. The current draft assumes a shared set: hash-based VRFs and signatures from the same primitive family throughout.

Track T5 (storage) is the engineering work that makes the chain usable at production load. Cardano has been rebuilding its on-disk format for years, and the LSM and V2InMemory backends are the latest iteration. Omega's storage layer needs to serve two query patterns: the conventional "what is the state at slot N" query that wallets and explorers issue, and the "give me a Merkle proof against the committed sub-tree at this slot" query that the bridge verifier issues. The latter is unusual and benefits from a custom layout. T5 is mostly downstream of T2 and T3 settling.

Tracks T9 (documentation), T10 (audits), T11 (testnet operations), and T12 (mainnet operations) are the rollout machinery. T9 is the whitepaper plus the formal protocol spec at the level of detail auditors and second-implementation teams need. T10 is the audit pass over the cryptographic primitives, the bridge protocol, and the consensus protocol, plus machine-checked proofs of the most critical invariants in Lean or Coq. T11 is the lifecycle of devnet, internal testnet, and public testnet. T12 is the genesis ceremony, key rollout, validator onboarding, and the claim-window rollout schedule. Each has a long tail of operational work that does not block downstream tracks but does block the ultimate launch.

The scope is large. The repository you are reading is roughly 1% of the total work. What it does, and what it must do correctly before anything else can proceed, is produce a reproducibly-verifiable Ω-Commitment from real Cardano mainnet state. Everything else either sits downstream of that or runs in parallel under the assumption that this part lands correctly. The next milestone is the v1.0 real-data golden vector (5 of 7 sub-trees on real mainnet, 2 placeholders). After that is v1.1 (chain-follower fills in the placeholders, complete 7-of-7 root tuple). Then a second implementation, then audits, then everything else. The road is long. But it is now mapped, and the first kilometer is paved.
