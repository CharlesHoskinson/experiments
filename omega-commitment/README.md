# omega-commitment

Reference tooling for the Ouroboros Omega Ω-Commitment.

This crate computes one of the seven sub-trees: the **UTXO set Merkle root**.
Subsequent crates will compute the remaining six (block headers, tx index,
token policies, script registry, stake state, governance state).

See `docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md` for design context.

## Performance baseline

| UTXOs   | Time/build       | Throughput     |
|---------|------------------|----------------|
| 1,000   | 124.97 µs        | 8.00 Melem/s   |
| 10,000  | 2.55 ms          | 3.92 Melem/s   |
| 100,000 | 19.47 ms         | 5.14 Melem/s   |

10M-UTXO mainnet target: extrapolate × 100 from 100k baseline.
If extrapolation > 60s, optimize Task 9.

## Optimization status

The 100k-UTXO benchmark extrapolates to ~1.95s for 10M UTXOs, well within the 60-second budget for production use. **No optimization required** for the v0.1.0 sub-tree generator; revisit if the underlying Cardano UTXO set grows substantially or if the recursive STARK proving cluster needs sub-second build times for incremental snapshots.

## Public API contract

This crate produces the **UTXO sub-tree** of the Ω-Commitment.
The output `commitment.json` is one of seven inputs to the final Ω-Commitment.

Downstream consumers:
- Plonky3 `claim_utxo` circuit (track T2) — uses the witness format from `witness.rs`.
- omega-node (track T6) — verifies `claim_utxo` proofs against the published root.

Next plan in this track: `2026-XX-XX-omega-block-header-accumulator-plan.md`
(builds the second sub-tree).

## Determinism guarantees

- Identical input UTXO set → identical commitment root, on any platform.
- Property test in `leaf::tests::encoding_is_pure` enforces this.
- Integration test in `tests/integration.rs` checks pipeline determinism.
- Full mainnet-snapshot regression test will be added in the next plan.

## Hash & encoding versions

- v0.1.0: Blake2b-256 + SHA3-256 dual-track hashing (see `hash.rs`)
- v0.1.0: Canonical UTXO leaf encoding (see `leaf.rs` module docstring)
- v0.1.0: Plonky3-friendly binary Merkle tree, sort-then-pad to power of two (see `tree.rs`)
- v0.1.0: Inclusion witnesses with hex-encoded sibling list (see `witness.rs`)

Any change to leaf encoding, hash, or tree layout MUST bump the major version
and require a new commitment generation against a known fixture.

## Crate layout

- `omega-commitment-core` — pure library: hashing, leaf encoding, Merkle tree, witnesses
- `omega-commitment-cli` — `omega-commitment` binary wrapping the core; `commit` subcommand

## Build & test

```bash
cargo build --release --workspace
cargo test --workspace          # 26 tests
cargo bench -p omega-commitment-core   # criterion benchmarks
```

## Usage

```bash
omega-commitment commit \
  --input path/to/utxos.json \
  --output ./out

# Outputs:
#   ./out/commitment.json  — root + leaf_count + tree_depth + utxo_count
#   ./out/witnesses/<leaf_hash>.json  — one per UTXO
```

## Resolved: dual-track shadow hash (decided 2026-05-01)

The Ouroboros Omega design specifies dual-track hashing — Blake2b-256 primary, SHA3-256 shadow — with the language "both must be checked by verifiers."

The decision (`docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md`) is **Option 3 — selective dual-track**:

- **Per-leaf hashing:** Blake2b-256 only.
- **Per-sub-tree Merkle root:** Blake2b-256 only.
- **Ω-Commitment bundle root** (the artifact attested by Mithril-PQ + recursive STARK + CIP-1694): published as a tuple `(blake2b_bundle_root, sha3_bundle_root)`. Both must be verified by anyone consuming the canonical Ω-Commitment.
- **Plonky3 claim circuits:** verify single-track Blake2b Merkle paths. The dual-track lives one layer above the claim layer.

This is what the per-sub-tree tooling in this repo (omega-commitment-core / omega-commitment-cli) ships: Blake2b-only roots. Bundle-assembly tooling (a future plan) will compute both Blake2b and SHA3 sub-tree roots from the same input on demand, then aggregate into the dual-track bundle root.

**Plonky3 circuit authors are NOW unblocked** to lock to v0.4.0+ single-track Blake2b root format.

If Blake2b is ever broken, the migration to full dual-track is mechanical via the existing `hash::dual_hash` primitive — see the decision document for the migration ramp.

## v0.2.0 — Block header sub-tree

Adds the second of seven Ω-Commitment sub-trees: the block header chain.

### Breaking changes from v0.1.0

- **Module rename:** `omega_commitment_core::leaf` → `omega_commitment_core::utxo_leaf`. Update imports.
- **CLI argument added:** `commit` now accepts `--sub-tree {utxo|header}` (default: `utxo`). Existing v0.1.0 invocations remain valid because the default preserves prior behavior.
- **CommitmentRecord schema changed:**
  - Renamed `utxo_count` → `item_count` (now sub-tree agnostic).
  - Added `sub_tree` field (`"utxo"` or `"header"`).
  - Added `input_digest` field (Blake2b-256 of raw input bytes — lets consumers confirm provenance).

