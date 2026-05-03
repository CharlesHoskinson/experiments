# Omega Transaction Index Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the transaction-index sub-tree (sub-tree 3 of 7) to the omega-commitment workspace, enabling `claim_tx` proofs that "tx H existed on the old chain at slot S, in block B at position P." Plus a small carry-over: mark `LeafError` `#[non_exhaustive]` to prevent future SemVer breaks.

**Architecture:** Reuse `tree.rs` and `witness.rs` unchanged (now validated across two prior sub-trees). Add `tx_index_leaf.rs` with a fixed-width 76-byte canonical encoding (tx_id ‖ slot ‖ block_hash ‖ tx_position). CLI gains a `--sub-tree tx-index` arm. Bump to v0.3.0.

**Tech Stack:** Rust 1.79+, blake2, sha3, serde, clap (no new deps).

**Track:** T1 (Ω-Commitment Tooling), sub-tree 3. See `2026-05-01-ouroboros-omega-program-roadmap.md`.

**Locked design decisions honored:**
- Decision 7 (PQ-only crypto): Blake2b-256 + SHA3-256 only.
- Decision 8 (Plonky3-friendly): same MerkleTree (binary, fixed-arity, sorted-padded). Tx leaves sort by leaf hash; verifiers reconstruct from preimage.
- Decision 3 (everything-provable): adding sub-tree 3 of 7. Four remaining: token policies, script registry, stake state, governance state.

**Carry-over from v0.2.0 final review:**
- Dual-track shadow hash decision STILL deferred. Plonky3 circuit authors must NOT lock to v0.3.0 single-root format until decided.
- `#[non_exhaustive]` on `LeafError` — addressed in this plan (Task 5).
- CLI dispatcher refactor (trait/registry) — still deferred. Current 3-arm `match` is fine.
- Hardening backlog (path traversal, input size cap, atomic write, layer cloning, hex codec dup, clippy CI) — STILL deferred. Recommended before sub-tree 4.

---

## File structure (post-plan)

```
omega-commitment/
├── Cargo.toml                                           (workspace; per-crate version bump to 0.3.0)
├── README.md                                            (extended: tx-index section, breaking-changes note)
├── crates/
│   ├── omega-commitment-core/
│   │   ├── Cargo.toml                                   (version bump)
│   │   ├── src/
│   │   │   ├── lib.rs                                   (add `pub mod tx_index_leaf`)
│   │   │   ├── hash.rs                                  (unchanged)
│   │   │   ├── utxo_leaf.rs                             (modify: #[non_exhaustive] on LeafError)
│   │   │   ├── header_leaf.rs                           (unchanged)
│   │   │   ├── tx_index_leaf.rs                         (NEW)
│   │   │   ├── tree.rs                                  (unchanged)
│   │   │   └── witness.rs                               (unchanged)
│   │   ├── tests/
│   │   │   ├── fixtures/
│   │   │   │   ├── utxo_set_small.json                  (existing)
│   │   │   │   ├── header_chain_small.json              (existing)
│   │   │   │   └── tx_index_small.json                  (NEW)
│   │   │   ├── utxo_integration.rs                      (existing)
│   │   │   ├── header_integration.rs                    (existing)
│   │   │   └── tx_index_integration.rs                  (NEW)
│   │   └── benches/tree.rs                              (unchanged)
│   └── omega-commitment-cli/
│       ├── Cargo.toml                                   (version bump)
│       ├── src/main.rs                                  (modify: add TxIndex arm to SubTree enum + dispatcher)
│       └── tests/cli.rs                                 (extend: tx-index smoke test)
```

---

## Task 1: `tx_index_leaf.rs` — canonical transaction index encoding

**Files:**
- Create: `crates/omega-commitment-core/src/tx_index_leaf.rs`
- Modify: `crates/omega-commitment-core/src/lib.rs`

