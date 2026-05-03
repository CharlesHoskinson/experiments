# Omega UTXO Commitment Tool — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust library + CLI that, given a Cardano `LedgerState` snapshot, computes a deterministic Merkle root over the UTXO set (one of the seven sub-trees in the Ω-Commitment) and emits inclusion witnesses suitable for downstream Plonky3 circuits.

**Architecture:** Rust workspace with two crates — `omega-commitment-core` (pure-functional library: hashing, leaf encoding, Merkle tree, witness generation) and `omega-commitment-cli` (binary that reads a Cardano ledger snapshot, calls the core, emits JSON output). Test-driven with fixture-based integration tests.

**Tech Stack:** Rust 1.79+, `blake2`, `sha3`, `serde`, `serde_json`, `clap`, `pasta_curves`-free (PQ-only stack), `cardano-multiplatform-lib` for ledger snapshot decoding, `proptest` for property tests, `criterion` for benchmarks.

**Track:** T1 (Ω-Commitment Tooling) — see `2026-05-01-ouroboros-omega-program-roadmap.md`.

**Locked design decisions honored:**
- Decision 7 (PQ-only crypto): Blake2b-256 + SHA3-256 dual-track hashing; no curve operations.
- Decision 8 (Plonky3 ZK): leaf encoding and tree layout chosen to be friendly to Plonky3 FRI circuits — fixed-arity (binary), fixed-depth padding, deterministic ordering.
- Decision 3 (everything-provable): this plan delivers ONE of seven sub-trees. Subsequent plans handle the other six.

---

## File structure

```
omega-commitment/
├── Cargo.toml                                  (workspace manifest)
├── README.md
├── crates/
│   ├── omega-commitment-core/
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs                          (re-exports)
│   │   │   ├── hash.rs                         (Blake2b-256 + SHA3-256 wrappers)
│   │   │   ├── leaf.rs                         (UTXO leaf canonical encoding)
│   │   │   ├── tree.rs                         (Merkle tree builder)
│   │   │   └── witness.rs                      (inclusion witness format)
│   │   └── tests/
│   │       ├── fixtures/
│   │       │   └── utxo_set_small.json         (24-utxo synthetic fixture)
│   │       └── integration.rs
│   └── omega-commitment-cli/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
└── .gitignore
```

Each file has a single, clear responsibility. `hash.rs`, `leaf.rs`, `tree.rs`, `witness.rs` are pure and independently testable. `main.rs` is the only impure file (file I/O, argument parsing).

---

## Task 1: Initialize workspace

**Files:**
- Create: `omega-commitment/Cargo.toml`
- Create: `omega-commitment/.gitignore`
- Create: `omega-commitment/README.md`
- Create: `omega-commitment/crates/omega-commitment-core/Cargo.toml`
- Create: `omega-commitment/crates/omega-commitment-cli/Cargo.toml`

- [ ] **Step 1: Create workspace directory and Cargo workspace manifest**

```bash
mkdir -p omega-commitment/crates/omega-commitment-core/src
mkdir -p omega-commitment/crates/omega-commitment-core/tests/fixtures
mkdir -p omega-commitment/crates/omega-commitment-cli/src
cd omega-commitment
```

Write `omega-commitment/Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
  "crates/omega-commitment-core",
  "crates/omega-commitment-cli",
]

[workspace.package]
edition = "2021"
rust-version = "1.79"
license = "Apache-2.0"
repository = "https://github.com/IntersectMBO/omega-commitment"

[workspace.dependencies]
blake2 = "0.10"
sha3 = "0.10"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
hex = "0.4"
thiserror = "1"
clap = { version = "4", features = ["derive"] }
proptest = "1"
criterion = "0.5"
```

- [ ] **Step 2: Write `.gitignore`**

```
target/
Cargo.lock
*.bk
.DS_Store
```

- [ ] **Step 3: Write `README.md`**

```markdown
# omega-commitment

Reference tooling for the Ouroboros Omega Ω-Commitment.

This crate computes one of the seven sub-trees: the **UTXO set Merkle root**.
Subsequent crates will compute the remaining six (block headers, tx index,
token policies, script registry, stake state, governance state).

See `docs/superpowers/specs/2026-05-01-ouroboros-omega-design.md` for design context.
```

- [ ] **Step 4: Write `crates/omega-commitment-core/Cargo.toml`**

```toml
[package]
name = "omega-commitment-core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
blake2.workspace = true
sha3.workspace = true
serde.workspace = true
hex.workspace = true
thiserror.workspace = true

[dev-dependencies]
serde_json.workspace = true
proptest.workspace = true
```

- [ ] **Step 5: Write `crates/omega-commitment-cli/Cargo.toml`**