### Header sub-tree usage

```bash
omega-commitment commit \
  --sub-tree header \
  --input path/to/headers.json \
  --output ./out
```

Header input JSON shape:

```json
{
  "headers": [
    {
      "slot": 1,
      "block_height": 1,
      "block_hash": "<64 hex chars>",
      "prev_hash": "<64 hex chars>"
    }
  ]
}
```

### Header leaf encoding

```
slot (u64 BE) || block_height (u64 BE) || block_hash (32 bytes) || prev_hash (32 bytes)
```

Total: 80 bytes per header, fixed-width. Hashed with Blake2b-256.

### Optional chain-link validation

`omega_commitment_core::header_leaf::validate_chain_links(&[BlockHeader])` returns `Some(index_of_first_failure)` if the chain has a non-monotonic slot or a `prev_hash` mismatch, else `None`. This is a sanity helper for callers; commitment generation does NOT require chain validity.

### Sub-trees status

| # | Sub-tree | Plan | Status |
|---|---|---|---|
| 1 | UTXO set | `2026-05-01-omega-utxo-commitment-plan.md` | Shipped (v0.1.0) |
| 2 | Block header chain | `2026-05-01-omega-block-header-accumulator-plan.md` | Shipped (v0.2.0) |
| 3 | Transaction index | TBD | Pending |
| 4 | Native token policies | TBD | Pending |
| 5 | Script registry | TBD | Pending |
| 6 | Stake state | TBD | Pending |
| 7 | Governance state | TBD | Pending |

## v0.3.0 — Transaction-index sub-tree

Adds the third of seven Ω-Commitment sub-trees: the transaction index. Powers `claim_tx` proofs that "tx H existed at slot S in block B at position P" — useful for legal compliance, RWA provenance, and audit trails.

### Breaking changes from v0.2.0

- **CLI argument value:** `--sub-tree` now accepts `tx-index` in addition to `utxo` and `header`.
- **`SubTree` JSON serialization** changed from `lowercase` to `kebab-case` rename rule. Single-word forms (`"utxo"`, `"header"`) are unchanged. The new `TxIndex` variant renders as `"tx-index"` (matches the CLI flag spelling).
- **`LeafError` is now `#[non_exhaustive]`** — downstream pattern matchers must include a wildcard arm.

### Tx-index sub-tree usage

```bash
omega-commitment commit \
  --sub-tree tx-index \
  --input path/to/tx_index.json \
  --output ./out
```

Tx-index input JSON shape:

```json
{
  "entries": [
    {
      "tx_id": "<64 hex chars>",
      "slot": 1,
      "block_hash": "<64 hex chars>",
      "tx_position": 0
    }
  ]
}
```

### Tx-index leaf encoding

```
tx_id (32 bytes) || slot (u64 BE) || block_hash (32 bytes) || tx_position (u32 BE)
```

Total: 76 bytes per entry, fixed-width. Hashed with Blake2b-256.

### Optional uniqueness validation

`omega_commitment_core::tx_index_leaf::validate_tx_uniqueness(&[TxIndexEntry])` returns `Some(index_of_first_duplicate)` if any `tx_id` appears more than once, else `None`. Sanity helper for callers; commitment generation does NOT require uniqueness.

### Sub-trees status

| # | Sub-tree | Plan | Status |
|---|---|---|---|
| 1 | UTXO set | `2026-05-01-omega-utxo-commitment-plan.md` | Shipped (v0.1.0) |
| 2 | Block header chain | `2026-05-01-omega-block-header-accumulator-plan.md` | Shipped (v0.2.0) |
| 3 | Transaction index | `2026-05-01-omega-tx-index-plan.md` | Shipped (v0.3.0) |
| 4 | Native token policies | TBD | Pending |
| 5 | Script registry | TBD | Pending |
| 6 | Stake state | TBD | Pending |
| 7 | Governance state | TBD | Pending |

### Hardening backlog (recommended before sub-tree 4)

Carry-over from v0.1.0 / v0.2.0 final reviews — operational hygiene items deferred to a hardening sprint between sub-tree 3 and sub-tree 4:
- Path traversal guards on `--input` / `--output`
- Input file size cap (e.g., `--max-input-bytes`)
- Atomic write of `commitment.json` (write-tempfile-then-rename)
- Layer cloning in `tree.rs::build` (perf at 10M-leaf scale)
- Hex codec deduplication (witness + utxo_leaf currently each have a small adapter)
- Clippy + rustfmt gating in CI
- CLI dispatcher refactor to trait/registry pattern (current 3-arm match scales to 4 arms with strain)
- Program-level: dual-track shadow hash decision before any Plonky3 circuit work locks

## v0.3.1 — Hardening sprint (no new sub-trees)

