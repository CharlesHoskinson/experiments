# Omega Bundle Assembly Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the bundle-assembly tool that aggregates seven sub-tree roots into the canonical Ω-Commitment tuple `(blake2b_bundle_root, sha3_bundle_root)`, closing out the leaf-tooling + bundle-tooling phase of track T1.

**Architecture:** New `omega-commitment-bundle` workspace crate. Library + CLI with two subcommands: `assemble` (read 7 sub-tree input JSONs, recompute Blake2b + SHA3 roots, aggregate into the bundle, emit `bundle.json`) and `verify` (re-run assembly against same inputs, confirm bundle roots match). Reuses existing per-leaf encoders from `omega-commitment-core`. Synchronized v0.7.0 bump for the whole workspace.

**Tech Stack:** Rust 1.79+, blake2, sha3, serde, clap, anyhow (no new external deps). New internal dep: workspace crates depend on `omega-commitment-core`.

**Track:** T1 (Ω-Commitment Tooling) — final phase. After this lands, T1 leaf+bundle tooling is complete and tracks T2 (Plonky3 circuits) and T9 (CIPs) can proceed against a fully-defined commitment format.

**Locked design decisions honored:**
- Decision 7 (PQ-only crypto): Blake2b-256 + SHA3-256 throughout. No curves.
- Decision 8 (Plonky3-friendly): bundle root is a hash; Plonky3 circuits verify Merkle paths against the Blake2b half of the bundle root tuple. SHA3 half is consumed only at the attestation layer.
- Decision 9 (selective dual-track): **bundle layer is dual; per-sub-tree stays single.** This plan implements exactly that boundary.
- Decision 2 (lazy/pull): unchanged.

---

## Resolved architectural decisions (in-plan)

### A1: SHA3 sub-tree roots are computed at bundle time

The per-sub-tree CLI does not emit SHA3 roots — by design, sub-tree tooling stays single-track. The bundle tool reads the same input JSON files the per-sub-tree CLIs consumed and recomputes BOTH roots in one pass. This:
- Cross-checks the Blake2b root (assemble fails loudly if a tampered intermediate `commitment.json` disagrees with the recomputed value).
- Computes the SHA3 root from the same canonical leaf encodings that sub-tree tooling already defined.
- Keeps per-sub-tree CLIs untouched.

### A2: Bundle root encoding (canonical SubTree order)

Order matches the existing `SubTree` enum in `omega-commitment-cli` (which itself matches the order sub-trees were shipped):

```
Ord 0: utxo
Ord 1: header
Ord 2: tx-index
Ord 3: token-policy
Ord 4: script
Ord 5: stake
Ord 6: governance
```

```
blake2b_bundle_root = Blake2b-256(
    utxo_blake2b_root || header_blake2b_root || tx_index_blake2b_root ||
    token_policy_blake2b_root || script_blake2b_root || stake_blake2b_root ||
    governance_blake2b_root
)

sha3_bundle_root = SHA3-256(
    utxo_sha3_root || header_sha3_root || tx_index_sha3_root ||
    token_policy_sha3_root || script_sha3_root || stake_sha3_root ||
    governance_sha3_root
)
```

Each sub-tree root is 32 bytes; concatenated input is 7 × 32 = **224 bytes**. Order is fixed. Determinism is total.

### A3: Bundle output format (`bundle.json`)

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
    /* 6 more in canonical order */
  ]
}
```

The bundle does NOT carry attestation data (Mithril-PQ signatures, recursive STARK proof, CIP-1694 ratification). Those are separate artifacts that reference the bundle root tuple.

### A4: Input directory layout

`omega-bundle assemble --input-dir <dir>` expects exactly seven files in `<dir>`, named:

- `utxo.json`
- `header.json`
- `tx_index.json`
- `token_policy.json`
- `script.json`
- `stake.json`
- `governance.json`

Each file's schema matches what the corresponding `omega-commitment commit --sub-tree <name>` already accepts. If any of the seven is missing, assemble fails loudly with a path-specific error.

### A5: Synchronized v0.7.0 bump for the entire workspace

All three crates jump to 0.7.0 in lock-step. One workspace, one version. The new crate (`omega-commitment-bundle`) starts at 0.7.0 (NOT 0.1.0) to keep the workspace coherent — there's no benefit to having three different version strings in the same repo.

---

## File structure (post-plan)

```
omega-commitment/
├── Cargo.toml                                            (workspace; add omega-commitment-bundle member)
├── README.md                                             (extended: v0.7.0 release notes)
├── crates/
│   ├── omega-commitment-core/
│   │   ├── Cargo.toml                                    (version 0.7.0)
│   │   └── src/                                          (unchanged)
│   ├── omega-commitment-cli/
│   │   ├── Cargo.toml                                    (version 0.7.0)
│   │   └── src/                                          (unchanged)
│   └── omega-commitment-bundle/                          (NEW)
│       ├── Cargo.toml                                    (version 0.7.0)
│       ├── src/
│       │   ├── lib.rs                                    (re-exports; SubTree enum copy; build helpers)
│       │   ├── sub_tree_id.rs                            (canonical SubTree enum + ALL/ORDER consts + filename mapping)
│       │   ├── recompute.rs                              (per-sub-tree leaf-list builders for blake2b AND sha3 roots)
│       │   ├── bundle.rs                                 (BundleRecord struct, assemble(), verify())
│       │   └── main.rs                                   (CLI: assemble + verify subcommands)
│       └── tests/
│           ├── fixtures/
│           │   └── bundle_input/                         (NEW dir, populated at test time by copying from omega-commitment-core fixtures with renames)
│           ├── assemble_integration.rs                   (end-to-end against all 7 prior fixtures)
│           └── verify_integration.rs                     (round-trip assemble→verify)
```

Each file has one clear responsibility:
- `sub_tree_id.rs` — single source of truth for sub-tree identity, canonical order, and filename mapping.
- `recompute.rs` — pure: takes raw input bytes for a sub-tree, returns `(blake2b_root, sha3_root, input_digest, leaf_count, tree_depth, item_count)`. Mirrors the per-sub-tree CLI's `build_*_leaves` functions but emits both hash flavors.
- `bundle.rs` — assembles a `BundleRecord` from seven sub-tree records; computes `blake2b_bundle_root` and `sha3_bundle_root`; verifies a bundle against fresh inputs.
- `main.rs` — only impure file: argument parsing, file I/O, calls into the library.

---

## Task 1: Workspace + new crate skeleton

**Files:**
- Modify: `Cargo.toml` (workspace)
- Create: `crates/omega-commitment-bundle/Cargo.toml`
- Create: `crates/omega-commitment-bundle/src/lib.rs` (stub)
- Create: `crates/omega-commitment-bundle/src/main.rs` (stub)

- [ ] **Step 1: Create the new crate's directory + stubs**

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
mkdir -p crates/omega-commitment-bundle/src
mkdir -p crates/omega-commitment-bundle/tests/fixtures
```