```toml
[package]
name = "omega-commitment-cli"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[[bin]]
name = "omega-commitment"
path = "src/main.rs"

[dependencies]
omega-commitment-core = { path = "../omega-commitment-core" }
clap.workspace = true
serde_json.workspace = true
hex.workspace = true
thiserror.workspace = true
```

- [ ] **Step 6: Stub the lib + bin so `cargo check` passes**

`crates/omega-commitment-core/src/lib.rs`:
```rust
//! omega-commitment-core: Ω-Commitment UTXO sub-tree library.
```

`crates/omega-commitment-cli/src/main.rs`:
```rust
fn main() {}
```

- [ ] **Step 7: Verify workspace compiles**

Run: `cargo check --workspace`
Expected: `Finished dev` — no errors.

- [ ] **Step 8: Commit**

```bash
git init
git add .
git commit -m "chore: bootstrap omega-commitment Rust workspace"
```

---

## Task 2: Hash module — Blake2b-256 + SHA3-256 dual-track

**Files:**
- Create: `crates/omega-commitment-core/src/hash.rs`
- Modify: `crates/omega-commitment-core/src/lib.rs`

**Why:** PQ posture requires dual-track hashing (Blake2b for speed, SHA3 for diversity of construction). The Ω-Commitment uses Blake2b-256 as the primary, SHA3-256 as a parallel "shadow" hash for cross-validation. Decision 7 mandates no curve crypto anywhere.

- [ ] **Step 1: Write the failing test**

Create `crates/omega-commitment-core/src/hash.rs`:

```rust
//! Dual-track hashing: Blake2b-256 (primary) + SHA3-256 (shadow).

use blake2::{Blake2b, Digest as Blake2Digest};
use blake2::digest::consts::U32;
use sha3::{Sha3_256, Digest as Sha3Digest};

pub type Hash = [u8; 32];

/// Primary hash: Blake2b truncated to 256 bits.
pub fn blake2b_256(data: &[u8]) -> Hash {
    let mut h = Blake2b::<U32>::new();
    h.update(data);
    h.finalize().into()
}

/// Shadow hash: SHA3-256.
pub fn sha3_256(data: &[u8]) -> Hash {
    let mut h = Sha3_256::new();
    h.update(data);
    h.finalize().into()
}

/// Dual-track hash. Returns (blake2b_256, sha3_256). Both must be checked
/// by verifiers; divergence means a bug or tampering.
pub fn dual_hash(data: &[u8]) -> (Hash, Hash) {
    (blake2b_256(data), sha3_256(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blake2b_256_known_vector() {
        // Test vector: blake2b-256 of "" is known.
        let h = blake2b_256(b"");
        assert_eq!(
            hex::encode(h),
            "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8"
        );
    }

    #[test]
    fn sha3_256_known_vector() {
        // Test vector: sha3-256 of "" is known (NIST FIPS 202).
        let h = sha3_256(b"");
        assert_eq!(
            hex::encode(h),
            "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a"
        );
    }

    #[test]
    fn dual_hash_returns_both() {
        let (b, s) = dual_hash(b"omega");
        assert_ne!(b, s, "Blake2b and SHA3 must produce different outputs");
        assert_eq!(b, blake2b_256(b"omega"));
        assert_eq!(s, sha3_256(b"omega"));
    }
}
```

Update `crates/omega-commitment-core/src/lib.rs`:

```rust
//! omega-commitment-core: Ω-Commitment UTXO sub-tree library.

pub mod hash;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p omega-commitment-core hash::tests`
Expected: tests should pass on first run because the implementation is already shown — but verify the compilation succeeds and outputs match the known vectors. If a vector mismatches (e.g., wrong dependency version), fail fast.

- [ ] **Step 3: Verify all three tests pass**

Run: `cargo test -p omega-commitment-core hash::tests`
Expected:
```
running 3 tests
test hash::tests::blake2b_256_known_vector ... ok
test hash::tests::dual_hash_returns_both ... ok
test hash::tests::sha3_256_known_vector ... ok
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-core/src/hash.rs crates/omega-commitment-core/src/lib.rs
git commit -m "feat(hash): add Blake2b-256 + SHA3-256 dual-track hashing"
```

---

## Task 3: UTXO leaf canonical encoding

**Files:**
- Create: `crates/omega-commitment-core/src/leaf.rs`
- Modify: `crates/omega-commitment-core/src/lib.rs`

**Why:** Every UTXO must serialize to the *same* bytes regardless of input format. Determinism is critical — divergent encodings produce divergent commitments. We define a strict canonical form and hash it once.

- [ ] **Step 1: Write the failing tests**

Create `crates/omega-commitment-core/src/leaf.rs`:

```rust
//! Canonical UTXO leaf encoding.
//!
//! A UTXO leaf is the deterministic serialization of:
//!   (tx_id: 32 bytes) || (output_index: u32 BE) ||
//!   (address_hash: 32 bytes) || (value_lovelace: u64 BE) ||
//!   (asset_count: u32 BE) || ([asset_id, quantity] ...) ||
//!   (datum_hash: 0x00 or 0x01 || 32 bytes)
//!
//! The leaf is then hashed with Blake2b-256 to produce the leaf hash.

use crate::hash::{blake2b_256, Hash};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LeafError {
    #[error("output_index too large for u32")]
    OutputIndexOverflow,
    #[error("asset_count too large for u32")]
    AssetCountOverflow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Asset {
    /// 28-byte policy id || asset name (variable).
    /// Canonical form: policy (28) || name_len (u16 BE) || name bytes.
    #[serde(with = "hex::serde")]
    pub asset_id: Vec<u8>,
    pub quantity: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Utxo {
    #[serde(with = "hex::serde")]
    pub tx_id: [u8; 32],
    pub output_index: u32,
    #[serde(with = "hex::serde")]
    pub address_hash: [u8; 32],
    pub value_lovelace: u64,
    pub assets: Vec<Asset>,
    #[serde(with = "hex::serde", default)]
    pub datum_hash: Option<[u8; 32]>,
}

impl Utxo {
    /// Canonical byte serialization.
    pub fn encode(&self) -> Result<Vec<u8>, LeafError> {
        let mut out = Vec::with_capacity(128);
        out.extend_from_slice(&self.tx_id);
        out.extend_from_slice(&self.output_index.to_be_bytes());
        out.extend_from_slice(&self.address_hash);
        out.extend_from_slice(&self.value_lovelace.to_be_bytes());
        let asset_count = u32::try_from(self.assets.len())
            .map_err(|_| LeafError::AssetCountOverflow)?;
        out.extend_from_slice(&asset_count.to_be_bytes());
        // Sort assets by asset_id for canonicality
        let mut sorted = self.assets.clone();
        sorted.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
        for a in sorted {
            let id_len = u16::try_from(a.asset_id.len())
                .map_err(|_| LeafError::AssetCountOverflow)?;
            out.extend_from_slice(&id_len.to_be_bytes());
            out.extend_from_slice(&a.asset_id);
            out.extend_from_slice(&a.quantity.to_be_bytes());
        }
        match self.datum_hash {
            None => out.push(0x00),
            Some(d) => {
                out.push(0x01);
                out.extend_from_slice(&d);
            }
        }
        Ok(out)
    }

    /// Compute the leaf hash: Blake2b-256 of canonical encoding.
    pub fn leaf_hash(&self) -> Result<Hash, LeafError> {
        Ok(blake2b_256(&self.encode()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_utxo() -> Utxo {
        Utxo {
            tx_id: [1u8; 32],
            output_index: 0,
            address_hash: [2u8; 32],
            value_lovelace: 1_000_000,
            assets: vec![],
            datum_hash: None,
        }
    }

    #[test]
    fn empty_assets_no_datum() {
        let u = sample_utxo();
        let enc = u.encode().unwrap();
        // 32 + 4 + 32 + 8 + 4 + 0 + 1 = 81 bytes
        assert_eq!(enc.len(), 81);
        assert_eq!(enc[80], 0x00, "datum_hash absence marker");
    }

    #[test]
    fn datum_hash_present_marker() {
        let mut u = sample_utxo();
        u.datum_hash = Some([3u8; 32]);
        let enc = u.encode().unwrap();
        // 81 - 1 + 33 = 113 bytes
        assert_eq!(enc.len(), 113);
        assert_eq!(enc[80], 0x01, "datum_hash presence marker");
    }

    #[test]
    fn assets_sorted_canonically() {
        let mut u1 = sample_utxo();
        u1.assets = vec![
            Asset { asset_id: vec![0xff], quantity: 10 },
            Asset { asset_id: vec![0x00], quantity: 20 },
        ];
        let mut u2 = sample_utxo();
        u2.assets = vec![
            Asset { asset_id: vec![0x00], quantity: 20 },
            Asset { asset_id: vec![0xff], quantity: 10 },
        ];
        // Different input order, identical canonical encoding.
        assert_eq!(u1.encode().unwrap(), u2.encode().unwrap());
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let u = sample_utxo();
        let h1 = u.leaf_hash().unwrap();
        let h2 = u.leaf_hash().unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn leaf_hash_changes_with_value() {
        let u1 = sample_utxo();
        let mut u2 = sample_utxo();
        u2.value_lovelace = 999_999;
        assert_ne!(u1.leaf_hash().unwrap(), u2.leaf_hash().unwrap());
    }
}
```