Closes the v0.1.0/v0.2.0 hardening backlog before sub-tree 4 lands. No new functionality, no breaking API changes (one minor behavior change: see "Path safety" below).

### Operational hygiene

- **Path safety:** CLI now canonicalizes `--input` (must exist) and `--output` (creatable). Witness files are guarded by a `safe_child` helper that refuses to write outside the canonicalized output dir. Today this is defense-in-depth (filenames are hex-only); it preempts a class of bugs as future sub-tree filenames could include richer data.
- **Input file size cap:** new `--max-input-bytes <BYTES>` flag, default 2 GiB. Prevents OOM on accidental large inputs.
- **Atomic write:** `commitment.json` is now written to a temp file and atomically renamed into place. A crash mid-run leaves no half-written commitment.

### Code quality

- **Hex serde adapters consolidated** into `omega_commitment_core::serde_helpers` (`hex_vec_hash` for `Vec<Hash>`, `opt_hex` for `Option<[u8; 32]>`). `utxo_leaf` and `witness` now use the shared module.
- **CLI dispatcher decomposed** into per-sub-tree free functions (`build_utxo_leaves`, `build_header_leaves`, `build_tx_index_leaves`). `commit()` is now ~30 lines.
- **`CommitmentRecord` field docs** clarify `leaf_count` (post-padding) vs. `item_count` (pre-padding).

### Performance

- **One redundant Vec clone per layer removed** from `MerkleTree::build`. No behavior change; smoke-tested via existing 256-leaf and integration tests, plus a new pinned-shape regression test.

### CI

- **`rust-toolchain.toml`** pins stable + clippy + rustfmt.
- **`.cargo/config.toml`** adds `cargo fmt-check` and `cargo lint` aliases.
- **`.github/workflows/ci.yml`** runs build + test + clippy (`-D warnings`) + fmt-check on every push and PR.

### Carried forward (still open)

- **Dual-track shadow hash** — was program-level pending decision when v0.3.1 was cut; **resolved on 2026-05-01** as Option 3 (selective dual-track). See `docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md` and the "Resolved: dual-track shadow hash" section above.

### Sub-trees status (unchanged)

| # | Sub-tree | Plan | Status |
|---|---|---|---|
| 1 | UTXO set | `2026-05-01-omega-utxo-commitment-plan.md` | Shipped (v0.1.0) |
| 2 | Block header chain | `2026-05-01-omega-block-header-accumulator-plan.md` | Shipped (v0.2.0) |
| 3 | Transaction index | `2026-05-01-omega-tx-index-plan.md` | Shipped (v0.3.0) |
| 4 | Native token policies | TBD | Pending |
| 5 | Script registry | TBD | Pending |
| 6 | Stake state | TBD | Pending |
| 7 | Governance state | TBD | Pending |

The dual-track shadow hash decision was resolved on 2026-05-01 (see decision doc); the next plan was sub-tree 4 (token policies, shipped as v0.4.0).

## v0.4.0 — Native token policy sub-tree

Adds the fourth of seven Ω-Commitment sub-trees: native token policies. Powers `claim_token_policy` proofs that "minting policy P existed on the old chain at slot S with total supply Q at fork height" — useful for stablecoin issuers (USDM, Djed, USDC bridge), NFT projects, and any project with a native-token brand to migrate.

### Breaking changes from v0.3.1

- **CLI argument value:** `--sub-tree` now accepts `token-policy` in addition to `utxo`, `header`, and `tx-index`.
- **`SubTree` enum is now `#[non_exhaustive]`.** Any external pattern matchers must include a `_ =>` arm. (Today there are no external consumers; this is a pre-emptive SemVer-safety addition.)

### Token policy sub-tree usage

```bash
omega-commitment commit \
  --sub-tree token-policy \
  --input path/to/token_policies.json \
  --output ./out
```

Token policy input JSON shape:

```json
{
  "policies": [
    {
      "policy_id": "<56 hex chars / 28 bytes>",
      "first_issuance_slot": 100,
      "total_supply_at_h": 1000000000
    }
  ]
}
```

### Token policy leaf encoding

```
policy_id (28 bytes) || first_issuance_slot (u64 BE) || total_supply_at_h (u128 BE)
```

Total: 52 bytes per policy, fixed-width. Hashed with Blake2b-256.

**Note on `policy_id` width:** Cardano native-token policy hashes are 28 bytes (Blake2b-224), not 32. This is the first cross-sub-tree asymmetry; verifiers must encode policies as 28-byte values to compute leaf hashes consistent with on-chain Cardano semantics. The leaf hash itself remains Blake2b-256 → 32 bytes; only the preimage contains a 28-byte field.

### Optional uniqueness validation

`omega_commitment_core::token_policy_leaf::validate_policy_id_uniqueness(&[TokenPolicy])` returns `Some(index_of_first_duplicate)` if any `policy_id` appears more than once, else `None`. Sanity helper for callers; commitment generation does NOT require uniqueness.