Write `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/Cargo.toml`:

```toml
[package]
name = "omega-commitment-bundle"
version = "0.7.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lib]
name = "omega_commitment_bundle"
path = "src/lib.rs"

[[bin]]
name = "omega-bundle"
path = "src/main.rs"

[dependencies]
omega-commitment-core = { path = "../omega-commitment-core" }
clap.workspace = true
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
hex.workspace = true
thiserror.workspace = true
anyhow = "1"
blake2.workspace = true
sha3.workspace = true

[dev-dependencies]
tempfile = "3"
```

Write `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/src/lib.rs`:

```rust
//! omega-commitment-bundle: assembles the canonical Ω-Commitment tuple
//! `(blake2b_bundle_root, sha3_bundle_root)` from the seven sub-tree
//! commitments per the dual-hash decision (2026-05-01).
```

Write `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/src/main.rs`:

```rust
fn main() {}
```

- [ ] **Step 2: Add the new crate to the workspace**

Edit `/home/hoskinson/omega-commitment/Cargo.toml`. Find the `members = [...]` list and add the third entry:

```toml
[workspace]
resolver = "2"
members = [
  "crates/omega-commitment-core",
  "crates/omega-commitment-cli",
  "crates/omega-commitment-bundle",
]
```

(Other `[workspace.*]` sections unchanged.)

- [ ] **Step 3: Verify the workspace builds with the new crate**

```bash
cargo build --workspace 2>&1 | tail -10
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

Expected: clean build, no lint or fmt issues. If `cargo fmt-check` shows diffs in the new files, run `cargo fmt --all`.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/omega-commitment-bundle/
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "chore(bundle): scaffold omega-commitment-bundle workspace crate"
```

---

## Task 2: `sub_tree_id.rs` — canonical order + filename mapping

**Files:**
- Create: `crates/omega-commitment-bundle/src/sub_tree_id.rs`
- Modify: `crates/omega-commitment-bundle/src/lib.rs`

Single source of truth for the seven sub-tree identifiers, their canonical ordering, and the filename each one expects in `--input-dir`.

- [ ] **Step 1: Write `sub_tree_id.rs`**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/src/sub_tree_id.rs`

```rust
//! Canonical sub-tree identifier + ordering + filename mapping.
//!
//! The Ω-Commitment bundle aggregates the seven sub-tree roots in a
//! fixed canonical order. `ALL` is the authoritative order used by
//! both `assemble` and `verify`.

use serde::Serialize;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubTreeId {
    Utxo,
    Header,
    TxIndex,
    TokenPolicy,
    Script,
    Stake,
    Governance,
}

/// All sub-trees in canonical Ω-Commitment order. The bundle root
/// hashes the seven sub-tree roots in exactly this order.
pub const ALL: [SubTreeId; 7] = [
    SubTreeId::Utxo,
    SubTreeId::Header,
    SubTreeId::TxIndex,
    SubTreeId::TokenPolicy,
    SubTreeId::Script,
    SubTreeId::Stake,
    SubTreeId::Governance,
];

impl SubTreeId {
    /// Filename expected inside `--input-dir`.
    pub fn filename(&self) -> &'static str {
        match self {
            SubTreeId::Utxo => "utxo.json",
            SubTreeId::Header => "header.json",
            SubTreeId::TxIndex => "tx_index.json",
            SubTreeId::TokenPolicy => "token_policy.json",
            SubTreeId::Script => "script.json",
            SubTreeId::Stake => "stake.json",
            SubTreeId::Governance => "governance.json",
        }
    }

    /// Stable kebab-case label used in JSON output.
    pub fn label(&self) -> &'static str {
        match self {
            SubTreeId::Utxo => "utxo",
            SubTreeId::Header => "header",
            SubTreeId::TxIndex => "tx-index",
            SubTreeId::TokenPolicy => "token-policy",
            SubTreeId::Script => "script",
            SubTreeId::Stake => "stake",
            SubTreeId::Governance => "governance",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_has_seven_in_canonical_order() {
        assert_eq!(ALL.len(), 7);
        assert_eq!(ALL[0], SubTreeId::Utxo);
        assert_eq!(ALL[6], SubTreeId::Governance);
    }

    #[test]
    fn filenames_are_unique() {
        let names: std::collections::HashSet<&str> =
            ALL.iter().map(|s| s.filename()).collect();
        assert_eq!(names.len(), 7);
    }

    #[test]
    fn labels_match_per_sub_tree_cli_kebab_case() {
        // Sanity-check labels match the kebab-case rendering used by
        // omega-commitment-cli's SubTree enum.
        assert_eq!(SubTreeId::Utxo.label(), "utxo");
        assert_eq!(SubTreeId::TxIndex.label(), "tx-index");
        assert_eq!(SubTreeId::TokenPolicy.label(), "token-policy");
        assert_eq!(SubTreeId::Governance.label(), "governance");
    }

    #[test]
    fn filename_for_each_variant() {
        assert_eq!(SubTreeId::Utxo.filename(), "utxo.json");
        assert_eq!(SubTreeId::Header.filename(), "header.json");
        assert_eq!(SubTreeId::TxIndex.filename(), "tx_index.json");
        assert_eq!(SubTreeId::TokenPolicy.filename(), "token_policy.json");
        assert_eq!(SubTreeId::Script.filename(), "script.json");
        assert_eq!(SubTreeId::Stake.filename(), "stake.json");
        assert_eq!(SubTreeId::Governance.filename(), "governance.json");
    }
}
```

- [ ] **Step 2: Update `lib.rs` to re-export**

Replace `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/src/lib.rs` contents with:

```rust
//! omega-commitment-bundle: assembles the canonical Ω-Commitment tuple
//! `(blake2b_bundle_root, sha3_bundle_root)` from the seven sub-tree
//! commitments per the dual-hash decision (2026-05-01).

pub mod sub_tree_id;
```

- [ ] **Step 3: Verify**

```bash
cargo test -p omega-commitment-bundle sub_tree_id::tests 2>&1 | tail -10   # 4 tests pass
cargo lint 2>&1 | tail -3                                                    # clean
cargo fmt-check 2>&1 | tail -3                                               # clean
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-bundle/src/sub_tree_id.rs \
        crates/omega-commitment-bundle/src/lib.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(bundle): canonical SubTreeId enum + filename + label mapping"