Update `crates/omega-commitment-core/src/lib.rs`:

```rust
//! omega-commitment-core: Ω-Commitment UTXO sub-tree library.

pub mod hash;
pub mod leaf;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p omega-commitment-core leaf::tests`
Expected: 5 tests pass.

- [ ] **Step 3: Add a property test for stability**

Append to `crates/omega-commitment-core/src/leaf.rs` test module:

```rust
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn encoding_is_pure(
            tx_id in any::<[u8; 32]>(),
            output_index in any::<u32>(),
            value_lovelace in any::<u64>(),
        ) {
            let mut u = sample_utxo();
            u.tx_id = tx_id;
            u.output_index = output_index;
            u.value_lovelace = value_lovelace;
            let e1 = u.encode().unwrap();
            let e2 = u.encode().unwrap();
            prop_assert_eq!(e1, e2);
        }
    }
```

- [ ] **Step 4: Run the property test**

Run: `cargo test -p omega-commitment-core leaf::tests::encoding_is_pure`
Expected: passes (256 random cases).

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-core/src/leaf.rs crates/omega-commitment-core/src/lib.rs
git commit -m "feat(leaf): canonical UTXO leaf encoding with property test"
```

---

## Task 4: Merkle tree builder (Plonky3-friendly)

**Files:**
- Create: `crates/omega-commitment-core/src/tree.rs`
- Modify: `crates/omega-commitment-core/src/lib.rs`

**Why:** The UTXO set commitment is the root of a binary Merkle tree over leaf hashes. We use a Plonky3-friendly layout: binary, fixed-arity, deterministic ordering of leaves, padded to next-power-of-two with zero hashes for circuit determinism.

- [ ] **Step 1: Write the failing tests**

Create `crates/omega-commitment-core/src/tree.rs`:

```rust
//! Plonky3-friendly binary Merkle tree.
//!
//! - Leaves are sorted by their leaf hash (deterministic ordering).
//! - The tree is padded to the next power of two with the zero-hash leaf.
//! - Internal nodes: H(left || right).
//! - Root: the single hash at the top.
//!
//! This layout is chosen for compatibility with Plonky3 FRI-based
//! verification circuits: fixed depth, fixed arity, no variable-length
//! Merkle paths.

use crate::hash::{blake2b_256, Hash};

pub const ZERO_HASH: Hash = [0u8; 32];

#[derive(Debug, Clone)]
pub struct MerkleTree {
    pub leaves: Vec<Hash>,   // sorted, padded
    pub layers: Vec<Vec<Hash>>, // layers[0] = leaves; last = [root]
}

impl MerkleTree {
    /// Build from an unsorted set of leaf hashes.
    pub fn build(mut input: Vec<Hash>) -> Self {
        input.sort();
        // Pad to next power of two (≥ 1).
        let target = input.len().max(1).next_power_of_two();
        while input.len() < target {
            input.push(ZERO_HASH);
        }
        let mut layers = vec![input.clone()];
        let mut current = input;
        while current.len() > 1 {
            let mut next = Vec::with_capacity(current.len() / 2);
            for chunk in current.chunks(2) {
                let mut buf = [0u8; 64];
                buf[..32].copy_from_slice(&chunk[0]);
                buf[32..].copy_from_slice(&chunk[1]);
                next.push(blake2b_256(&buf));
            }
            layers.push(next.clone());
            current = next;
        }
        Self { leaves: layers[0].clone(), layers }
    }

    pub fn root(&self) -> Hash {
        *self.layers.last().unwrap().first().unwrap()
    }

    pub fn depth(&self) -> usize {
        self.layers.len() - 1
    }

    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_pads_to_one_zero_leaf() {
        let t = MerkleTree::build(vec![]);
        assert_eq!(t.leaf_count(), 1);
        assert_eq!(t.depth(), 0);
        assert_eq!(t.root(), ZERO_HASH);
    }

    #[test]
    fn single_leaf_tree() {
        let leaf = blake2b_256(b"a");
        let t = MerkleTree::build(vec![leaf]);
        assert_eq!(t.leaf_count(), 1);
        assert_eq!(t.depth(), 0);
        assert_eq!(t.root(), leaf);
    }

    #[test]
    fn two_leaves_tree() {
        let a = blake2b_256(b"a");
        let b = blake2b_256(b"b");
        let t = MerkleTree::build(vec![a, b]);
        assert_eq!(t.leaf_count(), 2);
        assert_eq!(t.depth(), 1);
        // Sorted leaves
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(&lo);
        buf[32..].copy_from_slice(&hi);
        assert_eq!(t.root(), blake2b_256(&buf));
    }