### Sub-trees status

| # | Sub-tree | Plan | Status |
|---|---|---|---|
| 1 | UTXO set | `2026-05-01-omega-utxo-commitment-plan.md` | Shipped (v0.1.0) |
| 2 | Block header chain | `2026-05-01-omega-block-header-accumulator-plan.md` | Shipped (v0.2.0) |
| 3 | Transaction index | `2026-05-01-omega-tx-index-plan.md` | Shipped (v0.3.0) |
| 4 | Native token policies | `2026-05-01-omega-token-policies-plan.md` | Shipped (v0.4.0) |
| 5 | Script registry | TBD | Pending |
| 6 | Stake state | TBD | Pending |
| 7 | Governance state | TBD | Pending |

### Carried forward (still open)

- **Dual-track shadow hash** — was program-level pending decision when v0.4.0 was cut; **resolved on 2026-05-01** as Option 3 (selective dual-track). Plonky3 circuit authors are now unblocked to lock to single-track Blake2b root format. See `docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md` and the "Resolved: dual-track shadow hash" section earlier in this README.

## v0.5.0 — Script registry sub-tree

Adds the fifth of seven Ω-Commitment sub-trees: the script registry. Powers `claim_script` proofs that "validator hash V was canonical on the old chain at slot S, with size N bytes and language L" — pure provenance/identity continuity. Does NOT re-execute scripts; if the new chain's script ISA differs, developers port semantics separately and link the new validator to the old hash via the `claim_script` record.

### Breaking changes from v0.4.0

- **CLI argument value:** `--sub-tree` now accepts `script` in addition to `utxo`, `header`, `tx-index`, and `token-policy`.
- No other API or schema changes. `SubTree` was already `#[non_exhaustive]` from v0.4.0, so adding a fifth variant is non-breaking for downstream pattern matchers.

### Script registry sub-tree usage

```bash
omega-commitment commit \
  --sub-tree script \
  --input path/to/script_registry.json \
  --output ./out
```

Script registry input JSON shape:

```json
{
  "scripts": [
    {
      "script_hash": "<56 hex chars / 28 bytes>",
      "deployment_slot": 100,
      "script_size_bytes": 1024,
      "language": 2
    }
  ]
}
```

### Script-entry leaf encoding

```
script_hash (28 bytes) || deployment_slot (u64 BE) || script_size_bytes (u32 BE) || language (u8)
```

Total: 41 bytes per script, fixed-width. Hashed with Blake2b-256.

**Language byte values:**
- `0` — native multi-sig (timelock script)
- `1` — Plutus V1
- `2` — Plutus V2 (Vasil)
- `3` — Plutus V3 (Plomin)
- Future variants reserved.

**Note on `script_hash` width:** Same 28-byte (Blake2b-224) Cardano-native width as `policy_id` in the token-policy sub-tree. This is the consistent convention for Cardano-side hashes appearing inside Ω-Commitment leaf preimages.

### Optional uniqueness validation

`omega_commitment_core::script_registry_leaf::validate_script_hash_uniqueness(&[ScriptEntry])` returns `Some(index_of_first_duplicate)` if any `script_hash` appears more than once, else `None`. Sanity helper for callers; commitment generation does NOT require uniqueness.

### Sub-trees status

| # | Sub-tree | Plan | Status |
|---|---|---|---|
| 1 | UTXO set | `2026-05-01-omega-utxo-commitment-plan.md` | Shipped (v0.1.0) |
| 2 | Block header chain | `2026-05-01-omega-block-header-accumulator-plan.md` | Shipped (v0.2.0) |
| 3 | Transaction index | `2026-05-01-omega-tx-index-plan.md` | Shipped (v0.3.0) |
| 4 | Native token policies | `2026-05-01-omega-token-policies-plan.md` | Shipped (v0.4.0) |
| 5 | Script registry | `2026-05-01-omega-script-registry-plan.md` | Shipped (v0.5.0) |
| 6 | Stake state | TBD | Pending |
| 7 | Governance state | TBD | Pending |

## v0.6.0 — Stake state + governance state sub-trees (track T1 leaf-tooling phase complete)

Adds the final two of seven Ω-Commitment sub-trees in a single release: **stake state** (sub-tree 6) and **governance state** (sub-tree 7). Powers `claim_stake` (port over delegation, pool, DRep history) and `claim_governance` (port over treasury, CC seats, gov-action history) per spec §9.5 and §9.6.

**With this release the per-sub-tree leaf-tooling phase of track T1 is complete.** Remaining T1 work is the bundle-assembly tool (per the dual-hash decision: emits the `(blake2b_bundle, sha3_bundle)` tuple from the seven sub-tree roots).

### Breaking changes from v0.5.0

- **CLI argument values:** `--sub-tree` now accepts `stake` and `governance` in addition to `utxo`, `header`, `tx-index`, `token-policy`, and `script`.
- No other API or schema changes. `SubTree` was already `#[non_exhaustive]`; adding two variants is non-breaking for downstream pattern matchers.

