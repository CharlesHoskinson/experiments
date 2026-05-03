# Omega Block Header Accumulator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the block-header sub-tree (sub-tree 2 of 7) to the omega-commitment workspace, plus three carry-over improvements identified in the v0.1.0 review: rename `leaf.rs` → `utxo_leaf.rs`, add `input_digest` to `CommitmentRecord`, and extend the CLI to dispatch by `--sub-tree`.

**Architecture:** Reuse `tree.rs` and `witness.rs` (sub-tree agnostic). Add a new `header_leaf.rs` with a fixed-width `BlockHeader` canonical encoding (slot || block_height || block_hash || prev_hash). The CLI gains a `--sub-tree {utxo,header}` flag; today it routes between two paths, tomorrow between seven. Bump to v0.2.0.

**Tech Stack:** Rust 1.79+, blake2, sha3, serde, clap (continued from v0.1.0 stack). No new deps.

**Track:** T1 (Ω-Commitment Tooling), sub-tree 2. See `2026-05-01-ouroboros-omega-program-roadmap.md`.

**Locked design decisions honored:**
- Decision 7 (PQ-only crypto): Blake2b-256 + SHA3-256 only — continued.
- Decision 8 (Plonky3-friendly): same MerkleTree (binary, fixed-arity, sorted-padded). Header leaves sort by leaf hash; verifiers reconstruct the leaf from the preimage `(slot, block_height, block_hash, prev_hash)` and check inclusion.
- Decision 3 (everything-provable): adding sub-tree 2 of 7. Five remaining: tx index, token policies, script registry, stake state, governance state.

**Carry-over from v0.1.0 final review (open questions):**
- Dual-track shadow hash decision is STILL deferred. v0.2.0 continues to publish only the Blake2b root. Do not change this without an explicit program-level decision recorded in the program roadmap.
- Plonky3 circuit authors must NOT lock to v0.2.0 root format until the dual-hash decision is recorded.

---

## File structure (post-plan)

```
omega-commitment/
├── Cargo.toml                                           (workspace; bump to 0.2.0)
├── README.md                                            (extended; document header sub-tree + breaking change)
├── crates/
│   ├── omega-commitment-core/
│   │   ├── Cargo.toml                                   (version bump)
│   │   ├── src/
│   │   │   ├── lib.rs                                   (re-export new modules + SubTree enum)
│   │   │   ├── hash.rs                                  (unchanged)
│   │   │   ├── utxo_leaf.rs                             (RENAMED from leaf.rs)
│   │   │   ├── header_leaf.rs                           (NEW)
│   │   │   ├── tree.rs                                  (unchanged)
│   │   │   └── witness.rs                               (unchanged)
│   │   ├── tests/
│   │   │   ├── fixtures/
│   │   │   │   ├── utxo_set_small.json                  (existing)
│   │   │   │   └── header_chain_small.json              (NEW)
│   │   │   ├── utxo_integration.rs                      (RENAMED from integration.rs)
│   │   │   └── header_integration.rs                    (NEW)
│   │   └── benches/tree.rs                              (unchanged)
│   └── omega-commitment-cli/
│       ├── Cargo.toml                                   (version bump)
│       ├── src/main.rs                                  (refactor: --sub-tree dispatch + input_digest)
│       └── tests/cli.rs                                 (extend with header smoke test)
```

Each file has one clear responsibility:
- `utxo_leaf.rs`: UTXO-specific canonical encoding (existing logic, renamed).
- `header_leaf.rs`: header-specific canonical encoding (NEW).
- `tree.rs`, `witness.rs`, `hash.rs`: sub-tree agnostic, no changes.
- `lib.rs`: re-exports + a small `SubTree` enum the CLI uses to dispatch.

---

## Task 1: Rename `leaf.rs` → `utxo_leaf.rs` and update consumers

**Files:**
- Rename: `crates/omega-commitment-core/src/leaf.rs` → `crates/omega-commitment-core/src/utxo_leaf.rs`
- Modify: `crates/omega-commitment-core/src/lib.rs`
- Modify: `crates/omega-commitment-core/tests/integration.rs` (then rename to `utxo_integration.rs`)
- Modify: `crates/omega-commitment-cli/src/main.rs`

This is a breaking module rename (v0.2.0). It clears the path for `header_leaf.rs` to land alongside without naming awkwardness.