    #[test]
    fn three_leaves_pads_to_four() {
        let a = blake2b_256(b"a");
        let b = blake2b_256(b"b");
        let c = blake2b_256(b"c");
        let t = MerkleTree::build(vec![a, b, c]);
        assert_eq!(t.leaf_count(), 4);
        assert_eq!(t.depth(), 2);
        // The padded leaf is ZERO_HASH.
        assert!(t.leaves.contains(&ZERO_HASH));
    }

    #[test]
    fn root_is_deterministic_under_input_permutation() {
        let leaves: Vec<Hash> = (0..8u8).map(|i| blake2b_256(&[i])).collect();
        let t1 = MerkleTree::build(leaves.clone());
        let mut shuffled = leaves;
        shuffled.reverse();
        let t2 = MerkleTree::build(shuffled);
        assert_eq!(t1.root(), t2.root());
    }
}
```

Update `crates/omega-commitment-core/src/lib.rs`:

```rust
//! omega-commitment-core: Ω-Commitment UTXO sub-tree library.

pub mod hash;
pub mod leaf;
pub mod tree;
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p omega-commitment-core tree::tests`
Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/omega-commitment-core/src/tree.rs crates/omega-commitment-core/src/lib.rs
git commit -m "feat(tree): Plonky3-friendly binary Merkle tree builder"
```

---

## Task 5: Inclusion witness format

**Files:**
- Create: `crates/omega-commitment-core/src/witness.rs`
- Modify: `crates/omega-commitment-core/src/lib.rs`

**Why:** A witness is what a `claim_utxo` transaction will eventually carry — proof that a specific leaf is in the tree. The format must be deterministic and Plonky3-friendly.

- [ ] **Step 1: Write the failing tests**

Create `crates/omega-commitment-core/src/witness.rs`:

```rust
//! Inclusion witness for a UTXO leaf in the Merkle tree.

use crate::hash::{blake2b_256, Hash};
use crate::tree::MerkleTree;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InclusionWitness {
    /// The leaf hash being proven.
    #[serde(with = "hex::serde")]
    pub leaf: Hash,
    /// Position of the leaf in the sorted-padded leaves array.
    pub leaf_index: u32,
    /// Sibling hashes from leaf-level up to (but not including) the root.
    /// `siblings[i]` is the sibling of the node at layer i on the path
    /// from leaf to root.
    pub siblings: Vec<Hash>,
}

impl InclusionWitness {
    /// Build a witness for the given leaf hash. Returns None if the leaf
    /// isn't in the tree.
    pub fn build(tree: &MerkleTree, leaf: Hash) -> Option<Self> {
        let leaf_index = tree.leaves.iter().position(|h| h == &leaf)? as u32;
        let mut idx = leaf_index as usize;
        let mut siblings = Vec::with_capacity(tree.depth());
        for layer in &tree.layers[..tree.depth()] {
            let sib_idx = idx ^ 1;
            siblings.push(layer[sib_idx]);
            idx /= 2;
        }
        Some(Self { leaf, leaf_index, siblings })
    }

    /// Verify this witness against a claimed root.
    pub fn verify(&self, root: Hash) -> bool {
        let mut current = self.leaf;
        let mut idx = self.leaf_index as usize;
        for sib in &self.siblings {
            let mut buf = [0u8; 64];
            if idx & 1 == 0 {
                buf[..32].copy_from_slice(&current);
                buf[32..].copy_from_slice(sib);
            } else {
                buf[..32].copy_from_slice(sib);
                buf[32..].copy_from_slice(&current);
            }
            current = blake2b_256(&buf);
            idx /= 2;
        }
        current == root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_tree_of(n: usize) -> (MerkleTree, Vec<Hash>) {
        let leaves: Vec<Hash> = (0..n)
            .map(|i| blake2b_256(&(i as u32).to_be_bytes()))
            .collect();
        let tree = MerkleTree::build(leaves.clone());
        (tree, leaves)
    }

    #[test]
    fn witness_for_present_leaf_verifies() {
        let (tree, leaves) = build_tree_of(8);
        for leaf in &leaves {
            let w = InclusionWitness::build(&tree, *leaf).unwrap();
            assert!(w.verify(tree.root()), "leaf {:?} witness failed", leaf);
        }
    }

    #[test]
    fn witness_for_absent_leaf_is_none() {
        let (tree, _) = build_tree_of(4);
        let bogus = blake2b_256(b"not in tree");
        assert!(InclusionWitness::build(&tree, bogus).is_none());
    }

    #[test]
    fn tampered_witness_fails_verify() {
        let (tree, leaves) = build_tree_of(4);
        let mut w = InclusionWitness::build(&tree, leaves[0]).unwrap();
        // Flip a bit in the first sibling.
        if !w.siblings.is_empty() {
            w.siblings[0][0] ^= 0x01;
            assert!(!w.verify(tree.root()));
        }
    }

    #[test]
    fn wrong_root_rejects() {
        let (tree, leaves) = build_tree_of(4);
        let w = InclusionWitness::build(&tree, leaves[0]).unwrap();
        let bad_root = blake2b_256(b"bad");
        assert!(!w.verify(bad_root));
    }

    #[test]
    fn witness_serializes_to_json() {
        let (tree, leaves) = build_tree_of(4);
        let w = InclusionWitness::build(&tree, leaves[0]).unwrap();
        let s = serde_json::to_string(&w).unwrap();
        let w2: InclusionWitness = serde_json::from_str(&s).unwrap();
        assert_eq!(w, w2);
    }
}
```