A tx-index leaf is the deterministic serialization of:
```
tx_id (32 bytes) || slot (u64 BE) || block_hash (32 bytes) || tx_position (u32 BE)
```
Total: 76 bytes per entry. Fixed-width — no length prefixes. Hashed with Blake2b-256.

- [ ] **Step 1: Create `tx_index_leaf.rs`**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/src/tx_index_leaf.rs`

```rust
//! Canonical transaction-index leaf encoding.
//!
//! A tx-index leaf is the deterministic serialization of:
//!   (tx_id: 32 bytes) || (slot: u64 BE) ||
//!   (block_hash: 32 bytes) || (tx_position: u32 BE)
//!
//! Total: 76 bytes. The leaf is hashed with Blake2b-256 to produce
//! the leaf hash that goes into the Merkle tree. This sub-tree powers
//! `claim_tx` transactions: users prove "tx H existed at slot S in
//! block B at position P."

use crate::hash::{blake2b_256, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TxIndexEntry {
    #[serde(with = "hex::serde")]
    pub tx_id: [u8; 32],
    pub slot: u64,
    #[serde(with = "hex::serde")]
    pub block_hash: [u8; 32],
    pub tx_position: u32,
}

impl TxIndexEntry {
    /// Canonical 76-byte serialization.
    pub fn encode(&self) -> [u8; 76] {
        let mut out = [0u8; 76];
        out[0..32].copy_from_slice(&self.tx_id);
        out[32..40].copy_from_slice(&self.slot.to_be_bytes());
        out[40..72].copy_from_slice(&self.block_hash);
        out[72..76].copy_from_slice(&self.tx_position.to_be_bytes());
        out
    }

    /// Compute the leaf hash: Blake2b-256 of canonical encoding.
    pub fn leaf_hash(&self) -> Hash {
        blake2b_256(&self.encode())
    }
}

/// Validate that no `tx_id` appears more than once across the entries.
/// Returns the index of the second occurrence of the first duplicate
/// found, or None if all `tx_id`s are unique.
///
/// Cardano transaction hashes are deterministic functions of the tx
/// body and should be unique across the whole chain. Duplicate input
/// is a data error (e.g., a snapshot with overlapping epoch ranges).
/// This is an OPTIONAL sanity helper; commitment generation does NOT
/// require uniqueness.
pub fn validate_tx_uniqueness(entries: &[TxIndexEntry]) -> Option<usize> {
    let mut seen: HashSet<[u8; 32]> = HashSet::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        if !seen.insert(e.tx_id) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(tx_id_byte: u8, slot: u64, pos: u32) -> TxIndexEntry {
        TxIndexEntry {
            tx_id: [tx_id_byte; 32],
            slot,
            block_hash: [0xCC; 32],
            tx_position: pos,
        }
    }

    #[test]
    fn encoding_is_exactly_76_bytes() {
        let e = sample(0x11, 100, 0);
        assert_eq!(e.encode().len(), 76);
    }

    #[test]
    fn encoding_layout_is_correct() {
        let e = TxIndexEntry {
            tx_id: [0xAAu8; 32],
            slot: 0x0102030405060708,
            block_hash: [0xBBu8; 32],
            tx_position: 0x11223344,
        };
        let bytes = e.encode();
        assert_eq!(&bytes[0..32], &[0xAAu8; 32]);
        assert_eq!(&bytes[32..40], &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(&bytes[40..72], &[0xBBu8; 32]);
        assert_eq!(&bytes[72..76], &[0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let e = sample(0x11, 100, 0);
        assert_eq!(e.leaf_hash(), e.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_tx_id_change() {
        let a = sample(0x11, 100, 0);
        let b = sample(0x12, 100, 0);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_slot_change() {
        let a = sample(0x11, 100, 0);
        let b = sample(0x11, 101, 0);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_position_change() {
        let a = sample(0x11, 100, 0);
        let b = sample(0x11, 100, 1);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_block_hash_change() {
        let a = sample(0x11, 100, 0);
        let mut b = a.clone();
        b.block_hash = [0xDD; 32];
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn validate_tx_uniqueness_accepts_unique() {
        let entries = vec![
            sample(0x01, 1, 0),
            sample(0x02, 2, 0),
            sample(0x03, 3, 0),
        ];
        assert_eq!(validate_tx_uniqueness(&entries), None);
    }

    #[test]
    fn validate_tx_uniqueness_finds_duplicate() {
        let entries = vec![
            sample(0x01, 1, 0),
            sample(0x02, 2, 0),
            sample(0x01, 5, 1), // duplicate tx_id at index 2
        ];
        assert_eq!(validate_tx_uniqueness(&entries), Some(2));
    }

    #[test]
    fn validate_tx_uniqueness_empty_is_valid() {
        assert_eq!(validate_tx_uniqueness(&[]), None);
    }

    #[test]
    fn same_tx_id_different_slot_still_distinct_leaves() {
        // Two entries with same tx_id but different (slot, position) must
        // produce different leaf hashes — confirms the entire tuple
        // contributes to the leaf identity.
        let a = sample(0x11, 100, 0);
        let b = sample(0x11, 200, 0);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }
}
```

- [ ] **Step 2: Update `crates/omega-commitment-core/src/lib.rs`**

REPLACE its full contents with:

```rust
//! omega-commitment-core: Ω-Commitment sub-tree library.
//!
//! Provides canonical leaf encodings, a Plonky3-friendly Merkle tree, and
//! inclusion witnesses. v0.3.0 supports three of seven Ω-Commitment sub-trees:
//! UTXO set, block header chain, and transaction index.

pub mod hash;
pub mod tree;
pub mod witness;
pub mod utxo_leaf;
pub mod header_leaf;
pub mod tx_index_leaf;
```

- [ ] **Step 3: Run tests**

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
cargo test -p omega-commitment-core tx_index_leaf::tests 2>&1 | tail -15
```

Expected: 10 tests pass.

- [ ] **Step 4: Run full workspace**

```bash
cargo test --workspace 2>&1 | tail -5
```

Expected: 52 tests pass (42 prior + 10 new).

- [ ] **Step 5: Verify no warnings**

```bash
cargo build --workspace 2>&1 | grep -E "warning|error"   # empty
```

- [ ] **Step 6: Commit**

```bash
git add crates/omega-commitment-core/src/tx_index_leaf.rs \
        crates/omega-commitment-core/src/lib.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(tx_index_leaf): canonical 76-byte encoding + uniqueness validator"
```

---

## Task 2: Tx-index integration test

**Files:**
- Create: `crates/omega-commitment-core/tests/fixtures/tx_index_small.json`
- Create: `crates/omega-commitment-core/tests/tx_index_integration.rs`

8-entry synthetic fixture covering: distinct tx_ids, varied positions within a block (slot 5 has 3 transactions at positions 0/1/2), varied slots.

- [ ] **Step 1: Write fixture**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/tests/fixtures/tx_index_small.json`

```json
{
  "entries": [
    {
      "tx_id": "1100000000000000000000000000000000000000000000000000000000000000",
      "slot": 1,
      "block_hash": "aa00000000000000000000000000000000000000000000000000000000000000",
      "tx_position": 0
    },
    {
      "tx_id": "2200000000000000000000000000000000000000000000000000000000000000",
      "slot": 2,
      "block_hash": "bb00000000000000000000000000000000000000000000000000000000000000",
      "tx_position": 0
    },
    {
      "tx_id": "3300000000000000000000000000000000000000000000000000000000000000",
      "slot": 5,
      "block_hash": "cc00000000000000000000000000000000000000000000000000000000000000",
      "tx_position": 0
    },
    {
      "tx_id": "4400000000000000000000000000000000000000000000000000000000000000",
      "slot": 5,
      "block_hash": "cc00000000000000000000000000000000000000000000000000000000000000",
      "tx_position": 1
    },
    {
      "tx_id": "5500000000000000000000000000000000000000000000000000000000000000",
      "slot": 5,
      "block_hash": "cc00000000000000000000000000000000000000000000000000000000000000",
      "tx_position": 2
    },
    {
      "tx_id": "6600000000000000000000000000000000000000000000000000000000000000",
      "slot": 7,
      "block_hash": "dd00000000000000000000000000000000000000000000000000000000000000",
      "tx_position": 0
    },
    {
      "tx_id": "7700000000000000000000000000000000000000000000000000000000000000",
      "slot": 11,
      "block_hash": "ee00000000000000000000000000000000000000000000000000000000000000",
      "tx_position": 0
    },
    {
      "tx_id": "8800000000000000000000000000000000000000000000000000000000000000",
      "slot": 11,
      "block_hash": "ee00000000000000000000000000000000000000000000000000000000000000",
      "tx_position": 1
    }
  ]
}
```

- [ ] **Step 2: Write integration test**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/tests/tx_index_integration.rs`

```rust
//! End-to-end integration test for the transaction-index sub-tree.

use omega_commitment_core::{
    tx_index_leaf::{validate_tx_uniqueness, TxIndexEntry},
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    entries: Vec<TxIndexEntry>,
}

const FIXTURE: &str = include_str!("fixtures/tx_index_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.entries.len(), 8);

    // Sanity: tx_ids are unique.
    assert!(validate_tx_uniqueness(&f.entries).is_none(),
            "fixture has duplicate tx_ids");

    let leaves: Vec<_> = f.entries.iter().map(|e| e.leaf_hash()).collect();
    let tree = MerkleTree::build(leaves.clone());
    assert_eq!(tree.leaf_count(), 8); // already power of two
    assert_eq!(tree.depth(), 3);
    let root = tree.root();
    assert_ne!(root, [0u8; 32]);

    for leaf in leaves {
        let w = InclusionWitness::build(&tree, leaf)
            .expect("leaf is in tree");
        assert!(w.verify(root), "witness verification failed");
    }
}

#[test]
fn root_is_stable_across_runs() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let leaves1: Vec<_> = f.entries.iter().map(|e| e.leaf_hash()).collect();
    let leaves2: Vec<_> = f.entries.iter().map(|e| e.leaf_hash()).collect();
    assert_eq!(MerkleTree::build(leaves1).root(),
               MerkleTree::build(leaves2).root());
}

#[test]
fn same_block_txs_get_distinct_leaves() {
    // Slot 5 has 3 entries (positions 0/1/2). All should hash distinctly.
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let slot5: Vec<_> = f.entries.iter()
        .filter(|e| e.slot == 5)
        .map(|e| e.leaf_hash())
        .collect();
    assert_eq!(slot5.len(), 3);
    assert_ne!(slot5[0], slot5[1]);
    assert_ne!(slot5[1], slot5[2]);
    assert_ne!(slot5[0], slot5[2]);
}

#[test]
fn duplicate_tx_id_rejected_by_validator() {
    // Construct an input that would violate uniqueness.
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let mut entries = f.entries;
    let dup = entries[0].clone();
    entries.push(dup);
    assert_eq!(validate_tx_uniqueness(&entries), Some(8));
}
```

- [ ] **Step 3: Run integration tests**

```bash
cargo test -p omega-commitment-core --test tx_index_integration 2>&1 | tail -10
```

Expected: 4 tests pass.

- [ ] **Step 4: Run full workspace**

```bash
cargo test --workspace 2>&1 | tail -5
```

Expected: 56 tests pass (52 prior + 4 new).

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-core/tests/tx_index_integration.rs \
        crates/omega-commitment-core/tests/fixtures/tx_index_small.json
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test: tx-index sub-tree integration test against synthetic 8-entry fixture"
```

---

## Task 3: CLI `--sub-tree tx-index` arm

**Files:**
- Modify: `crates/omega-commitment-cli/src/main.rs`

Add a third arm to the `SubTree` enum and the dispatcher in `commit()`. The 3-arm `match` is still readable; defer the trait/registry refactor until sub-tree 4.

- [ ] **Step 1: Replace `crates/omega-commitment-cli/src/main.rs`**

```rust
//! omega-commitment CLI.
//!
//! Subcommand `commit`: read a JSON sub-tree input, emit:
//!   - a `commitment.json` containing the root + metadata + input digest
//!   - a `witnesses/<leaf_hash>.json` per leaf

use clap::{Parser, Subcommand, ValueEnum};
use omega_commitment_core::{
    hash::{blake2b_256, Hash},
    header_leaf::BlockHeader,
    tree::MerkleTree,
    tx_index_leaf::TxIndexEntry,
    utxo_leaf::Utxo,
    witness::InclusionWitness,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Parser)]
#[command(name = "omega-commitment", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build a sub-tree commitment from a JSON input.
    Commit {
        /// Input JSON file. Schema depends on --sub-tree.
        #[arg(short, long)]
        input: PathBuf,
        /// Output directory.
        #[arg(short, long)]
        output: PathBuf,
        /// Which Ω-Commitment sub-tree to build.
        #[arg(short = 's', long, value_enum, default_value_t = SubTree::Utxo)]
        sub_tree: SubTree,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SubTree {
    Utxo,
    Header,
    TxIndex,
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

#[derive(Serialize)]
struct CommitmentRecord {
    sub_tree: SubTree,
    /// Blake2b-256 of the raw input file bytes.
    #[serde(with = "hex::serde")]
    input_digest: Hash,
    #[serde(with = "hex::serde")]
    root: Hash,
    leaf_count: usize,
    tree_depth: usize,
    item_count: usize,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Commit { input, output, sub_tree } => commit(input, output, sub_tree),
    }
}

fn commit(input: PathBuf, output: PathBuf, sub_tree: SubTree) -> anyhow::Result<()> {
    let raw = fs::read_to_string(&input)?;
    let input_digest = blake2b_256(raw.as_bytes());

    let (leaves, item_count) = match sub_tree {
        SubTree::Utxo => {
            let parsed: UtxoInput = serde_json::from_str(&raw)?;
            let leaves: Vec<Hash> = parsed.utxos.iter()
                .map(|u| u.leaf_hash())
                .collect::<Result<Vec<_>, _>>()?;
            let n = parsed.utxos.len();
            (leaves, n)
        }
        SubTree::Header => {
            let parsed: HeaderInput = serde_json::from_str(&raw)?;
            let leaves: Vec<Hash> = parsed.headers.iter()
                .map(|h| h.leaf_hash())
                .collect();
            let n = parsed.headers.len();
            (leaves, n)
        }
        SubTree::TxIndex => {
            let parsed: TxIndexInput = serde_json::from_str(&raw)?;
            let leaves: Vec<Hash> = parsed.entries.iter()
                .map(|e| e.leaf_hash())
                .collect();
            let n = parsed.entries.len();
            (leaves, n)
        }
    };

    let tree = MerkleTree::build(leaves.clone());

    fs::create_dir_all(&output)?;
    let witness_dir = output.join("witnesses");
    fs::create_dir_all(&witness_dir)?;

    let record = CommitmentRecord {
        sub_tree,
        input_digest,
        root: tree.root(),
        leaf_count: tree.leaf_count(),
        tree_depth: tree.depth(),
        item_count,
    };
    fs::write(
        output.join("commitment.json"),
        serde_json::to_string_pretty(&record)?,
    )?;

    let mut leaf_idx: HashMap<Hash, u32> = HashMap::with_capacity(tree.leaf_count());
    for (i, h) in tree.leaves().iter().enumerate() {
        leaf_idx.entry(*h).or_insert(i as u32);
    }
    for leaf in leaves {
        let idx = *leaf_idx.get(&leaf)
            .ok_or_else(|| anyhow::anyhow!("leaf vanished from tree"))?;
        let w = InclusionWitness::build_at_index(&tree, idx)
            .ok_or_else(|| anyhow::anyhow!("index out of range"))?;
        let fname = format!("{}.json", hex::encode(leaf));
        fs::write(
            witness_dir.join(fname),
            serde_json::to_string_pretty(&w)?,
        )?;
    }

    println!("ok: sub_tree={:?} root={} input_digest={} items={}",
        record.sub_tree,
        hex::encode(record.root),
        hex::encode(record.input_digest),
        record.item_count);
    Ok(())
}
```

Note: the serde `rename_all` on `SubTree` changed from `"lowercase"` (v0.2.0) to `"kebab-case"`. This is a TINY breaking change for serialized output: `Utxo` still renders as `"utxo"` and `Header` still as `"header"` (single-word lowercase forms unchanged), but `TxIndex` will render as `"tx-index"` rather than `"txindex"`. This matches the CLI flag spelling (`--sub-tree tx-index`) so users see one consistent identifier.

- [ ] **Step 2: Build and run all 3 sub-trees end-to-end**

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
cargo build --release -p omega-commitment-cli 2>&1 | tail -5
```

UTXO smoke:
```bash
mkdir -p /tmp/o-u && rm -rf /tmp/o-u/*
./target/release/omega-commitment commit --sub-tree utxo \
  --input crates/omega-commitment-core/tests/fixtures/utxo_set_small.json \
  --output /tmp/o-u
cat /tmp/o-u/commitment.json
```
Expected: `"sub_tree": "utxo"`, item_count 3.

Header smoke:
```bash
mkdir -p /tmp/o-h && rm -rf /tmp/o-h/*
./target/release/omega-commitment commit --sub-tree header \
  --input crates/omega-commitment-core/tests/fixtures/header_chain_small.json \
  --output /tmp/o-h
cat /tmp/o-h/commitment.json
```
Expected: `"sub_tree": "header"`, item_count 8.

Tx-index smoke:
```bash
mkdir -p /tmp/o-t && rm -rf /tmp/o-t/*
./target/release/omega-commitment commit --sub-tree tx-index \
  --input crates/omega-commitment-core/tests/fixtures/tx_index_small.json \
  --output /tmp/o-t
cat /tmp/o-t/commitment.json
ls /tmp/o-t/witnesses/ | wc -l
```
Expected: `"sub_tree": "tx-index"`, item_count 8, 8 witness files.

Default flag smoke (no --sub-tree → utxo):
```bash
mkdir -p /tmp/o-d && rm -rf /tmp/o-d/*
./target/release/omega-commitment commit \
  --input crates/omega-commitment-core/tests/fixtures/utxo_set_small.json \
  --output /tmp/o-d
cat /tmp/o-d/commitment.json
```
Expected: defaults to `"sub_tree": "utxo"`.

- [ ] **Step 3: Run existing tests (smoke test will pass — see note below)**

```bash
cargo test --workspace 2>&1 | tail -10
```

Note: the existing v0.2.0 `cli_commit_utxo_smoke` and `cli_commit_header_smoke` tests use substring assertions like `"\"sub_tree\": \"utxo\""` which still match (single-word lowercase tags unchanged). They should pass without modification. Task 4 adds a tx-index smoke test.

If any existing test fails, STOP and report — that means the kebab-case change broke something unexpected.

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-cli/src/main.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(cli): add tx-index arm to --sub-tree dispatcher"
```

---

## Task 4: CLI smoke test for tx-index

**Files:**
- Modify: `crates/omega-commitment-cli/tests/cli.rs`

Add one new test parallel to the existing utxo/header smoke tests.

- [ ] **Step 1: Append new test**

Edit `/home/hoskinson/omega-commitment/crates/omega-commitment-cli/tests/cli.rs`. Append AT THE END (after the existing `cli_commit_default_is_utxo` test):

```rust

#[test]
fn cli_commit_tx_index_smoke() {
    let out = run_commit("tx-index", "tx_index_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(body.contains("\"sub_tree\": \"tx-index\""), "wrong sub_tree tag: {body}");
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 8"));
    let witness_count = fs::read_dir(out.path().join("witnesses"))
        .unwrap().count();
    assert_eq!(witness_count, 8, "expected 8 tx-index witness files");
}
```

(`run_commit` and `fixture_path` are already defined at the top of the file from v0.2.0; reuse them.)

- [ ] **Step 2: Run CLI tests**

```bash
cargo test -p omega-commitment-cli --test cli 2>&1 | tail -10
```

Expected: 4 tests pass (3 prior + 1 new).

- [ ] **Step 3: Run full workspace**

```bash
cargo test --workspace 2>&1 | tail -5
```

Expected: 57 tests pass (56 prior + 1 new).

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-cli/tests/cli.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test(cli): tx-index smoke test"
```

---

## Task 5: Mark `LeafError` `#[non_exhaustive]` (carry-over fix)

**Files:**
- Modify: `crates/omega-commitment-core/src/utxo_leaf.rs`

Pre-empts a SemVer break if a future variant lands. v0.2.0 final review flagged this as cheap-to-fix-now-painful-later.

- [ ] **Step 1: Edit `LeafError` declaration**

In `/home/hoskinson/omega-commitment/crates/omega-commitment-core/src/utxo_leaf.rs`, find the existing enum (it currently looks like):

```rust
#[derive(Debug, Error)]
pub enum LeafError {
    #[error("asset count exceeds u32::MAX")]
    AssetCountOverflow,
    #[error("asset_id length exceeds u16::MAX")]
    AssetIdLenOverflow,
}
```

Change it to add `#[non_exhaustive]` directly above the `pub enum` line:

```rust
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LeafError {
    #[error("asset count exceeds u32::MAX")]
    AssetCountOverflow,
    #[error("asset_id length exceeds u16::MAX")]
    AssetIdLenOverflow,
}
```

- [ ] **Step 2: Verify build + tests**

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
cargo build --workspace 2>&1 | grep -E "warning|error"   # empty
cargo test --workspace 2>&1 | tail -5                      # 57 tests pass
```

Note: `#[non_exhaustive]` is purely a downstream-API hint — it doesn't change construction or matching inside the defining crate. All existing tests continue to pass.

- [ ] **Step 3: Commit**

```bash
git add crates/omega-commitment-core/src/utxo_leaf.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "chore(utxo_leaf): mark LeafError #[non_exhaustive] before external consumers"
```

---

## Task 6: Bump to v0.3.0 + extend README

**Files:**
- Modify: `crates/omega-commitment-core/Cargo.toml`
- Modify: `crates/omega-commitment-cli/Cargo.toml`
- Modify: `README.md`

- [ ] **Step 1: Bump core crate version**

In `/home/hoskinson/omega-commitment/crates/omega-commitment-core/Cargo.toml`, change `version = "0.2.0"` to `version = "0.3.0"`.

- [ ] **Step 2: Bump CLI crate version**

In `/home/hoskinson/omega-commitment/crates/omega-commitment-cli/Cargo.toml`, change `version = "0.2.0"` to `version = "0.3.0"`.

- [ ] **Step 3: Verify build + tests**

```bash
cargo build --workspace 2>&1 | grep -E "warning|error"   # empty
cargo test --workspace 2>&1 | tail -5                      # 57 tests pass
```

- [ ] **Step 4: Append README section**

Append to `/home/hoskinson/omega-commitment/README.md`:

```markdown
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
```

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-core/Cargo.toml \
        crates/omega-commitment-cli/Cargo.toml \
        README.md
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "chore: bump to 0.3.0; document tx-index sub-tree + hardening backlog"
```

- [ ] **Step 6: Final verification**

```bash
git log --oneline | head -8
cargo test --workspace 2>&1 | tail -5
```

Expected: `chore: bump to 0.3.0` is HEAD; 57 tests pass.

---

## Self-review

**Spec coverage** (from `2026-05-01-ouroboros-omega-design.md` §7, sub-tree 3):
- "Transaction index — Merkle tree of all tx hashes ever included on the old chain, mapping `tx_id → (slot, block_hash, tx_position)`."
  - ✅ Leaf encodes (tx_id, slot, block_hash, tx_position) — exact spec match.
  - ✅ Plonky3-friendly tree reused unchanged.
  - ✅ Witness format unchanged from sub-trees 1 & 2.
  - ✅ `claim_tx` (spec §9.3) gets a tree to prove against.

**Decision honoring:**
- ✅ Decision 7 (PQ-only): only Blake2b for leaf hashing, no curves anywhere.
- ✅ Decision 8 (Plonky3-friendly): fixed-width 76-byte leaf encoding, sort-then-pad tree.
- ✅ Decision 3 (everything-provable): adds 3rd of 7 sub-trees.

**Carry-over status:**
- ✅ `#[non_exhaustive]` on `LeafError` — Task 5.
- ⏸️ CLI dispatcher refactor — deferred (3-arm match still readable).
- ⏸️ Hardening backlog — deferred to between sub-tree 3 and 4 per reviewer recommendation.
- ⏸️ Dual-hash decision — program-level pending, plan does not change anything.

**Placeholder scan:** All code blocks complete and runnable. No "TBD"s in implementation steps. ✅

**Type consistency:**
- `TxIndexEntry { tx_id: [u8;32], slot: u64, block_hash: [u8;32], tx_position: u32 }` referenced consistently across `tx_index_leaf.rs`, the JSON fixture, the integration test, and the CLI `TxIndexInput`.
- `SubTree` enum: `Utxo`, `Header`, `TxIndex` — kebab-case rename means stored form is `"utxo"`, `"header"`, `"tx-index"` matching CLI flag spelling.
- `validate_tx_uniqueness` signature consistent throughout.
- ✅ No drift.

**Bite-sized tasks:** 6 tasks, each with 2–6 numbered steps; each step is one action. ✅

**Net delta:** +14 tests (10 unit + 4 integration), +1 CLI smoke test, +1 carry-over fix, +1 version bump = ~57 total tests after execution.

---

## What's NOT in this plan (and why)

- **Real Cardano mainnet tx-index ingestion.** Synthetic fixtures only. A `cardano-multiplatform-lib` integration is deferred to a follow-on plan.
- **Cross-sub-tree validation.** A future `validate_against_header_subtree(tx_index, header_subtree)` helper that confirms each tx's `(slot, block_hash)` matches a header in the header sub-tree would be valuable but requires both commitments to be loaded simultaneously. Deferred — most naturally lives in a future "cross-validation" plan after all 7 sub-trees ship.
- **The remaining four sub-trees** (token policies, script registry, stake state, governance state) — each gets its own plan.
- **Plonky3 `claim_tx` circuit.** Track T2, separate plan.
- **Dual-track shadow hash.** Program-level pending decision, unchanged in this plan.
- **Hardening backlog items.** Reviewer recommended these before sub-tree 4. This plan acknowledges them but doesn't address them; a separate "v0.3.x hardening" plan should slot in after this one ships.

---

## How to execute this plan

Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Six tasks, each independently committable. Total runway estimate: **3–5 days** for an experienced Rust dev (smaller than v0.2.0 because there's no module rename; just one new file + small CLI extension + one carry-over).

Expected post-execution state:
- 6 commits added on top of v0.2.0 (currently 23 commits)
- ~15 net new tests (57 total)
- Both crates at version 0.3.0
- README documents 3 of 7 sub-trees + the hardening backlog
- Sub-tree 4 (token policies) plan slot is open

Next plan in this track: `2026-XX-XX-omega-token-policies-plan.md` (sub-tree 4 of 7) — but the program should consider a hardening sprint first.