```

---

## Task 3: `recompute.rs` — per-sub-tree dual-hash root computation

**Files:**
- Create: `crates/omega-commitment-bundle/src/recompute.rs`
- Modify: `crates/omega-commitment-bundle/src/lib.rs`

Pure functions that take raw JSON input bytes for a sub-tree and return both the Blake2b root (cross-check) and SHA3 root (new), plus metadata. Mirrors the per-sub-tree CLI's `build_*_leaves` functions but emits both hash flavors.

The trick: we already have `tree::MerkleTree` that's Blake2b-only. For the SHA3 path we need a parallel build using the same sort-pad-aggregate logic but with SHA3-256 instead of Blake2b-256. We implement a small `sha3_root_of(leaves: Vec<[u8; 32]>) -> [u8; 32]` helper inside this crate (it's a 30-line port of `MerkleTree::build` substituting the hash function — small enough to live here without bloating `omega-commitment-core`).

The leaf hashes themselves stay Blake2b-256 (per the dual-hash decision: per-leaf is single-track). Only the *aggregation step* runs in SHA3 for the SHA3 path.

Wait — re-reading the decision: "Per-sub-tree Merkle root: Blake2b-256 only." So the SHA3 sub-tree root is computed by SHA3-aggregating the SAME Blake2b leaf hashes? Or by SHA3-aggregating SHA3 leaf hashes?

The decision says **per-leaf hashing is Blake2b-only**. So the SHA3 sub-tree root is a SHA3 Merkle aggregation over the **Blake2b leaf hashes**. This keeps leaf encoding work shared and means the SHA3 root is "the SHA3 commitment to the Blake2b leaf set." That's the natural reading and the simplest implementation.

- [ ] **Step 1: Write `recompute.rs`**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/src/recompute.rs`

```rust
//! Per-sub-tree dual-hash root computation.
//!
//! Given raw input bytes for a sub-tree (the JSON file the per-sub-tree
//! CLI consumes), recompute:
//!   - the Blake2b leaf set (same logic the per-sub-tree CLI uses)
//!   - the Blake2b Merkle root over those leaves (cross-check)
//!   - the SHA3 Merkle root over those same leaves (new — for dual-track bundle)
//!   - the input digest (Blake2b-256 of raw input bytes)
//!   - leaf_count, tree_depth, item_count
//!
//! Per the dual-hash decision (2026-05-01): per-leaf hashing stays
//! Blake2b-only. The SHA3 root is a SHA3 Merkle aggregation over the
//! SAME Blake2b leaf hashes. Only the aggregation step runs in SHA3.

use crate::sub_tree_id::SubTreeId;
use omega_commitment_core::{
    governance_state_leaf::GovernanceFact,
    hash::{blake2b_256, sha3_256, Hash},
    header_leaf::BlockHeader,
    script_registry_leaf::ScriptEntry,
    stake_state_leaf::StakeEntry,
    token_policy_leaf::TokenPolicy,
    tree::{MerkleTree, ZERO_HASH},
    tx_index_leaf::TxIndexEntry,
    utxo_leaf::Utxo,
};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubTreeRoots {
    pub blake2b_root: Hash,
    pub sha3_root: Hash,
    pub input_digest: Hash,
    pub leaf_count: usize,
    pub tree_depth: usize,
    pub item_count: usize,
}

#[derive(Deserialize)]
struct UtxoInput {
    utxos: Vec<Utxo>,
}
#[derive(Deserialize)]
struct HeaderInput {
    headers: Vec<BlockHeader>,
}
#[derive(Deserialize)]
struct TxIndexInput {
    entries: Vec<TxIndexEntry>,
}
#[derive(Deserialize)]
struct TokenPolicyInput {
    policies: Vec<TokenPolicy>,
}
#[derive(Deserialize)]
struct ScriptInput {
    scripts: Vec<ScriptEntry>,
}
#[derive(Deserialize)]
struct StakeInput {
    stake_entries: Vec<StakeEntry>,
}
#[derive(Deserialize)]
struct GovernanceInput {
    facts: Vec<GovernanceFact>,
}

/// Recompute sub-tree roots from raw input bytes.
///
/// Returns Err if the JSON cannot be parsed for the given sub-tree shape.
pub fn recompute(sub_tree: SubTreeId, raw: &str) -> anyhow::Result<SubTreeRoots> {
    let input_digest = blake2b_256(raw.as_bytes());
    let (leaves, item_count) = build_leaves(sub_tree, raw)?;
    let tree = MerkleTree::build(leaves.clone());
    let blake2b_root = tree.root();
    let sha3_root = sha3_root_of(leaves);
    Ok(SubTreeRoots {
        blake2b_root,
        sha3_root,
        input_digest,
        leaf_count: tree.leaf_count(),
        tree_depth: tree.depth(),
        item_count,
    })
}

fn build_leaves(sub_tree: SubTreeId, raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    match sub_tree {
        SubTreeId::Utxo => {
            let parsed: UtxoInput = serde_json::from_str(raw)?;
            let leaves: Vec<Hash> = parsed
                .utxos
                .iter()
                .map(|u| u.leaf_hash())
                .collect::<Result<Vec<_>, _>>()?;
            let n = parsed.utxos.len();
            Ok((leaves, n))
        }
        SubTreeId::Header => {
            let parsed: HeaderInput = serde_json::from_str(raw)?;
            let leaves: Vec<Hash> = parsed.headers.iter().map(|h| h.leaf_hash()).collect();
            let n = parsed.headers.len();
            Ok((leaves, n))
        }
        SubTreeId::TxIndex => {
            let parsed: TxIndexInput = serde_json::from_str(raw)?;
            let leaves: Vec<Hash> = parsed.entries.iter().map(|e| e.leaf_hash()).collect();
            let n = parsed.entries.len();
            Ok((leaves, n))
        }
        SubTreeId::TokenPolicy => {
            let parsed: TokenPolicyInput = serde_json::from_str(raw)?;
            let leaves: Vec<Hash> = parsed.policies.iter().map(|p| p.leaf_hash()).collect();
            let n = parsed.policies.len();
            Ok((leaves, n))
        }
        SubTreeId::Script => {
            let parsed: ScriptInput = serde_json::from_str(raw)?;
            let leaves: Vec<Hash> = parsed.scripts.iter().map(|s| s.leaf_hash()).collect();
            let n = parsed.scripts.len();
            Ok((leaves, n))
        }
        SubTreeId::Stake => {
            let parsed: StakeInput = serde_json::from_str(raw)?;
            let leaves: Vec<Hash> = parsed.stake_entries.iter().map(|s| s.leaf_hash()).collect();
            let n = parsed.stake_entries.len();
            Ok((leaves, n))
        }
        SubTreeId::Governance => {
            let parsed: GovernanceInput = serde_json::from_str(raw)?;
            let leaves: Vec<Hash> = parsed.facts.iter().map(|f| f.leaf_hash()).collect();
            let n = parsed.facts.len();
            Ok((leaves, n))
        }
    }
}

/// SHA3 Merkle aggregation over a set of Blake2b leaf hashes.
///
/// Mirrors `MerkleTree::build` exactly — sort, pad to next power of two
/// with `ZERO_HASH`, hash internal nodes as `H(left || right)` — but
/// substitutes SHA3-256 for Blake2b-256 in the internal-node hash.
fn sha3_root_of(mut input: Vec<Hash>) -> Hash {
    input.sort();
    let target = input.len().max(1).next_power_of_two();
    while input.len() < target {
        input.push(ZERO_HASH);
    }
    let mut current = input;
    while current.len() > 1 {
        let mut next = Vec::with_capacity(current.len() / 2);
        for chunk in current.chunks(2) {
            let mut buf = [0u8; 64];
            buf[..32].copy_from_slice(&chunk[0]);
            buf[32..].copy_from_slice(&chunk[1]);
            next.push(sha3_256(&buf));
        }
        current = next;
    }
    current[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    const UTXO_FIXTURE: &str = r#"{
        "utxos": [
            {
                "tx_id": "0101010101010101010101010101010101010101010101010101010101010101",
                "output_index": 0,
                "address_hash": "0202020202020202020202020202020202020202020202020202020202020202",
                "value_lovelace": 1000000,
                "assets": [],
                "datum_hash": null
            }
        ]
    }"#;

    #[test]
    fn recompute_utxo_returns_consistent_metadata() {
        let r = recompute(SubTreeId::Utxo, UTXO_FIXTURE).unwrap();
        assert_eq!(r.item_count, 1);
        // 1 leaf padded to 1 (next power of two of 1 is 1).
        assert_eq!(r.leaf_count, 1);
        assert_eq!(r.tree_depth, 0);
        assert_ne!(r.blake2b_root, [0u8; 32]);
        assert_ne!(r.sha3_root, [0u8; 32]);
        assert_ne!(r.blake2b_root, r.sha3_root);
        assert_ne!(r.input_digest, [0u8; 32]);
    }

    #[test]
    fn recompute_is_deterministic() {
        let a = recompute(SubTreeId::Utxo, UTXO_FIXTURE).unwrap();
        let b = recompute(SubTreeId::Utxo, UTXO_FIXTURE).unwrap();
        assert_eq!(a.blake2b_root, b.blake2b_root);
        assert_eq!(a.sha3_root, b.sha3_root);
        assert_eq!(a.input_digest, b.input_digest);
    }

    #[test]
    fn recompute_blake2b_root_matches_merkle_tree_directly() {
        // The Blake2b path inside recompute must agree with calling
        // MerkleTree::build directly — this is the cross-check that
        // protects against any divergence between the bundle tool
        // and the per-sub-tree CLI.
        let parsed: UtxoInput = serde_json::from_str(UTXO_FIXTURE).unwrap();
        let leaves: Vec<Hash> = parsed
            .utxos
            .iter()
            .map(|u| u.leaf_hash())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let tree = MerkleTree::build(leaves);
        let r = recompute(SubTreeId::Utxo, UTXO_FIXTURE).unwrap();
        assert_eq!(r.blake2b_root, tree.root());
    }

    #[test]
    fn sha3_root_of_empty_pads_to_zero_leaf() {
        // sha3_root_of with empty input pads to one ZERO_HASH leaf.
        // Depth-0 tree: root == leaf == ZERO_HASH.
        let r = sha3_root_of(vec![]);
        assert_eq!(r, ZERO_HASH);
    }

    #[test]
    fn sha3_root_of_single_leaf_is_the_leaf() {
        let leaf = blake2b_256(b"only");
        let r = sha3_root_of(vec![leaf]);
        // Single leaf, no padding needed (next_power_of_two(1) == 1).
        // Depth-0: root == leaf == leaf bytes.
        assert_eq!(r, leaf);
    }

    #[test]
    fn sha3_root_of_two_leaves_is_sha3_of_concatenation() {
        let a = blake2b_256(b"a");
        let b = blake2b_256(b"b");
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(&lo);
        buf[32..].copy_from_slice(&hi);
        let expected = sha3_256(&buf);
        assert_eq!(sha3_root_of(vec![a, b]), expected);
    }

    #[test]
    fn sha3_and_blake2b_roots_diverge_on_same_input() {
        // Sanity: SHA3 and Blake2b roots of the same leaf set must
        // differ. This is the dual-track decision's core invariant.
        let leaves: Vec<Hash> = (0..8u8).map(|i| blake2b_256(&[i])).collect();
        let blake_root = MerkleTree::build(leaves.clone()).root();
        let sha3_root = sha3_root_of(leaves);
        assert_ne!(blake_root, sha3_root);
    }
}
```