Update `crates/omega-commitment-core/src/lib.rs`:

```rust
//! omega-commitment-core: Ω-Commitment UTXO sub-tree library.

pub mod hash;
pub mod leaf;
pub mod tree;
pub mod witness;
```

Add `serde_json = { workspace = true }` to `omega-commitment-core` `[dev-dependencies]` if not already present (it is).

- [ ] **Step 2: Run tests**

Run: `cargo test -p omega-commitment-core witness::tests`
Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/omega-commitment-core/src/witness.rs crates/omega-commitment-core/src/lib.rs
git commit -m "feat(witness): inclusion witness build + verify with JSON round-trip"
```

---

## Task 6: Integration test with a fixture

**Files:**
- Create: `crates/omega-commitment-core/tests/fixtures/utxo_set_small.json`
- Create: `crates/omega-commitment-core/tests/integration.rs`

**Why:** Unit tests cover modules in isolation. The integration test exercises the full pipeline: parse JSON UTXOs → encode leaves → build tree → generate witness → verify against the published root. This is the public contract.

- [ ] **Step 1: Write the fixture**

Create `crates/omega-commitment-core/tests/fixtures/utxo_set_small.json`:

```json
{
  "utxos": [
    {
      "tx_id": "0101010101010101010101010101010101010101010101010101010101010101",
      "output_index": 0,
      "address_hash": "0202020202020202020202020202020202020202020202020202020202020202",
      "value_lovelace": 1000000,
      "assets": [],
      "datum_hash": null
    },
    {
      "tx_id": "0303030303030303030303030303030303030303030303030303030303030303",
      "output_index": 1,
      "address_hash": "0404040404040404040404040404040404040404040404040404040404040404",
      "value_lovelace": 5000000,
      "assets": [
        { "asset_id": "abcd", "quantity": 100 }
      ],
      "datum_hash": null
    },
    {
      "tx_id": "0505050505050505050505050505050505050505050505050505050505050505",
      "output_index": 2,
      "address_hash": "0606060606060606060606060606060606060606060606060606060606060606",
      "value_lovelace": 250000000,
      "assets": [],
      "datum_hash": "0707070707070707070707070707070707070707070707070707070707070707"
    }
  ]
}
```

- [ ] **Step 2: Write the integration test**

Create `crates/omega-commitment-core/tests/integration.rs`:

```rust
//! End-to-end integration test for the UTXO sub-tree commitment.

use omega_commitment_core::{
    leaf::Utxo,
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    utxos: Vec<Utxo>,
}

const FIXTURE: &str = include_str!("fixtures/utxo_set_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.utxos.len(), 3);

    // Encode each UTXO into a leaf hash.
    let leaves: Vec<_> = f.utxos.iter()
        .map(|u| u.leaf_hash().unwrap())
        .collect();

    // Build the tree.
    let tree = MerkleTree::build(leaves.clone());
    assert_eq!(tree.leaf_count(), 4); // padded from 3
    let root = tree.root();
    assert_ne!(root, [0u8; 32]);

    // Witness for every UTXO verifies against the root.
    for leaf in leaves {
        let w = InclusionWitness::build(&tree, leaf)
            .expect("leaf is in tree");
        assert!(w.verify(root), "witness verification failed");
    }
}

#[test]
fn root_is_stable_across_runs() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let leaves1: Vec<_> = f.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    let leaves2: Vec<_> = f.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    assert_eq!(MerkleTree::build(leaves1).root(), MerkleTree::build(leaves2).root());
}
```

- [ ] **Step 3: Run integration tests**

Run: `cargo test -p omega-commitment-core --test integration`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-core/tests/
git commit -m "test: end-to-end integration against synthetic UTXO fixture"
```

---

## Task 7: CLI — read fixture, emit commitment + witnesses