### Stake state usage

```bash
omega-commitment commit \
  --sub-tree stake \
  --input path/to/stake_state.json \
  --output ./out
```

Stake state input JSON shape:

```json
{
  "stake_entries": [
    {
      "stake_credential_hash": "<56 hex / 28 bytes>",
      "delegated_pool": "<56 hex / 28 bytes (or all-zero = undelegated)>",
      "delegated_drep": "<56 hex / 28 bytes (or all-zero = no DRep)>",
      "rewards_lovelace": 1000000,
      "is_pool_operator": 0
    }
  ]
}
```

Stake-entry leaf encoding:

```
stake_credential_hash (28) || delegated_pool (28) || delegated_drep (28) || rewards_lovelace (u64 BE) || is_pool_operator (u8)
= 93 bytes
```

### Governance state usage

```bash
omega-commitment commit \
  --sub-tree governance \
  --input path/to/governance_state.json \
  --output ./out
```

Governance state input JSON shape:

```json
{
  "facts": [
    {
      "kind": 0,
      "key": "<64 hex / 32 bytes>",
      "value": 1700000000000000,
      "slot": 100000
    }
  ]
}
```

Governance-fact leaf encoding:

```
kind (u8) || key (32 bytes) || value (u128 BE) || slot (u64 BE)
= 57 bytes
```

**Kind discriminants:**
- `0` — Treasury balance. `key` = all-zero. `value` = lovelace balance.
- `1` — CC seat. `key` = member's credential hash (28 bytes right-padded to 32). `value` = expiration epoch.
- `2` — Ratified gov action. `key` = action's tx_id. `value` = packed `(action_type:u16) | (slot_ratified:u64 << 16)`; top 48 bits reserved.
- `3` — In-flight gov action. `key` = action's tx_id. `value` = packed `(action_type:u16) | (slot_submitted:u64 << 16)`; top 48 bits reserved.
- Future variants reserved.

### Optional uniqueness validators

- `omega_commitment_core::stake_state_leaf::validate_stake_credential_uniqueness(&[StakeEntry])` — Some(idx) on first duplicate `stake_credential_hash`.
- `omega_commitment_core::governance_state_leaf::validate_governance_keys_unique_per_kind(&[GovernanceFact])` — Some(idx) on first duplicate `(kind, key)` pair. Same `key` across different `kind`s is allowed.

### Sub-trees status (final per-sub-tree state)

| # | Sub-tree | Plan | Status |
|---|---|---|---|
| 1 | UTXO set | `2026-05-01-omega-utxo-commitment-plan.md` | Shipped (v0.1.0) |
| 2 | Block header chain | `2026-05-01-omega-block-header-accumulator-plan.md` | Shipped (v0.2.0) |
| 3 | Transaction index | `2026-05-01-omega-tx-index-plan.md` | Shipped (v0.3.0) |
| 4 | Native token policies | `2026-05-01-omega-token-policies-plan.md` | Shipped (v0.4.0) |
| 5 | Script registry | `2026-05-01-omega-script-registry-plan.md` | Shipped (v0.5.0) |
| 6 | Stake state | `2026-05-01-omega-stake-and-governance-plan.md` | **Shipped (v0.6.0)** |
| 7 | Governance state | `2026-05-01-omega-stake-and-governance-plan.md` | **Shipped (v0.6.0)** |

### What ships next

The per-sub-tree leaf-tooling phase of track T1 is **complete**. Remaining T1 work:
- **Bundle-assembly tool** — aggregates the seven Blake2b sub-tree roots and seven SHA3 sub-tree roots, emits the canonical Ω-Commitment tuple `(blake2b_bundle_root, sha3_bundle_root)` per the dual-hash decision (see `docs/superpowers/decisions/2026-05-01-omega-dual-hash-decision.md`).

Adjacent unblocked tracks:
- **Track T2 (Plonky3 claim circuits)** — circuit format locked to single-track Blake2b root per the resolved dual-hash decision; circuits for all 7 claim types now have stable input formats.
- **Track T9 (CIPs)** — CIP-Ω-1 (commitment format spec) can be drafted with concrete language for all seven sub-trees.

## v0.7.0 — Bundle assembly tool (track T1 leaf+bundle phase complete)

Adds the **bundle-assembly tool** that aggregates the seven sub-tree roots into the canonical Ω-Commitment tuple `(blake2b_bundle_root, sha3_bundle_root)` per the dual-hash decision (2026-05-01).

**With this release the leaf-tooling AND bundle-tooling phases of track T1 are complete.** The Ω-Commitment is now end-to-end producible from raw per-sub-tree inputs.

### New crate

`omega-commitment-bundle` — workspace member at v0.7.0. Library + binary `omega-bundle`.

### CLI usage

**Assemble:**
```bash
omega-bundle assemble \
  --input-dir path/to/input/ \
  --output path/to/bundle.json
```