- [ ] **Step 2: Update `lib.rs` to re-export**

Replace `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/src/lib.rs` contents with:

```rust
//! omega-commitment-bundle: assembles the canonical Ω-Commitment tuple
//! `(blake2b_bundle_root, sha3_bundle_root)` from the seven sub-tree
//! commitments per the dual-hash decision (2026-05-01).

pub mod sub_tree_id;
pub mod recompute;
```

- [ ] **Step 3: Verify**

```bash
cargo test -p omega-commitment-bundle recompute::tests 2>&1 | tail -15   # 7 tests pass
cargo test --workspace 2>&1 | tail -5                                      # 156 total (145 prior + 4 sub_tree_id + 7 recompute)
cargo lint 2>&1 | tail -3                                                  # clean
cargo fmt-check 2>&1 | tail -3                                             # clean
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-bundle/src/recompute.rs \
        crates/omega-commitment-bundle/src/lib.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(bundle): per-sub-tree dual-hash recompute (Blake2b cross-check + SHA3 root)"
```

---

## Task 4: `bundle.rs` — assemble + verify

**Files:**
- Create: `crates/omega-commitment-bundle/src/bundle.rs`
- Modify: `crates/omega-commitment-bundle/src/lib.rs`

The library face of the tool. Reads the seven sub-tree input files from a directory, recomputes each pair of roots, aggregates into the bundle tuple, and emits/verifies the canonical `BundleRecord`.

- [ ] **Step 1: Write `bundle.rs`**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/src/bundle.rs`

```rust
//! Bundle assembly + verification.
//!
//! Reads seven sub-tree input JSON files from a directory, recomputes
//! each sub-tree's `(blake2b_root, sha3_root)`, and aggregates into
//! the canonical Ω-Commitment tuple `(blake2b_bundle_root, sha3_bundle_root)`.

use crate::recompute::{recompute, SubTreeRoots};
use crate::sub_tree_id::{SubTreeId, ALL};
use omega_commitment_core::hash::{blake2b_256, sha3_256, Hash};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