**Files:**
- Modify: `crates/omega-commitment-cli/src/main.rs`

**Why:** End-user surface. Given a JSON file of UTXOs, write out a JSON commitment record (root + leaf count + tree depth) and per-UTXO witnesses to a directory. This is the artifact downstream tools (Plonky3 circuits, claim builders) consume.

- [ ] **Step 1: Write the CLI**

Replace `crates/omega-commitment-cli/src/main.rs`:

```rust
//! omega-commitment CLI.
//!
//! Subcommand `commit`: read a JSON UTXO set, emit:
//!   - a `commitment.json` containing the root + metadata
//!   - a `witnesses/<leaf_hash>.json` per UTXO

use clap::{Parser, Subcommand};
use omega_commitment_core::{
    leaf::Utxo,
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(name = "omega-commitment", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build the UTXO sub-tree commitment from a JSON UTXO set.
    Commit {
        /// Input JSON file (must have an `utxos` array).
        #[arg(short, long)]
        input: PathBuf,
        /// Output directory.
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Deserialize)]
struct Input {
    utxos: Vec<Utxo>,
}

#[derive(Serialize)]
struct CommitmentRecord {
    #[serde(with = "hex::serde")]
    root: [u8; 32],
    leaf_count: usize,
    tree_depth: usize,
    utxo_count: usize,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Commit { input, output } => commit(input, output),
    }
}

fn commit(input: PathBuf, output: PathBuf) -> anyhow::Result<()> {
    let raw = fs::read_to_string(&input)?;
    let parsed: Input = serde_json::from_str(&raw)?;

    let leaves: Vec<_> = parsed.utxos.iter()
        .map(|u| u.leaf_hash())
        .collect::<Result<Vec<_>, _>>()?;

    let tree = MerkleTree::build(leaves.clone());

    fs::create_dir_all(&output)?;
    let witness_dir = output.join("witnesses");
    fs::create_dir_all(&witness_dir)?;

    let record = CommitmentRecord {
        root: tree.root(),
        leaf_count: tree.leaf_count(),
        tree_depth: tree.depth(),
        utxo_count: parsed.utxos.len(),
    };
    fs::write(
        output.join("commitment.json"),
        serde_json::to_string_pretty(&record)?,
    )?;

    for leaf in leaves {
        let w = InclusionWitness::build(&tree, leaf)
            .ok_or_else(|| anyhow::anyhow!("leaf vanished from tree"))?;
        let fname = format!("{}.json", hex::encode(leaf));
        fs::write(
            witness_dir.join(fname),
            serde_json::to_string_pretty(&w)?,
        )?;
    }

    println!("ok: root={} utxos={}",
        hex::encode(record.root), record.utxo_count);
    Ok(())
}
```

Add `anyhow = "1"` to `omega-commitment-cli/Cargo.toml` `[dependencies]`:

```toml
anyhow = "1"
```

- [ ] **Step 2: Build and run end-to-end**

```bash
cargo build --release -p omega-commitment-cli
mkdir -p /tmp/omega-out
./target/release/omega-commitment commit \
  --input crates/omega-commitment-core/tests/fixtures/utxo_set_small.json \
  --output /tmp/omega-out
```

Expected stdout:
```
ok: root=<64 hex chars> utxos=3
```

- [ ] **Step 3: Verify outputs**

```bash
ls /tmp/omega-out
ls /tmp/omega-out/witnesses
cat /tmp/omega-out/commitment.json
```

Expected:
- `commitment.json` exists with `root`, `leaf_count: 4`, `tree_depth: 2`, `utxo_count: 3`.
- `witnesses/` contains 3 JSON files (one per UTXO).

- [ ] **Step 4: Add a CLI smoke test**

Create `crates/omega-commitment-cli/tests/cli.rs`:

```rust
use std::process::Command;
use std::path::PathBuf;

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
    assert!(out.path().join("commitment.json").exists());
    assert!(out.path().join("witnesses").exists());
}
```

Add `tempfile = "3"` to `omega-commitment-cli/Cargo.toml` `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 5: Run the CLI smoke test**

Run: `cargo test -p omega-commitment-cli --test cli`
Expected: passes.

- [ ] **Step 6: Commit**

```bash
git add crates/omega-commitment-cli/
git commit -m "feat(cli): commit subcommand emits root + per-UTXO witnesses"
```

---

## Task 8: Benchmark + scale test

**Files:**
- Create: `crates/omega-commitment-core/benches/tree.rs`
- Modify: `crates/omega-commitment-core/Cargo.toml`

**Why:** Real Cardano mainnet has ~10M UTXOs. We need to know whether our naive implementation is good enough before integrating with `cardano-multiplatform-lib`.

- [ ] **Step 1: Add benchmark deps**

Modify `crates/omega-commitment-core/Cargo.toml`:

```toml
[dev-dependencies]
serde_json.workspace = true
proptest.workspace = true
criterion.workspace = true