`--input-dir` must contain exactly seven files:
- `utxo.json`
- `header.json`
- `tx_index.json`
- `token_policy.json`
- `script.json`
- `stake.json`
- `governance.json`

Each file's schema matches what the corresponding `omega-commitment commit --sub-tree <name>` already accepts.

**Verify:**
```bash
omega-bundle verify \
  --bundle path/to/bundle.json \
  --input-dir path/to/input/
```

Re-runs assembly against the same inputs and confirms both bundle roots match. Exits non-zero on mismatch with a specific error (which root diverged, expected vs actual).

### Bundle output (`bundle.json`)

```json
{
  "schema_version": 1,
  "blake2b_bundle_root": "<64 hex>",
  "sha3_bundle_root": "<64 hex>",
  "sub_trees": [
    {
      "sub_tree": "utxo",
      "blake2b_root": "<64 hex>",
      "sha3_root": "<64 hex>",
      "input_digest": "<64 hex>",
      "leaf_count": 4,
      "tree_depth": 2,
      "item_count": 3
    }
    /* 6 more in canonical SubTree enum order */
  ]
}
```

### Bundle root encoding (canonical)

```
blake2b_bundle_root = Blake2b-256(
    utxo_root || header_root || tx_index_root ||
    token_policy_root || script_root || stake_root || governance_root
)

sha3_bundle_root = SHA3-256(
    utxo_sha3_root || header_sha3_root || tx_index_sha3_root ||
    token_policy_sha3_root || script_sha3_root || stake_sha3_root ||
    governance_sha3_root
)
```

Sub-tree roots are 32 bytes each; concatenated input is 7 × 32 = 224 bytes. Order is fixed by the `SubTreeId::ALL` constant in `omega-commitment-bundle::sub_tree_id`.

### Per-sub-tree SHA3 root semantics

Per the dual-hash decision: per-leaf hashing stays Blake2b-only. The SHA3 root for a sub-tree is a **SHA3 Merkle aggregation over the same Blake2b leaf hashes** (i.e., the leaves are unchanged; only the aggregation step runs in SHA3-256 instead of Blake2b-256). This keeps leaf-encoding work shared between the two hash flavors and means the SHA3 root is "the SHA3 commitment to the Blake2b leaf set."

### Bundle does NOT carry attestations

The bundle is the *commitment* that downstream attestations attest TO. Mithril-PQ signatures, recursive STARK proofs, and CIP-1694 ratification all reference the bundle root tuple but live in separate artifacts.

### What's complete in track T1

| Phase | Status |
|---|---|
| Per-sub-tree leaf encoders + Merkle trees + CLIs (sub-trees 1–7) | Shipped (v0.1.0–v0.6.0) |
| Bundle assembly + verification tooling | Shipped (v0.7.0) |

### Adjacent tracks now fully unblocked

- **Track T2 (Plonky3 claim circuits)** — all seven `claim_*` types have stable, fully-specified leaf encodings + sub-tree roots + bundle root format. Circuits can be designed against single-track Blake2b Merkle paths against per-sub-tree roots, with the dual-track concern living one layer above.
- **Track T9 (CIP-Ω-1: commitment format spec)** — every encoding, every aggregation, and the full canonical order is concrete. Drafting can complete the formal CIP.

### Sub-trees status (unchanged from v0.6.0)

| # | Sub-tree | Plan | Status |
|---|---|---|---|
| 1 | UTXO set | `2026-05-01-omega-utxo-commitment-plan.md` | Shipped (v0.1.0) |
| 2 | Block header chain | `2026-05-01-omega-block-header-accumulator-plan.md` | Shipped (v0.2.0) |
| 3 | Transaction index | `2026-05-01-omega-tx-index-plan.md` | Shipped (v0.3.0) |
| 4 | Native token policies | `2026-05-01-omega-token-policies-plan.md` | Shipped (v0.4.0) |
| 5 | Script registry | `2026-05-01-omega-script-registry-plan.md` | Shipped (v0.5.0) |
| 6 | Stake state | `2026-05-01-omega-stake-and-governance-plan.md` | Shipped (v0.6.0) |
| 7 | Governance state | `2026-05-01-omega-stake-and-governance-plan.md` | Shipped (v0.6.0) |

## v0.8.0 — Cardano ingestion + Golden Vector QA suite

Adds the **`omega-commitment-ingest`** crate (4th workspace member) with binary `omega-ingest` for transforming Cardano data into the per-sub-tree JSON formats. Also locks **canonical golden vectors** across every existing component so future encoding regressions are caught immediately.

### Honest scope

- **UTXO ingestion** — fully implemented end-to-end against a hand-crafted minimal CBOR fixture committed in-tree. Pipeline: CBOR -> `omega-ingest utxo` -> JSON -> `omega-commitment commit --sub-tree utxo` -> root.
- **Golden vectors** — pinned per-sub-tree roots for all seven synthetic fixtures, plus the canonical Ω-Commitment bundle root tuple.
- **Other 4 LedgerState-derivable sub-trees** (token-policy, script, stake, governance) — scaffolded with `unimplemented!()` and `#[ignore]`d test stubs. Real implementation requires Conway-era LedgerState parsing against `pallas` and is gated on the follow-up `omega-commitment-ingest-mainnet` plan.
- **Header + tx-index ingestion** — requires a chain-follower (multi-day operation, separate from LedgerState parsing). Out of scope for v0.8.0.