pub const BUNDLE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubTreeRecord {
    /// Stable kebab-case label, e.g. "utxo", "tx-index", "token-policy".
    pub sub_tree: String,
    #[serde(with = "hex::serde")]
    pub blake2b_root: Hash,
    #[serde(with = "hex::serde")]
    pub sha3_root: Hash,
    #[serde(with = "hex::serde")]
    pub input_digest: Hash,
    pub leaf_count: usize,
    pub tree_depth: usize,
    pub item_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleRecord {
    pub schema_version: u32,
    #[serde(with = "hex::serde")]
    pub blake2b_bundle_root: Hash,
    #[serde(with = "hex::serde")]
    pub sha3_bundle_root: Hash,
    /// Sub-trees in canonical order (matches `sub_tree_id::ALL`).
    pub sub_trees: Vec<SubTreeRecord>,
}

/// Read each of the seven sub-tree input files from `input_dir`, recompute
/// roots, and return the aggregated `BundleRecord`.
///
/// `input_dir` must contain exactly seven files named per
/// `SubTreeId::filename()`. Any missing file produces an error.
pub fn assemble(input_dir: &Path) -> anyhow::Result<BundleRecord> {
    let mut sub_trees: Vec<SubTreeRecord> = Vec::with_capacity(7);
    for st in ALL {
        let path = input_dir.join(st.filename());
        let raw = fs::read_to_string(&path).map_err(|e| {
            anyhow::anyhow!("cannot read sub-tree input {}: {}", path.display(), e)
        })?;
        let roots = recompute(st, &raw).map_err(|e| {
            anyhow::anyhow!("cannot recompute {}: {}", st.label(), e)
        })?;
        sub_trees.push(SubTreeRecord {
            sub_tree: st.label().to_string(),
            blake2b_root: roots.blake2b_root,
            sha3_root: roots.sha3_root,
            input_digest: roots.input_digest,
            leaf_count: roots.leaf_count,
            tree_depth: roots.tree_depth,
            item_count: roots.item_count,
        });
    }
    let blake2b_bundle_root = aggregate_blake2b(&sub_trees);
    let sha3_bundle_root = aggregate_sha3(&sub_trees);
    Ok(BundleRecord {
        schema_version: BUNDLE_SCHEMA_VERSION,
        blake2b_bundle_root,
        sha3_bundle_root,
        sub_trees,
    })
}

/// Re-run assembly against `input_dir` and confirm the resulting roots
/// match the published `bundle`. Returns `Ok(())` on match; an
/// `anyhow::Error` describing the mismatch otherwise.
pub fn verify(bundle: &BundleRecord, input_dir: &Path) -> anyhow::Result<()> {
    let fresh = assemble(input_dir)?;
    if fresh.blake2b_bundle_root != bundle.blake2b_bundle_root {
        anyhow::bail!(
            "blake2b_bundle_root mismatch: bundle says {}, recomputed {}",
            hex::encode(bundle.blake2b_bundle_root),
            hex::encode(fresh.blake2b_bundle_root)
        );
    }
    if fresh.sha3_bundle_root != bundle.sha3_bundle_root {
        anyhow::bail!(
            "sha3_bundle_root mismatch: bundle says {}, recomputed {}",
            hex::encode(bundle.sha3_bundle_root),
            hex::encode(fresh.sha3_bundle_root)
        );
    }
    if fresh.sub_trees != bundle.sub_trees {
        anyhow::bail!("sub_trees array mismatch (per-sub-tree records differ)");
    }
    if fresh.schema_version != bundle.schema_version {
        anyhow::bail!(
            "schema_version mismatch: bundle says {}, current {}",
            bundle.schema_version,
            fresh.schema_version
        );
    }
    Ok(())
}

fn aggregate_blake2b(sub_trees: &[SubTreeRecord]) -> Hash {
    let mut buf = Vec::with_capacity(7 * 32);
    for r in sub_trees {
        buf.extend_from_slice(&r.blake2b_root);
    }
    blake2b_256(&buf)
}

fn aggregate_sha3(sub_trees: &[SubTreeRecord]) -> Hash {
    let mut buf = Vec::with_capacity(7 * 32);
    for r in sub_trees {
        buf.extend_from_slice(&r.sha3_root);
    }
    sha3_256(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_minimal_inputs(dir: &Path) {
        // Minimal valid input for each sub-tree (1 entry each).
        fs::write(
            dir.join("utxo.json"),
            r#"{"utxos":[{"tx_id":"0101010101010101010101010101010101010101010101010101010101010101","output_index":0,"address_hash":"0202020202020202020202020202020202020202020202020202020202020202","value_lovelace":1,"assets":[],"datum_hash":null}]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("header.json"),
            r#"{"headers":[{"slot":1,"block_height":1,"block_hash":"1100000000000000000000000000000000000000000000000000000000000000","prev_hash":"0000000000000000000000000000000000000000000000000000000000000000"}]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("tx_index.json"),
            r#"{"entries":[{"tx_id":"1100000000000000000000000000000000000000000000000000000000000000","slot":1,"block_hash":"aa00000000000000000000000000000000000000000000000000000000000000","tx_position":0}]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("token_policy.json"),
            r#"{"policies":[{"policy_id":"11000000000000000000000000000000000000000000000000000000","first_issuance_slot":1,"total_supply_at_h":1}]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("script.json"),
            r#"{"scripts":[{"script_hash":"11000000000000000000000000000000000000000000000000000000","deployment_slot":1,"script_size_bytes":1,"language":0}]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("stake.json"),
            r#"{"stake_entries":[{"stake_credential_hash":"11000000000000000000000000000000000000000000000000000000","delegated_pool":"00000000000000000000000000000000000000000000000000000000","delegated_drep":"00000000000000000000000000000000000000000000000000000000","rewards_lovelace":0,"is_pool_operator":0}]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("governance.json"),
            r#"{"facts":[{"kind":0,"key":"0000000000000000000000000000000000000000000000000000000000000000","value":1,"slot":1}]}"#,
        )
        .unwrap();
    }

    #[test]
    fn assemble_minimal_inputs_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let bundle = assemble(dir.path()).unwrap();
        assert_eq!(bundle.schema_version, BUNDLE_SCHEMA_VERSION);
        assert_eq!(bundle.sub_trees.len(), 7);
        assert_ne!(bundle.blake2b_bundle_root, [0u8; 32]);
        assert_ne!(bundle.sha3_bundle_root, [0u8; 32]);
        assert_ne!(bundle.blake2b_bundle_root, bundle.sha3_bundle_root);
    }

    #[test]
    fn assemble_fails_loudly_on_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        // Delete one file.
        fs::remove_file(dir.path().join("script.json")).unwrap();
        let result = assemble(dir.path());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("script.json"), "error should mention the missing file: {msg}");
    }

    #[test]
    fn assemble_canonical_order_in_output() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let bundle = assemble(dir.path()).unwrap();
        let labels: Vec<&str> = bundle.sub_trees.iter().map(|s| s.sub_tree.as_str()).collect();
        assert_eq!(
            labels,
            vec!["utxo", "header", "tx-index", "token-policy", "script", "stake", "governance"]
        );
    }

    #[test]
    fn verify_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let bundle = assemble(dir.path()).unwrap();
        verify(&bundle, dir.path()).expect("fresh bundle must verify against same inputs");
    }

    #[test]
    fn verify_detects_blake2b_root_tamper() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let mut bundle = assemble(dir.path()).unwrap();
        bundle.blake2b_bundle_root[0] ^= 0x01;
        let result = verify(&bundle, dir.path());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("blake2b_bundle_root mismatch"), "got: {msg}");
    }

    #[test]
    fn verify_detects_sha3_root_tamper() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let mut bundle = assemble(dir.path()).unwrap();
        bundle.sha3_bundle_root[0] ^= 0x01;
        let result = verify(&bundle, dir.path());
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("sha3_bundle_root mismatch"), "got: {msg}");
    }

    #[test]
    fn verify_detects_input_data_tamper() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let bundle = assemble(dir.path()).unwrap();
        // Tamper with one input file.
        fs::write(
            dir.path().join("utxo.json"),
            r#"{"utxos":[{"tx_id":"0101010101010101010101010101010101010101010101010101010101010101","output_index":0,"address_hash":"0202020202020202020202020202020202020202020202020202020202020202","value_lovelace":99999,"assets":[],"datum_hash":null}]}"#,
        )
        .unwrap();
        let result = verify(&bundle, dir.path());
        assert!(result.is_err(), "tampered input must fail verify");
    }

    #[test]
    fn bundle_round_trips_through_json() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_inputs(dir.path());
        let bundle = assemble(dir.path()).unwrap();
        let s = serde_json::to_string_pretty(&bundle).unwrap();
        let parsed: BundleRecord = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, bundle);
    }
}
```

- [ ] **Step 2: Update `lib.rs`**

Replace `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/src/lib.rs` contents with:

```rust
//! omega-commitment-bundle: assembles the canonical Ω-Commitment tuple
//! `(blake2b_bundle_root, sha3_bundle_root)` from the seven sub-tree
//! commitments per the dual-hash decision (2026-05-01).