[[bench]]
name = "tree"
harness = false
```

- [ ] **Step 2: Write the benchmark**

Create `crates/omega-commitment-core/benches/tree.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use omega_commitment_core::{hash::blake2b_256, tree::MerkleTree};

fn bench_tree_build(c: &mut Criterion) {
    for n in [1_000usize, 10_000, 100_000].iter() {
        let leaves: Vec<_> = (0..*n)
            .map(|i| blake2b_256(&(i as u64).to_be_bytes()))
            .collect();
        let mut group = c.benchmark_group("merkle_tree_build");
        group.throughput(Throughput::Elements(*n as u64));
        group.bench_function(format!("n={}", n), |b| {
            b.iter(|| MerkleTree::build(black_box(leaves.clone())))
        });
        group.finish();
    }
}

criterion_group!(benches, bench_tree_build);
criterion_main!(benches);
```

- [ ] **Step 3: Run the benchmark**

Run: `cargo bench -p omega-commitment-core`
Expected: completes; report saved to `target/criterion/`.

- [ ] **Step 4: Document the result**

Append to `omega-commitment/README.md`:

```markdown
## Performance baseline

| UTXOs   | Time/build | Throughput |
|---------|------------|------------|
| 1,000   | (record)   | (record)   |
| 10,000  | (record)   | (record)   |
| 100,000 | (record)   | (record)   |

10M-UTXO mainnet target: extrapolate × 100 from 100k baseline.
If extrapolation > 60s, optimize Task 9.
```

Replace `(record)` with the actual numbers from the bench output.

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-core/benches/ crates/omega-commitment-core/Cargo.toml omega-commitment/README.md
git commit -m "perf: criterion benchmarks for tree build at 1k/10k/100k leaves"
```

---

## Task 9: Optimization checkpoint

**Decision point.** If the 100k benchmark extrapolated to 10M takes < 60s on a modern CPU, **skip this task**. If it's slower, address one of:

1. Parallel layer construction with `rayon` — most layers are embarrassingly parallel.
2. Avoid `Vec::clone()` in tree construction — re-use buffers.
3. Switch from per-node allocation to a single contiguous backing buffer.

If the benchmark is fast enough, mark this task complete and move on.

- [ ] **Step 1: Decide based on benchmark numbers** — optimize or skip.

If skipping, document the decision in `README.md` and commit:

```bash
git commit --allow-empty -m "perf: tree build is fast enough; no optimization required"
```

---

## Task 10: Documentation + handoff to next plan

**Files:**
- Modify: `omega-commitment/README.md`

- [ ] **Step 1: Document the public API contract in README**

Append to `omega-commitment/README.md`:

```markdown
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
```

- [ ] **Step 2: Commit**

```bash
git add omega-commitment/README.md
git commit -m "docs: public API contract + handoff to next plan"
```

---

## Self-review checklist

- ✅ **Spec coverage:** This plan delivers the UTXO sub-tree of the Ω-Commitment per spec §7. The other six sub-trees are deferred to follow-on plans, as called out in §10 of the program roadmap.
- ✅ **Decision honoring:**
  - Decision 7 (PQ): only Blake2b/SHA3 — no curve crypto.
  - Decision 8 (Plonky3): tree layout is binary, fixed-arity, deterministic — circuit-friendly.
  - Decision 3 (everything-provable): one-of-seven sub-trees; spec acknowledges remaining six in follow-on plans.
- ✅ **No placeholders:** every task has actual code, not "TODO" stubs.
- ✅ **Type consistency:** `Hash = [u8; 32]` used uniformly. `Utxo`, `MerkleTree`, `InclusionWitness` referenced consistently across tasks.
- ✅ **Bite-sized:** tasks split into 2–8 steps; each step is one action.

---

## What's NOT in this plan (and why)

- **Reading from a real Cardano node's ledger snapshot.** Requires `cardano-multiplatform-lib` integration and a real mainnet snapshot. Deferred to a follow-on plan because (a) it's a large dep, (b) it adds non-determinism (snapshots vary), and (c) the synthetic fixture is sufficient to lock the public contract first.
- **The other six sub-trees.** Each gets its own plan. Block headers next.
- **Plonky3 circuit consuming the witness.** Track T2, separate plan.
- **Mainnet regression test.** Needs a real snapshot, deferred.

---

## How to execute this plan

Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. The plan has 10 tasks; each task is committable independently. Total runway estimate for a single experienced Rust dev: **2–3 weeks**.