### Golden vectors

Three golden-vector test files lock canonical outputs from current code:

- `crates/omega-commitment-core/tests/golden_vectors.rs` — per-sub-tree root for each of the seven synthetic fixtures + UTXO witness round-trip.
- `crates/omega-commitment-bundle/tests/golden_bundle.rs` — pinned bundle root tuple from the v0.7.0 smoke run:
  - `blake2b_bundle_root = ee308b538b26e6d87b115ffac5676f39d0059f75dd8c79221b6b80186aebd712`
  - `sha3_bundle_root    = 189826cfa4be57615db0ac4e5fab2602291921d54365198847927e5461638b77`
- `crates/omega-commitment-ingest/tests/qa_pipeline.rs` — end-to-end CBOR-fixture pipeline test.

Any drift in these constants means encoding logic changed and must be investigated before regenerating the pin.

### omega-ingest CLI

```bash
omega-ingest utxo --input path/to/snapshot.cbor --output path/to/utxos.json
omega-ingest token-policy --input ...   # SCAFFOLD - unimplemented
omega-ingest script --input ...         # SCAFFOLD - unimplemented
omega-ingest stake --input ...          # SCAFFOLD - unimplemented
omega-ingest governance --input ...     # SCAFFOLD - unimplemented
```

The `utxo` subcommand consumes the in-tree simplified CBOR fixture format documented at `crates/omega-commitment-ingest/tests/fixtures/ledger_state_minimal.cbor.md`. Real Conway-era LedgerState parsing is the next plan in this track.

### Manual real-data QA

A human-invoked downloader is provided:

```bash
./scripts/download_snapshot.sh
```

Downloads the latest Mithril-attested preview-testnet snapshot to `var/snapshots/<digest>/` (gitignored). Multi-GB; not invoked by tests.

Once downloaded, point `omega-ingest` at it — but note that v0.8.0 only knows the simplified fixture format, so real-snapshot ingestion will fail until the follow-up plan adds Conway-era LedgerState parsing.

### What's complete in track T1

| Phase | Status |
|---|---|
| Per-sub-tree leaf encoders + Merkle trees + CLIs | Shipped (v0.1.0–v0.6.0) |
| Bundle assembly + verification | Shipped (v0.7.0) |
| Golden vector QA suite (synthetic + bundle) | Shipped (v0.8.0) |
| UTXO ingestion from hand-crafted CBOR | Shipped (v0.8.0) |
| Other 4 LedgerState-derivable sub-tree ingestion | Scaffolded — follow-up plan |
| Header + tx-index ingestion (chain-follower) | Future plan |
| Real Mithril snapshot end-to-end | Manual via download script |

### Sub-trees status (ingestion overlay v0.8.0)

| # | Sub-tree | Status |
|---|---|---|
| 1 | UTXO set | Shipped (v0.1.0) + UTXO ingestion v0.8.0 |
| 2 | Block header chain | Shipped (v0.2.0) — ingestion: future |
| 3 | Transaction index | Shipped (v0.3.0) — ingestion: future |
| 4 | Native token policies | Shipped (v0.4.0) — ingestion: scaffolded |
| 5 | Script registry | Shipped (v0.5.0) — ingestion: scaffolded |
| 6 | Stake state | Shipped (v0.6.0) — ingestion: scaffolded |
| 7 | Governance state | Shipped (v0.6.0) — ingestion: scaffolded |

## v0.9.0 — Four scaffolded ingestion paths implemented

The `unimplemented!()` markers from v0.8.0's token-policy, script, stake, and governance ingestion modules are now real implementations against richer hand-crafted CBOR fixtures. All five LedgerState-derivable sub-trees have working CBOR-fixture ingestion paths end-to-end, and ingestion-layer golden vectors are pinned for regression detection.

### What changed