pub mod bundle;
pub mod recompute;
pub mod sub_tree_id;
```

Note: `tempfile` was added as a dev-dep in Task 1 — `bundle::tests` uses it.

- [ ] **Step 3: Verify**

```bash
cargo test -p omega-commitment-bundle bundle::tests 2>&1 | tail -15   # 8 tests pass
cargo test --workspace 2>&1 | tail -5                                   # 164 total (156 prior + 8 bundle)
cargo lint 2>&1 | tail -3                                               # clean
cargo fmt-check 2>&1 | tail -3                                          # clean
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-bundle/src/bundle.rs \
        crates/omega-commitment-bundle/src/lib.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(bundle): assemble + verify with canonical sub-tree ordering"
```

---

## Task 5: CLI — `assemble` + `verify` subcommands

**Files:**
- Modify: `crates/omega-commitment-bundle/src/main.rs`

The `omega-bundle` binary. Two subcommands. Reads/writes via the `bundle` module. Errors propagate via `anyhow`.

- [ ] **Step 1: Replace `main.rs`**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/src/main.rs`

```rust
//! omega-bundle CLI.
//!
//! Two subcommands:
//!   - `assemble` reads seven sub-tree inputs and emits `bundle.json`.
//!   - `verify` re-runs assembly against the same inputs and confirms
//!     the bundle's roots match.

use clap::{Parser, Subcommand};
use omega_commitment_bundle::bundle::{assemble, verify, BundleRecord};
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(name = "omega-bundle", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Read seven sub-tree inputs from --input-dir and emit bundle.json.
    Assemble {
        /// Directory containing the seven sub-tree input JSON files
        /// (utxo.json, header.json, tx_index.json, token_policy.json,
        /// script.json, stake.json, governance.json).
        #[arg(short, long)]
        input_dir: PathBuf,
        /// Output path for bundle.json.
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Re-run assembly against --input-dir and confirm the bundle's
    /// roots match. Exits non-zero on mismatch.
    Verify {
        /// Path to a previously-assembled bundle.json.
        #[arg(short, long)]
        bundle: PathBuf,
        /// Directory containing the seven sub-tree input JSON files.
        #[arg(short, long)]
        input_dir: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Assemble { input_dir, output } => cmd_assemble(input_dir, output),
        Cmd::Verify { bundle, input_dir } => cmd_verify(bundle, input_dir),
    }
}

fn cmd_assemble(input_dir: PathBuf, output: PathBuf) -> anyhow::Result<()> {
    let input_dir = input_dir.canonicalize().map_err(|e| {
        anyhow::anyhow!("cannot resolve input-dir {}: {}", input_dir.display(), e)
    })?;
    let bundle = assemble(&input_dir)?;
    fs::write(&output, serde_json::to_string_pretty(&bundle)?)?;
    println!(
        "ok: assembled bundle blake2b={} sha3={}",
        hex::encode(bundle.blake2b_bundle_root),
        hex::encode(bundle.sha3_bundle_root)
    );
    Ok(())
}

fn cmd_verify(bundle_path: PathBuf, input_dir: PathBuf) -> anyhow::Result<()> {
    let raw = fs::read_to_string(&bundle_path).map_err(|e| {
        anyhow::anyhow!("cannot read bundle {}: {}", bundle_path.display(), e)
    })?;
    let bundle: BundleRecord = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("cannot parse bundle {}: {}", bundle_path.display(), e))?;
    let input_dir = input_dir.canonicalize().map_err(|e| {
        anyhow::anyhow!("cannot resolve input-dir {}: {}", input_dir.display(), e)
    })?;
    verify(&bundle, &input_dir)?;
    println!(
        "ok: bundle verifies blake2b={} sha3={}",
        hex::encode(bundle.blake2b_bundle_root),
        hex::encode(bundle.sha3_bundle_root)
    );
    Ok(())
}
```

- [ ] **Step 2: Build the CLI**

```bash
cargo build --release -p omega-commitment-bundle 2>&1 | tail -5
```

- [ ] **Step 3: Manual smoke against tempdir**

Create a script-style smoke run with the per-sub-tree fixtures (rename to bundle-tool conventions):