- [ ] **Step 1: Move the file**

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
git mv crates/omega-commitment-core/src/leaf.rs crates/omega-commitment-core/src/utxo_leaf.rs
```

- [ ] **Step 2: Update `crates/omega-commitment-core/src/lib.rs`**

Replace the contents with:

```rust
//! omega-commitment-core: Ω-Commitment sub-tree library.
//!
//! Provides canonical leaf encodings, a Plonky3-friendly Merkle tree, and
//! inclusion witnesses. v0.2.0 supports two of seven Ω-Commitment sub-trees:
//! UTXO set and block header chain.

pub mod hash;
pub mod tree;
pub mod witness;
pub mod utxo_leaf;
```

Note: `header_leaf` is added in Task 3. Don't add it yet.

- [ ] **Step 3: Update integration test imports and rename the file**

```bash
git mv crates/omega-commitment-core/tests/integration.rs crates/omega-commitment-core/tests/utxo_integration.rs
```

Edit `crates/omega-commitment-core/tests/utxo_integration.rs`. Replace the imports block:

```rust
//! End-to-end integration test for the UTXO sub-tree commitment.

use omega_commitment_core::{
    utxo_leaf::Utxo,
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::Deserialize;
```

Leave the rest of the test body unchanged.

- [ ] **Step 4: Update CLI imports**

Edit `crates/omega-commitment-cli/src/main.rs`. Change the import block:

```rust
use omega_commitment_core::{
    utxo_leaf::Utxo,
    tree::MerkleTree,
    witness::InclusionWitness,
};
```

(Other imports unchanged.)

- [ ] **Step 5: Verify**

```bash
cargo build --workspace 2>&1 | grep -E "warning|error"   # empty
cargo test --workspace 2>&1 | tail -5                      # 28 tests still pass
```

- [ ] **Step 6: Commit**

```bash
git add -A
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "refactor!: rename leaf -> utxo_leaf, integration -> utxo_integration"
```

---

## Task 2: Add `input_digest` to `CommitmentRecord` (v0.1.0 review item)

**Files:**
- Modify: `crates/omega-commitment-cli/src/main.rs`
- Modify: `crates/omega-commitment-cli/tests/cli.rs`

Add a `input_digest: Hash` field to `CommitmentRecord` so consumers (Plonky3 circuits, omega-node) can independently confirm "this commitment is for snapshot X." The digest is Blake2b-256 of the raw input file bytes.

- [ ] **Step 1: Update `CommitmentRecord` and `commit()` in `main.rs`**

Edit `crates/omega-commitment-cli/src/main.rs`. Update the import block to include `blake2b_256`:

```rust
use omega_commitment_core::{
    hash::blake2b_256,
    utxo_leaf::Utxo,
    tree::MerkleTree,
    witness::InclusionWitness,
};
```

Replace the `CommitmentRecord` struct:

```rust
#[derive(Serialize)]
struct CommitmentRecord {
    /// Blake2b-256 of the raw input file bytes. Lets consumers confirm
    /// the commitment is bound to a specific input snapshot.
    #[serde(with = "hex::serde")]
    input_digest: [u8; 32],
    #[serde(with = "hex::serde")]
    root: [u8; 32],
    leaf_count: usize,
    tree_depth: usize,
    utxo_count: usize,
}
```

In the `commit()` function, compute the digest from `raw` and populate the field. Update the body to:

```rust
fn commit(input: PathBuf, output: PathBuf) -> anyhow::Result<()> {
    let raw = fs::read_to_string(&input)?;
    let input_digest = blake2b_256(raw.as_bytes());
    let parsed: Input = serde_json::from_str(&raw)?;

    let leaves: Vec<_> = parsed.utxos.iter()
        .map(|u| u.leaf_hash())
        .collect::<Result<Vec<_>, _>>()?;

    let tree = MerkleTree::build(leaves.clone());

    fs::create_dir_all(&output)?;
    let witness_dir = output.join("witnesses");
    fs::create_dir_all(&witness_dir)?;

    let record = CommitmentRecord {
        input_digest,
        root: tree.root(),
        leaf_count: tree.leaf_count(),
        tree_depth: tree.depth(),
        utxo_count: parsed.utxos.len(),
    };
    fs::write(
        output.join("commitment.json"),
        serde_json::to_string_pretty(&record)?,
    )?;

    use std::collections::HashMap;
    let mut leaf_index: HashMap<[u8; 32], u32> = HashMap::with_capacity(tree.leaf_count());
    for (i, h) in tree.leaves().iter().enumerate() {
        leaf_index.entry(*h).or_insert(i as u32);
    }
    for leaf in leaves {
        let idx = *leaf_index.get(&leaf)
            .ok_or_else(|| anyhow::anyhow!("leaf vanished from tree"))?;
        let w = InclusionWitness::build_at_index(&tree, idx)
            .ok_or_else(|| anyhow::anyhow!("index out of range"))?;
        let fname = format!("{}.json", hex::encode(leaf));
        fs::write(
            witness_dir.join(fname),
            serde_json::to_string_pretty(&w)?,
        )?;
    }

    println!("ok: root={} input_digest={} utxos={}",
        hex::encode(record.root),
        hex::encode(record.input_digest),
        record.utxo_count);
    Ok(())
}
```

- [ ] **Step 2: Update CLI smoke test to verify the new field**

Edit `crates/omega-commitment-cli/tests/cli.rs`. Replace contents:

```rust
use std::process::Command;
use std::path::PathBuf;
use std::fs;

#[test]
fn cli_commit_smoke() {
    let exe = env!("CARGO_BIN_EXE_omega-commitment");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("omega-commitment-core/tests/fixtures/utxo_set_small.json");
    let out = tempfile::tempdir().unwrap();
    let status = Command::new(exe)
        .args([
            "commit",
            "--input", fixture.to_str().unwrap(),
            "--output", out.path().to_str().unwrap(),
        ])
        .status()
        .expect("cli runs");
    assert!(status.success());
    let commitment = out.path().join("commitment.json");
    assert!(commitment.exists());
    let body = fs::read_to_string(&commitment).unwrap();
    assert!(body.contains("\"input_digest\":"), "input_digest missing: {body}");
    assert!(body.contains("\"root\":"), "root missing: {body}");
    assert!(out.path().join("witnesses").exists());
}
```

- [ ] **Step 3: Verify**

```bash
cargo test -p omega-commitment-cli --test cli 2>&1 | tail -5   # passes
cargo test --workspace 2>&1 | tail -5                            # 28 tests pass
```

Manual check:
```bash
mkdir -p /tmp/omega-out3 && rm -rf /tmp/omega-out3/*
./target/debug/omega-commitment commit \
  --input crates/omega-commitment-core/tests/fixtures/utxo_set_small.json \
  --output /tmp/omega-out3
cat /tmp/omega-out3/commitment.json
```

Expected: `commitment.json` contains both `input_digest` (hex) and `root` (hex).

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-cli/
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(cli): add input_digest to CommitmentRecord"
```

---

## Task 3: `header_leaf.rs` — Block header canonical encoding

**Files:**
- Create: `crates/omega-commitment-core/src/header_leaf.rs`
- Modify: `crates/omega-commitment-core/src/lib.rs`

A block header leaf is the deterministic serialization of:
```
slot (u64 BE) || block_height (u64 BE) || block_hash (32) || prev_hash (32)
```
Total: 80 bytes per header. Fixed-width — no length prefixes needed. Hashed with Blake2b-256.

- [ ] **Step 1: Write `header_leaf.rs`**

Create `crates/omega-commitment-core/src/header_leaf.rs`:

```rust
//! Canonical block header leaf encoding.
//!
//! A header leaf is the deterministic serialization of:
//!   (slot: u64 BE) || (block_height: u64 BE) ||
//!   (block_hash: 32 bytes) || (prev_hash: 32 bytes)
//!
//! Total: 80 bytes. The leaf is hashed with Blake2b-256 to produce
//! the leaf hash that goes into the Merkle tree.

use crate::hash::{blake2b_256, Hash};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockHeader {
    pub slot: u64,
    pub block_height: u64,
    #[serde(with = "hex::serde")]
    pub block_hash: [u8; 32],
    #[serde(with = "hex::serde")]
    pub prev_hash: [u8; 32],
}

impl BlockHeader {
    /// Canonical 80-byte serialization.
    pub fn encode(&self) -> [u8; 80] {
        let mut out = [0u8; 80];
        out[0..8].copy_from_slice(&self.slot.to_be_bytes());
        out[8..16].copy_from_slice(&self.block_height.to_be_bytes());
        out[16..48].copy_from_slice(&self.block_hash);
        out[48..80].copy_from_slice(&self.prev_hash);
        out
    }

    /// Compute the leaf hash: Blake2b-256 of canonical encoding.
    pub fn leaf_hash(&self) -> Hash {
        blake2b_256(&self.encode())
    }
}

/// Validate that a slice of headers forms a well-linked chain ordered by
/// strictly-increasing slot, where each header's `prev_hash` matches the
/// previous header's `block_hash`. Returns the index of the first failure,
/// or None if the chain is valid. The first header is treated as genesis
/// and its `prev_hash` is not validated.
///
/// This is an optional sanity check for callers; it is NOT required for
/// commitment generation.
pub fn validate_chain_links(headers: &[BlockHeader]) -> Option<usize> {
    for i in 1..headers.len() {
        if headers[i].slot <= headers[i - 1].slot {
            return Some(i);
        }
        if headers[i].prev_hash != headers[i - 1].block_hash {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header(slot: u64, height: u64) -> BlockHeader {
        BlockHeader {
            slot,
            block_height: height,
            block_hash: [slot as u8; 32],
            prev_hash: [(slot.saturating_sub(1)) as u8; 32],
        }
    }

    #[test]
    fn encoding_is_exactly_80_bytes() {
        let h = sample_header(100, 50);
        assert_eq!(h.encode().len(), 80);
    }

    #[test]
    fn encoding_layout_is_correct() {
        let h = BlockHeader {
            slot: 0x0102030405060708,
            block_height: 0x1112131415161718,
            block_hash: [0xAAu8; 32],
            prev_hash: [0xBBu8; 32],
        };
        let e = h.encode();
        assert_eq!(&e[0..8], &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(&e[8..16], &[0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18]);
        assert_eq!(&e[16..48], &[0xAAu8; 32]);
        assert_eq!(&e[48..80], &[0xBBu8; 32]);
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let h = sample_header(7, 3);
        assert_eq!(h.leaf_hash(), h.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_slot_change() {
        let h1 = sample_header(7, 3);
        let mut h2 = h1.clone();
        h2.slot = 8;
        assert_ne!(h1.leaf_hash(), h2.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_height_change() {
        let h1 = sample_header(7, 3);
        let mut h2 = h1.clone();
        h2.block_height = 4;
        assert_ne!(h1.leaf_hash(), h2.leaf_hash());
    }

    #[test]
    fn validate_chain_links_accepts_well_linked() {
        let mut a = sample_header(1, 1);
        a.block_hash = [0x01; 32];
        let mut b = sample_header(2, 2);
        b.block_hash = [0x02; 32];
        b.prev_hash = a.block_hash;
        let mut c = sample_header(3, 3);
        c.block_hash = [0x03; 32];
        c.prev_hash = b.block_hash;
        assert_eq!(validate_chain_links(&[a, b, c]), None);
    }

    #[test]
    fn validate_chain_links_rejects_bad_prev_hash() {
        let mut a = sample_header(1, 1);
        a.block_hash = [0x01; 32];
        let mut b = sample_header(2, 2);
        b.prev_hash = [0xFF; 32]; // does not match a.block_hash
        assert_eq!(validate_chain_links(&[a, b]), Some(1));
    }

    #[test]
    fn validate_chain_links_rejects_non_monotonic_slot() {
        let mut a = sample_header(5, 1);
        a.block_hash = [0x01; 32];
        let mut b = sample_header(3, 2);
        b.prev_hash = a.block_hash;
        assert_eq!(validate_chain_links(&[a, b]), Some(1));
    }

    #[test]
    fn validate_chain_links_empty_and_single_are_valid() {
        assert_eq!(validate_chain_links(&[]), None);
        assert_eq!(validate_chain_links(&[sample_header(0, 0)]), None);
    }
}
```

- [ ] **Step 2: Update `lib.rs` to expose `header_leaf`**

Edit `crates/omega-commitment-core/src/lib.rs`. The new contents:

```rust
//! omega-commitment-core: Ω-Commitment sub-tree library.
//!
//! Provides canonical leaf encodings, a Plonky3-friendly Merkle tree, and
//! inclusion witnesses. v0.2.0 supports two of seven Ω-Commitment sub-trees:
//! UTXO set and block header chain.

pub mod hash;
pub mod tree;
pub mod witness;
pub mod utxo_leaf;
pub mod header_leaf;
```

- [ ] **Step 3: Run header tests**

```bash
cargo test -p omega-commitment-core header_leaf::tests 2>&1 | tail -15
```

Expected: 9 tests pass.

- [ ] **Step 4: Run full workspace tests**

```bash
cargo test --workspace 2>&1 | tail -5
```

Expected: 37 tests pass (28 prior + 9 new header_leaf).

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-core/src/header_leaf.rs crates/omega-commitment-core/src/lib.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(header_leaf): canonical block header encoding + chain-link validator"
```

---

## Task 4: Header chain integration test

**Files:**
- Create: `crates/omega-commitment-core/tests/fixtures/header_chain_small.json`
- Create: `crates/omega-commitment-core/tests/header_integration.rs`

Build a synthetic 8-header chain, encode each into a leaf hash, build the Merkle tree, and verify witnesses for every header. Mirrors the UTXO integration test.

- [ ] **Step 1: Write the fixture**

Create `crates/omega-commitment-core/tests/fixtures/header_chain_small.json`:

```json
{
  "headers": [
    {
      "slot": 1,
      "block_height": 1,
      "block_hash": "1100000000000000000000000000000000000000000000000000000000000000",
      "prev_hash": "0000000000000000000000000000000000000000000000000000000000000000"
    },
    {
      "slot": 2,
      "block_height": 2,
      "block_hash": "2200000000000000000000000000000000000000000000000000000000000000",
      "prev_hash": "1100000000000000000000000000000000000000000000000000000000000000"
    },
    {
      "slot": 5,
      "block_height": 3,
      "block_hash": "3300000000000000000000000000000000000000000000000000000000000000",
      "prev_hash": "2200000000000000000000000000000000000000000000000000000000000000"
    },
    {
      "slot": 6,
      "block_height": 4,
      "block_hash": "4400000000000000000000000000000000000000000000000000000000000000",
      "prev_hash": "3300000000000000000000000000000000000000000000000000000000000000"
    },
    {
      "slot": 8,
      "block_height": 5,
      "block_hash": "5500000000000000000000000000000000000000000000000000000000000000",
      "prev_hash": "4400000000000000000000000000000000000000000000000000000000000000"
    },
    {
      "slot": 11,
      "block_height": 6,
      "block_hash": "6600000000000000000000000000000000000000000000000000000000000000",
      "prev_hash": "5500000000000000000000000000000000000000000000000000000000000000"
    },
    {
      "slot": 13,
      "block_height": 7,
      "block_hash": "7700000000000000000000000000000000000000000000000000000000000000",
      "prev_hash": "6600000000000000000000000000000000000000000000000000000000000000"
    },
    {
      "slot": 14,
      "block_height": 8,
      "block_hash": "8800000000000000000000000000000000000000000000000000000000000000",
      "prev_hash": "7700000000000000000000000000000000000000000000000000000000000000"
    }
  ]
}
```

Note: 8 headers, slots are non-contiguous (some gaps), so the test exercises that the tree doesn't depend on slot continuity.

- [ ] **Step 2: Write the integration test**

Create `crates/omega-commitment-core/tests/header_integration.rs`:

```rust
//! End-to-end integration test for the block header sub-tree commitment.

use omega_commitment_core::{
    header_leaf::{validate_chain_links, BlockHeader},
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    headers: Vec<BlockHeader>,
}

const FIXTURE: &str = include_str!("fixtures/header_chain_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.headers.len(), 8);

    // Sanity check the chain is well-linked (optional helper).
    assert!(validate_chain_links(&f.headers).is_none(),
            "fixture chain is not well-linked");

    // Encode each header into a leaf hash.
    let leaves: Vec<_> = f.headers.iter().map(|h| h.leaf_hash()).collect();

    // Build the tree.
    let tree = MerkleTree::build(leaves.clone());
    assert_eq!(tree.leaf_count(), 8); // already a power of two
    assert_eq!(tree.depth(), 3);
    let root = tree.root();
    assert_ne!(root, [0u8; 32]);

    // Witness for every header verifies against the root.
    for leaf in leaves {
        let w = InclusionWitness::build(&tree, leaf)
            .expect("leaf is in tree");
        assert!(w.verify(root), "witness verification failed");
    }
}

#[test]
fn root_is_stable_across_runs() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let leaves1: Vec<_> = f.headers.iter().map(|h| h.leaf_hash()).collect();
    let leaves2: Vec<_> = f.headers.iter().map(|h| h.leaf_hash()).collect();
    assert_eq!(MerkleTree::build(leaves1).root(),
               MerkleTree::build(leaves2).root());
}

#[test]
fn header_witness_independent_of_chain_validity() {
    // Even if the chain is malformed, the commitment is still well-defined
    // (commitment generation does NOT depend on chain validity).
    let mut headers: Vec<BlockHeader> = serde_json::from_str::<Fixture>(FIXTURE)
        .unwrap().headers;
    // Tamper with the last header's prev_hash.
    headers.last_mut().unwrap().prev_hash = [0xFFu8; 32];
    assert!(validate_chain_links(&headers).is_some(), "tamper detected");

    let leaves: Vec<_> = headers.iter().map(|h| h.leaf_hash()).collect();
    let tree = MerkleTree::build(leaves.clone());
    for leaf in leaves {
        let w = InclusionWitness::build(&tree, leaf).unwrap();
        assert!(w.verify(tree.root()));
    }
}
```

- [ ] **Step 3: Run the integration tests**

```bash
cargo test -p omega-commitment-core --test header_integration 2>&1 | tail -10
```

Expected: 3 tests pass.

- [ ] **Step 4: Run full workspace tests**

```bash
cargo test --workspace 2>&1 | tail -5
```

Expected: 40 tests pass (37 prior + 3 new header_integration).

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-core/tests/header_integration.rs \
        crates/omega-commitment-core/tests/fixtures/header_chain_small.json
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test: header sub-tree integration test against synthetic 8-block chain"
```

---

## Task 5: CLI sub-tree dispatcher

**Files:**
- Modify: `crates/omega-commitment-cli/src/main.rs`

Add a `--sub-tree {utxo,header}` flag to `commit`. Defaults to `utxo` for backwards compat with v0.1.0 callers. Internally the subcommand dispatches to either the existing UTXO path or a new header path.

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
#[serde(rename_all = "lowercase")]
enum SubTree {
    Utxo,
    Header,
}

#[derive(Deserialize)]
struct UtxoInput {
    utxos: Vec<Utxo>,
}

#[derive(Deserialize)]
struct HeaderInput {
    headers: Vec<BlockHeader>,
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

Note: The `CommitmentRecord` field `utxo_count` is renamed to `item_count` to be sub-tree agnostic. The `sub_tree` field is also added so consumers can verify which sub-tree the commitment is for.

- [ ] **Step 2: Build and run end-to-end (UTXO path)**

```bash
cargo build --release -p omega-commitment-cli
mkdir -p /tmp/omega-out-u && rm -rf /tmp/omega-out-u/*
./target/release/omega-commitment commit \
  --sub-tree utxo \
  --input crates/omega-commitment-core/tests/fixtures/utxo_set_small.json \
  --output /tmp/omega-out-u
cat /tmp/omega-out-u/commitment.json
```

Expected: stdout shows `sub_tree=Utxo`, `commitment.json` contains `"sub_tree": "utxo"` and the same root/digest fields.

- [ ] **Step 3: Build and run end-to-end (header path)**

```bash
mkdir -p /tmp/omega-out-h && rm -rf /tmp/omega-out-h/*
./target/release/omega-commitment commit \
  --sub-tree header \
  --input crates/omega-commitment-core/tests/fixtures/header_chain_small.json \
  --output /tmp/omega-out-h
cat /tmp/omega-out-h/commitment.json
ls /tmp/omega-out-h/witnesses/ | wc -l
```

Expected: stdout shows `sub_tree=Header`, `commitment.json` contains `"sub_tree": "header"`, 8 witness files exist.

- [ ] **Step 4: Default-flag backwards compat**

```bash
mkdir -p /tmp/omega-out-d && rm -rf /tmp/omega-out-d/*
./target/release/omega-commitment commit \
  --input crates/omega-commitment-core/tests/fixtures/utxo_set_small.json \
  --output /tmp/omega-out-d
cat /tmp/omega-out-d/commitment.json
```

Expected: defaults to `sub_tree=Utxo` (backwards compatible with v0.1.0 callers).

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-cli/src/main.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(cli): --sub-tree dispatcher for utxo and header sub-trees"
```

---

## Task 6: CLI smoke tests for both sub-trees

**Files:**
- Modify: `crates/omega-commitment-cli/tests/cli.rs`

Replace the single UTXO smoke test with two: one for each sub-tree path.

- [ ] **Step 1: Replace test file**

Create / replace `crates/omega-commitment-cli/tests/cli.rs`:

```rust
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("omega-commitment-core/tests/fixtures")
        .join(name)
}

fn run_commit(sub_tree: &str, fixture: &str) -> tempfile::TempDir {
    let exe = env!("CARGO_BIN_EXE_omega-commitment");
    let out = tempfile::tempdir().unwrap();
    let status = Command::new(exe)
        .args([
            "commit",
            "--sub-tree", sub_tree,
            "--input", fixture_path(fixture).to_str().unwrap(),
            "--output", out.path().to_str().unwrap(),
        ])
        .status()
        .expect("cli runs");
    assert!(status.success(), "sub-tree {sub_tree} failed");
    out
}

#[test]
fn cli_commit_utxo_smoke() {
    let out = run_commit("utxo", "utxo_set_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(body.contains("\"sub_tree\": \"utxo\""), "wrong sub_tree tag: {body}");
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 3"));
    assert!(out.path().join("witnesses").exists());
}

#[test]
fn cli_commit_header_smoke() {
    let out = run_commit("header", "header_chain_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(body.contains("\"sub_tree\": \"header\""), "wrong sub_tree tag: {body}");
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 8"));
    let witness_count = fs::read_dir(out.path().join("witnesses"))
        .unwrap().count();
    assert_eq!(witness_count, 8, "expected 8 header witness files");
}

#[test]
fn cli_commit_default_is_utxo() {
    let exe = env!("CARGO_BIN_EXE_omega-commitment");
    let out = tempfile::tempdir().unwrap();
    let status = Command::new(exe)
        .args([
            "commit",
            "--input", fixture_path("utxo_set_small.json").to_str().unwrap(),
            "--output", out.path().to_str().unwrap(),
        ])
        .status()
        .expect("cli runs");
    assert!(status.success());
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(body.contains("\"sub_tree\": \"utxo\""),
            "default sub_tree should be utxo: {body}");
}
```

- [ ] **Step 2: Run the smoke tests**

```bash
cargo test -p omega-commitment-cli --test cli 2>&1 | tail -10
```

Expected: 3 tests pass.

- [ ] **Step 3: Run full workspace tests**

```bash
cargo test --workspace 2>&1 | tail -5
```

Expected: 42 tests pass (40 prior + 2 net-new CLI tests; the v0.1.0 single CLI test was replaced by 3, so net delta is +2).

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-cli/tests/cli.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test(cli): smoke tests for utxo, header, and default-flag paths"
```

---

## Task 7: Bump version to 0.2.0 + extend README

**Files:**
- Modify: `Cargo.toml` (workspace package version — there isn't one currently; per-crate bump instead)
- Modify: `crates/omega-commitment-core/Cargo.toml`
- Modify: `crates/omega-commitment-cli/Cargo.toml`
- Modify: `README.md`

- [ ] **Step 1: Bump core crate version**

Edit `crates/omega-commitment-core/Cargo.toml`. Change:

```toml
version = "0.1.0"
```

to:

```toml
version = "0.2.0"
```

- [ ] **Step 2: Bump CLI crate version**

Edit `crates/omega-commitment-cli/Cargo.toml`. Change:

```toml
version = "0.1.0"
```

to:

```toml
version = "0.2.0"
```

- [ ] **Step 3: Verify build**

```bash
cargo build --workspace 2>&1 | grep -E "warning|error"   # empty
cargo test --workspace 2>&1 | tail -5                      # 42 tests pass
```

- [ ] **Step 4: Update README.md**

Append to `/home/hoskinson/omega-commitment/README.md`:

```markdown
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
```

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/omega-commitment-core/Cargo.toml crates/omega-commitment-cli/Cargo.toml README.md
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "chore: bump to 0.2.0; document header sub-tree"
```

Note: `Cargo.toml` (workspace) is NOT modified for version because the workspace doesn't define `version` at the workspace level — only individual crates carry versions.

If `git diff --cached` shows zero changes for the workspace `Cargo.toml`, drop it from the `git add` list and proceed; the commit is still valid with just the per-crate bumps and README.

---

## Self-review

**Spec coverage** (from `2026-05-01-ouroboros-omega-design.md` §7):
- "Block header accumulator — FRI-friendly accumulator over all old-chain block headers from genesis to H. Lets users prove 'block at slot S had hash B.'"
  - ✅ Block header canonical encoding includes slot AND block_hash → users can prove slot↔hash binding.
  - ✅ Plonky3-friendly tree (binary, fixed-arity, sorted-padded) is reused — FRI verification is straightforward.
  - ✅ Witness format unchanged from sub-tree 1; same Plonky3 circuit shape will work for both.

**v0.1.0 final-review carryovers:**
- ✅ Rename `leaf.rs` → `utxo_leaf.rs` (Task 1).
- ✅ Add `input_digest` to `CommitmentRecord` (Task 2).
- ✅ Sub-tree dispatcher in CLI (Task 5).
- ❌ NOT addressed: the dual-track shadow hash decision. This is intentionally deferred per the user's direction; documented in v0.1.0 README as an open question.
- ❌ NOT addressed: `Cargo.lock` commit policy (since the workspace ships a binary). Spec defers; revisit later.

**Placeholder scan:** All code blocks contain complete, runnable code. No `// TODO`, no "fill in details", no "similar to Task N" — every step is self-contained. ✅

**Type consistency:**
- `Hash = [u8; 32]` used uniformly everywhere.
- `BlockHeader { slot: u64, block_height: u64, block_hash: [u8;32], prev_hash: [u8;32] }` matches in `header_leaf.rs`, the JSON fixture, the CLI `HeaderInput`, and the integration test.
- `SubTree` enum values are `Utxo` and `Header` everywhere; serde renames to lowercase, matching the JSON output check in the smoke test.
- `CommitmentRecord` field names: `sub_tree`, `input_digest`, `root`, `leaf_count`, `tree_depth`, `item_count`. Smoke tests match.
- `CommitmentRecord` field rename `utxo_count` → `item_count` is consistently propagated across CLI main.rs and smoke tests.
- ✅ No drift identified.

**Bite-sized tasks:** Each task has 4–8 numbered steps; each step is a single action (write code / run command / commit). ✅

**No hidden dependencies:** Tasks build linearly. Task 1 enables 3 (header_leaf needs lib.rs ready for new module); Task 4 needs 3 (uses `BlockHeader`); Task 5 needs 3 + 4 (uses both header types and the fixture); Task 6 needs 5 (CLI dispatcher). Task 7 wraps. ✅

---

## What's NOT in this plan (and why)

- **Real Cardano mainnet header ingestion.** Like sub-tree 1, this plan uses a synthetic fixture. A `cardano-multiplatform-lib` integration to read real headers is deferred to a follow-on plan.
- **Issuer VRF / body hash / protocol version fields.** v0.2.0 keeps the leaf minimal: slot, block_height, block_hash, prev_hash. Adding richer header fields is a v0.3.0 schema-extension question that needs a separate spec decision.
- **Plonky3 circuit consuming the witness.** Track T2, separate plan.
- **Dual-track shadow hash.** Still deferred per the v0.1.0 open question.
- **The remaining five sub-trees** (tx index, token policies, script registry, stake state, governance state). Each gets its own plan.

---

## How to execute this plan

Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Seven tasks, each independently committable. Total runway estimate for an experienced Rust dev: **1–2 weeks** (smaller than v0.1.0 because the tree/witness machinery is reused).

After execution, expected state:
- 16 commits added on top of v0.1.0 (currently at 16 commits)
- ~28 tests added (3 new for input_digest, 9 new in header_leaf, 3 new in header_integration, 2 net-new in CLI smoke = ~17 net new tests; final total ~42–45 tests)
- Both crates at version 0.2.0
- README documents both sub-trees and the next-plan handoff slot for sub-tree 3

Next plan in this track: `2026-XX-XX-omega-tx-index-plan.md` (sub-tree 3 of 7).