- **Extended UTXO CBOR fixture** (`tests/fixtures/ledger_state_extended.cbor`) — 6-element format adds multi-asset bundles and optional script credentials per UTXO.
- **UTXO ingestion** is now backwards-compatible: it accepts both the v0.8.0 minimal 4-element format and the v0.9.0 extended 6-element format. (Note: at v0.9.0 the extended fields fed token-policy/script ingestion only and the UTXO sub-tree's `Utxo.assets` was empty — v0.9.1's P0 fix corrects this; see the v0.9.1 section below.)
- **Token-policy ingestion** walks the extended UTXO fixture's multi-asset bundles, sums quantities per `policy_id` across all UTXOs, and emits a deduplicated, sorted policy list. `first_issuance_slot` is pinned to 0 (synthetic-fixture limitation).
- **Script ingestion** walks the extended UTXO fixture's `script_credential` fields, deduplicates by `script_hash`, and emits the script registry. `deployment_slot` is pinned to 0 (same limitation).
- **Stake ingestion** parses a new dedicated CBOR fixture `tests/fixtures/stake_snapshot.cbor` (4 entries covering undelegated / pool-only / pool+DRep / pool-operator).
- **Governance ingestion** parses a new dedicated CBOR fixture `tests/fixtures/governance_snapshot.cbor` (4 facts, one per `kind`: treasury / CC seat / ratified action / in-flight action).
- **End-to-end pipeline test** runs all 5 sub-trees through ingestion → leaf hashes → root, asserts each root is non-zero and distinct.
- **Ingestion-layer golden vectors** pinned in `crates/omega-commitment-ingest/tests/golden_ingest.rs`: 5 per-sub-tree roots + the canonical "hybrid" bundle root tuple (5 from CBOR + header & tx-index from existing JSON fixtures).

### What v0.9.0 still does NOT do

Real Conway-era LedgerState parsing remains future work. v0.9.0's CBOR fixtures are hand-crafted simplified formats — sufficient to exercise every ingestion code path deterministically in CI, but not the same shape as a Mithril snapshot or a `cardano-node` LedgerState dump. The `scripts/download_snapshot.sh` script can fetch real Mithril preview-testnet data for human inspection, but `omega-ingest` cannot yet parse it.

**v1.0 ingestion architecture (resolved 2026-05-03):** a two-stream pipeline drives the five LedgerState-derivable sub-trees against a real Mithril-bootstrapped headless mainnet node — see `docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md` (REVISION 2026-05-03 section).

| Source | Sub-trees produced |
|---|---|
| `cardano-cli conway query ledger-state` (single ~2 GB JSON) | stake, governance |
| `omega-utxo-snapshot` binary (pallas-network LSQ client → CBOR) | utxo, token-policy, script |

The third-party indexer / REST fallback (Koios / Blockfrost) is no longer planned — both streams now have a working in-tree producer. The `omega-utxo-snapshot` crate ships at `crates/omega-utxo-snapshot/`; it bypasses `cardano-cli ... query utxo --whole-utxo` (documented testnet-only and broken on mainnet by the upstream Word16-VLE TxIx decoder bug).

### Sub-trees status

| # | Sub-tree | Commitment layer | Ingestion (CBOR fixture) | Real-snapshot |
|---|---|---|---|---|
| 1 | UTXO | Shipped (v0.1.0) | Shipped (v0.8.0+) | v1.0 |
| 2 | Block header chain | Shipped (v0.2.0) | Chain-follower req'd | v1.0+ |
| 3 | Transaction index | Shipped (v0.3.0) | Chain-follower req'd | v1.0+ |
| 4 | Native token policies | Shipped (v0.4.0) | **Shipped (v0.9.0)** | v1.0 |
| 5 | Script registry | Shipped (v0.5.0) | **Shipped (v0.9.0)** | v1.0 |
| 6 | Stake state | Shipped (v0.6.0) | **Shipped (v0.9.0)** | v1.0 |
| 7 | Governance state | Shipped (v0.6.0) | **Shipped (v0.9.0)** | v1.0 |

## v0.9.1 — Codex audit fixes (P0 + medium + doc-drift)

Patch release closing four findings from the Codex audit pass on
`codex-review-2026-05-02` (commit `2464926`):

- **P0 (CLI):** Four `omega-ingest` runners (`token-policy`, `script`,
  `stake`, `governance`) discarded their library output and exited
  successfully. Now they write JSON to `--output` like `utxo` does.
  Four new `cli_ingest_*_writes_output` tests assert on file contents.
- **P0 (semantics):** UTXO sub-tree ingestion no longer drops native
  asset bundles. The 6-elem extended fixture's `multi_assets` field
  is now parsed into `Utxo.assets` (was silently set to `Vec::new()`),
  matching spec §9.1's `claim_utxo` "resurrects ADA + native tokens"
  contract. Token-policy and script sub-trees still derive from the
  same multi-asset walk.
- **Medium (CBOR):** All five CBOR parsers now reject trailing bytes
  via a new `cbor::expect_end` helper. Previously, garbage suffix on
  a valid CBOR prefix was silently ignored.
- **Low (docs):** Brief and wiki log claimed 236 tests; actual count
  was 228 in v0.9.0. Corrected to 248 in v0.9.1.

### Golden vectors that drifted

UTXO sub-tree leaf encoding now includes asset bundles, so:
- `omega-commitment-ingest/tests/golden_ingest.rs::golden_utxo_root_from_extended_cbor` — re-pinned
- `omega-commitment-ingest/tests/golden_ingest.rs::golden_hybrid_bundle_roots` — both blake2b + sha3 re-pinned

Synthetic per-sub-tree golden vectors in `omega-commitment-core` and
the bundle golden vector in `omega-commitment-bundle` are UNCHANGED
(they consume JSON fixtures that already had `assets` populated).