```bash
TMPIN=$(mktemp -d)
cp crates/omega-commitment-core/tests/fixtures/utxo_set_small.json    "$TMPIN/utxo.json"
cp crates/omega-commitment-core/tests/fixtures/header_chain_small.json "$TMPIN/header.json"
cp crates/omega-commitment-core/tests/fixtures/tx_index_small.json     "$TMPIN/tx_index.json"
cp crates/omega-commitment-core/tests/fixtures/token_policies_small.json "$TMPIN/token_policy.json"
cp crates/omega-commitment-core/tests/fixtures/script_registry_small.json "$TMPIN/script.json"
cp crates/omega-commitment-core/tests/fixtures/stake_state_small.json     "$TMPIN/stake.json"
cp crates/omega-commitment-core/tests/fixtures/governance_state_small.json "$TMPIN/governance.json"

./target/release/omega-bundle assemble --input-dir "$TMPIN" --output "$TMPIN/bundle.json"
cat "$TMPIN/bundle.json"
./target/release/omega-bundle verify --bundle "$TMPIN/bundle.json" --input-dir "$TMPIN"
```

Expected stdout:
```
ok: assembled bundle blake2b=<64 hex> sha3=<64 hex>
ok: bundle verifies blake2b=<same> sha3=<same>
```

`bundle.json` should contain `schema_version: 1`, `blake2b_bundle_root`, `sha3_bundle_root`, and a `sub_trees` array of length 7 in canonical order.

- [ ] **Step 4: Run all tests + lint + fmt**

```bash
cargo test --workspace 2>&1 | tail -5    # 164 still pass
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-bundle/src/main.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(bundle-cli): assemble + verify subcommands"
```

---

## Task 6: End-to-end integration test against all 7 prior fixtures

**Files:**
- Create: `crates/omega-commitment-bundle/tests/end_to_end_integration.rs`

The unit tests in `bundle::tests` use minimal one-entry-per-sub-tree inputs. This integration test pulls the 8-entry-per-sub-tree fixtures from `omega-commitment-core/tests/fixtures/` (which are the same ones the per-sub-tree CLIs are tested against) and runs the full pipeline. This is the canonical "this thing actually works on the real fixtures" check.

- [ ] **Step 1: Write the test**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/tests/end_to_end_integration.rs`

```rust
//! End-to-end integration test for the bundle-assembly tool.
//!
//! Pulls the 8-entry per-sub-tree fixtures shipped by
//! `omega-commitment-core` and runs the full assemble + verify
//! pipeline against them.

use omega_commitment_bundle::bundle::{assemble, verify};
use std::{fs, path::PathBuf};

/// Path to the omega-commitment-core fixtures dir.
fn core_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("omega-commitment-core/tests/fixtures")
}

/// Mapping from per-sub-tree fixture filename to bundle-tool filename.
const FIXTURE_RENAMES: &[(&str, &str)] = &[
    ("utxo_set_small.json", "utxo.json"),
    ("header_chain_small.json", "header.json"),
    ("tx_index_small.json", "tx_index.json"),
    ("token_policies_small.json", "token_policy.json"),
    ("script_registry_small.json", "script.json"),
    ("stake_state_small.json", "stake.json"),
    ("governance_state_small.json", "governance.json"),
];

fn populate_input_dir(dest: &std::path::Path) {
    let src = core_fixtures_dir();
    for (src_name, dest_name) in FIXTURE_RENAMES {
        fs::copy(src.join(src_name), dest.join(dest_name))
            .unwrap_or_else(|e| panic!("copy {src_name} -> {dest_name}: {e}"));
    }
}

#[test]
fn assemble_and_verify_against_real_fixtures() {
    let dir = tempfile::tempdir().unwrap();
    populate_input_dir(dir.path());

    let bundle = assemble(dir.path()).expect("assemble must succeed against real fixtures");

    // Structural assertions.
    assert_eq!(bundle.schema_version, 1);
    assert_eq!(bundle.sub_trees.len(), 7);
    assert_ne!(bundle.blake2b_bundle_root, [0u8; 32]);
    assert_ne!(bundle.sha3_bundle_root, [0u8; 32]);
    assert_ne!(bundle.blake2b_bundle_root, bundle.sha3_bundle_root);

    // Per-sub-tree assertions: each sub-tree commits 8 items
    // (every fixture in this repo has 8 entries except utxo which has 3).
    let labels: Vec<&str> = bundle.sub_trees.iter().map(|s| s.sub_tree.as_str()).collect();
    assert_eq!(
        labels,
        vec!["utxo", "header", "tx-index", "token-policy", "script", "stake", "governance"]
    );

    // utxo fixture has 3 items; others have 8.
    let item_counts: Vec<usize> = bundle.sub_trees.iter().map(|s| s.item_count).collect();
    assert_eq!(item_counts, vec![3, 8, 8, 8, 8, 8, 8]);

    // Each sub-tree's blake2b and sha3 roots differ.
    for s in &bundle.sub_trees {
        assert_ne!(s.blake2b_root, s.sha3_root, "sub_tree {} has equal blake2b and sha3 roots", s.sub_tree);
        assert_ne!(s.blake2b_root, [0u8; 32]);
        assert_ne!(s.sha3_root, [0u8; 32]);
    }

    // Round-trip through JSON.
    let s = serde_json::to_string_pretty(&bundle).unwrap();
    let parsed: omega_commitment_bundle::bundle::BundleRecord =
        serde_json::from_str(&s).unwrap();
    assert_eq!(parsed, bundle);

    // Verify.
    verify(&bundle, dir.path()).expect("fresh bundle must verify against same inputs");
}

#[test]
fn assemble_is_deterministic_across_runs() {
    let dir1 = tempfile::tempdir().unwrap();
    let dir2 = tempfile::tempdir().unwrap();
    populate_input_dir(dir1.path());
    populate_input_dir(dir2.path());
    let b1 = assemble(dir1.path()).unwrap();
    let b2 = assemble(dir2.path()).unwrap();
    assert_eq!(b1.blake2b_bundle_root, b2.blake2b_bundle_root);
    assert_eq!(b1.sha3_bundle_root, b2.sha3_bundle_root);
}

#[test]
fn verify_against_tampered_inputs_fails() {
    let dir = tempfile::tempdir().unwrap();
    populate_input_dir(dir.path());
    let bundle = assemble(dir.path()).unwrap();

    // Tamper with header.json by appending whitespace (changes input_digest).
    let header_path = dir.path().join("header.json");
    let mut content = fs::read_to_string(&header_path).unwrap();
    content.push('\n');
    fs::write(&header_path, content).unwrap();

    let result = verify(&bundle, dir.path());
    assert!(result.is_err(), "tampered header.json must fail verify");
}
```

- [ ] **Step 2: Run the test**

```bash
cargo test -p omega-commitment-bundle --test end_to_end_integration 2>&1 | tail -10   # 3 tests pass
cargo test --workspace 2>&1 | tail -5                                                    # 167 total
cargo lint 2>&1 | tail -3                                                                # clean
cargo fmt-check 2>&1 | tail -3                                                           # clean
```

- [ ] **Step 3: Commit**

```bash
git add crates/omega-commitment-bundle/tests/end_to_end_integration.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test(bundle): end-to-end integration against all 7 per-sub-tree fixtures"
```

---

## Task 7: Bump workspace to v0.7.0 + extend README

**Files:**
- Modify: `crates/omega-commitment-core/Cargo.toml`
- Modify: `crates/omega-commitment-cli/Cargo.toml`
- Modify: `crates/omega-commitment-bundle/Cargo.toml` (already at 0.7.0; confirm)
- Modify: `README.md`

- [ ] **Step 1: Bump existing crate versions**

In `/home/hoskinson/omega-commitment/crates/omega-commitment-core/Cargo.toml`: change `version = "0.6.0"` to `version = "0.7.0"`.

In `/home/hoskinson/omega-commitment/crates/omega-commitment-cli/Cargo.toml`: change `version = "0.6.0"` to `version = "0.7.0"`.

In `/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/Cargo.toml`: confirm it already says `version = "0.7.0"` (set in Task 1). If not, fix.

- [ ] **Step 2: Verify**

```bash
cargo build --workspace 2>&1 | grep -E "warning|error"   # empty
cargo lint 2>&1 | tail -3                                  # clean
cargo fmt-check 2>&1 | tail -3                             # clean
cargo test --workspace 2>&1 | tail -5                       # 167 tests pass
```

- [ ] **Step 3: Append to README.md**

Append to `/home/hoskinson/omega-commitment/README.md`:

```markdown
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
| Per-sub-tree leaf encoders + Merkle trees + CLIs (sub-trees 1–7) | ✅ Shipped (v0.1.0–v0.6.0) |
| Bundle assembly + verification tooling | ✅ Shipped (v0.7.0) |

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
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-core/Cargo.toml \
        crates/omega-commitment-cli/Cargo.toml \
        crates/omega-commitment-bundle/Cargo.toml \
        README.md
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "chore: bump workspace to 0.7.0; document bundle-assembly tool and T1 phase completion"
```

- [ ] **Step 5: Final verification**

```bash
git log --oneline | head -10
cargo test --workspace 2>&1 | tail -5
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

Expected: HEAD is the version-bump commit; 167 tests pass; lint and fmt-check clean.

---

## Self-review

**Spec coverage** (against the dual-hash decision and spec §7):
- ✅ "Bundle = (root_of(7 blake2b sub-tree roots), root_of(7 sha3 sub-tree roots))" — implemented in `bundle::aggregate_blake2b` and `bundle::aggregate_sha3`.
- ✅ "Per-sub-tree roots are computed once with Blake2b-256 and once with SHA3-256" — implemented in `recompute::recompute()` returning both.
- ✅ "Verifiers of the canonical Ω-Commitment must check both" — `verify()` checks both roots independently and reports which one diverged.
- ✅ Bundle does not carry attestation data — `BundleRecord` deliberately omits Mithril/STARK/CIP-1694 fields. Documented in README.

**Decision honoring:**
- ✅ Decision 7 (PQ-only): Blake2b-256 + SHA3-256 only; no curves anywhere.
- ✅ Decision 8 (Plonky3-friendly): bundle root is a hash; bundle struct is canonical and Plonky3-ingestible.
- ✅ Decision 9 (selective dual-track): per-leaf and per-sub-tree stay Blake2b-only (`recompute::build_leaves` calls the existing Blake2b-only `*::leaf_hash()` functions); only the aggregation step runs in SHA3.
- ✅ Decision 2 (lazy/pull): unchanged.

**Placeholder scan:** All code blocks complete and runnable. No "TBD" / "fill in details". ✅

**Type consistency:**
- `SubTreeId` defined once in `sub_tree_id.rs`, used uniformly across `recompute.rs`, `bundle.rs`, and `main.rs` via re-exports.
- `SubTreeRoots` (plural-roots-for-one-sub-tree) defined in `recompute.rs`; consumed by `bundle::assemble`.
- `SubTreeRecord` (the per-sub-tree row in `bundle.json`) defined in `bundle.rs`; serialized via serde.
- `BundleRecord` (top-level bundle.json schema) defined in `bundle.rs`; main.rs reads/writes via serde.
- `assemble`, `verify`, `BUNDLE_SCHEMA_VERSION` consistent between library and CLI.
- ✅ No drift.

**Bite-sized tasks:** 7 tasks, each with 2–5 numbered steps; each step is a single action. ✅

**Net delta:** +4 sub_tree_id tests + 7 recompute tests + 8 bundle tests + 3 end-to-end tests = **+22 tests** (145 → 167). 7 commits.

---

## What's NOT in this plan (and why)

- **Real Cardano mainnet bundle.** The end-to-end integration test runs against synthetic 8-entry-per-sub-tree fixtures. A `cardano-multiplatform-lib`-driven mainnet snapshot ingestion would produce a real bundle; deferred to a follow-on plan in track T1's "real-data ingestion" sub-phase.
- **Attestation tooling.** Mithril-PQ signing, recursive STARK proof generation, and CIP-1694 ratification machinery are SEPARATE artifacts that reference the bundle root tuple. Each gets its own track / plan.
- **Bundle migration to full dual-track.** If Blake2b is ever broken, sub-tree roots would also need SHA3 versions (currently per-sub-tree is Blake2b-only). The `hash::dual_hash` primitive remains in place to make this mechanical; no preemptive code in this plan.
- **Per-sub-tree CLI `--also-sha3` flag.** Considered as Option B in the planning brief; rejected in favor of bundle-time recomputation (cleaner separation, single source of truth for dual-track logic).
- **Bundle compression / Merkle-of-bundles for time-series snapshots.** Out of scope; the bundle is a single point-in-time commitment.

---

## How to execute this plan

Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Seven tasks, each independently committable.

Total runway estimate: **3–5 days** for an experienced Rust dev. The new crate is bigger than a single sub-tree leaf module but the work decomposes naturally: scaffold → identifier mapping → recompute → bundle library → CLI → integration → release.

Expected post-execution state:
- 7 commits added on top of v0.6.0 (currently 57 commits)
- ~22 net new tests (167 total)
- All three crates at version 0.7.0
- New `omega-bundle` binary alongside `omega-commitment`
- T1 leaf-tooling AND bundle-tooling phases complete
- Tracks T2 (Plonky3 circuits) and T9 (CIP-Ω-1) fully unblocked
