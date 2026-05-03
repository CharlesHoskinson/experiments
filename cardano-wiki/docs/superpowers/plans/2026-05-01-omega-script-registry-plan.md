# Omega Script Registry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the script-registry sub-tree (sub-tree 5 of 7) so developers can re-anchor a Plutus validator hash on the new chain with verifiable lineage. Powers `claim_script` proofs per spec §9.4 — pure provenance/identity continuity, no script re-execution.

**Architecture:** Reuse `tree.rs`, `witness.rs`, `serde_helpers` unchanged. Add a new `script_registry_leaf.rs` with a fixed-width 41-byte canonical encoding (script_hash ‖ deployment_slot ‖ script_size_bytes ‖ language). CLI gains a `script` arm. Bump to v0.5.0.

**Tech Stack:** Rust 1.79+, blake2, sha3, serde, clap (no new deps).

**Track:** T1 (Ω-Commitment Tooling), sub-tree 5. See `2026-05-01-ouroboros-omega-program-roadmap.md`.

**Locked design decisions honored (unchanged):**
- Decision 7 (PQ-only crypto): Blake2b-256 leaf hash. The 28-byte `script_hash` is Cardano-native Blake2b-224 — upstream data, outside our crypto scope.
- Decision 8 (Plonky3-friendly): same MerkleTree (binary, fixed-arity, sorted-padded). Script leaves sort by leaf hash; verifiers reconstruct from preimage.
- Decision 3 (everything-provable): adding sub-tree 5 of 7. Two remaining: stake state, governance state.
- **Decision 9 (dual-hash resolved 2026-05-01)**: per-sub-tree tooling stays Blake2b-only. The dual-track lives at the future bundle-assembly tooling layer.

**Cross-sub-tree consistency:**
- Sub-tree 4 (token policies) used a 28-byte `policy_id` — Cardano Blake2b-224 width.
- Sub-tree 5 also uses a 28-byte `script_hash` — same width, same upstream provenance. The pattern of "Cardano-native 28-byte identifiers in the preimage; 32-byte Blake2b-256 leaf hash" is now the established convention for Cardano-side hashes appearing inside Ω-Commitment leaves.

---

## File structure (post-plan)

```
omega-commitment/
├── Cargo.toml                                           (workspace)
├── README.md                                            (extended: v0.5.0 release notes)
├── crates/
│   ├── omega-commitment-core/
│   │   ├── Cargo.toml                                   (version bump to 0.5.0)
│   │   ├── src/
│   │   │   ├── lib.rs                                   (add `pub mod script_registry_leaf`)
│   │   │   ├── hash.rs                                  (unchanged)
│   │   │   ├── serde_helpers.rs                         (unchanged)
│   │   │   ├── utxo_leaf.rs                             (unchanged)
│   │   │   ├── header_leaf.rs                           (unchanged)
│   │   │   ├── tx_index_leaf.rs                         (unchanged)
│   │   │   ├── token_policy_leaf.rs                     (unchanged)
│   │   │   ├── script_registry_leaf.rs                  (NEW)
│   │   │   ├── tree.rs                                  (unchanged)
│   │   │   └── witness.rs                               (unchanged)
│   │   ├── tests/
│   │   │   ├── fixtures/
│   │   │   │   ├── utxo_set_small.json                  (existing)
│   │   │   │   ├── header_chain_small.json              (existing)
│   │   │   │   ├── tx_index_small.json                  (existing)
│   │   │   │   ├── token_policies_small.json            (existing)
│   │   │   │   └── script_registry_small.json           (NEW)
│   │   │   ├── utxo_integration.rs                      (existing)
│   │   │   ├── header_integration.rs                    (existing)
│   │   │   ├── tx_index_integration.rs                  (existing)
│   │   │   ├── token_policy_integration.rs              (existing)
│   │   │   └── script_registry_integration.rs           (NEW)
│   │   └── benches/tree.rs                              (unchanged)
│   └── omega-commitment-cli/
│       ├── Cargo.toml                                   (version bump to 0.5.0)
│       ├── src/main.rs                                  (modify: add Script variant + ScriptInput + build_script_leaves + match arm)
│       └── tests/cli.rs                                 (extend with script-registry smoke test)
```

---

## Task 1: `script_registry_leaf.rs` — canonical script-registry encoding

**Files:**
- Create: `crates/omega-commitment-core/src/script_registry_leaf.rs`
- Modify: `crates/omega-commitment-core/src/lib.rs`

A script-registry leaf is the deterministic serialization of:
```
script_hash (28 bytes) || deployment_slot (u64 BE) || script_size_bytes (u32 BE) || language (u8)
```
Total: 41 bytes. Hashed with Blake2b-256 to produce the 32-byte leaf hash.

`language` byte: 0 = native multi-sig (timelock), 1 = Plutus V1, 2 = Plutus V2, 3 = Plutus V3 (Plomin era). Future variants reserved.

- [ ] **Step 1: Create `script_registry_leaf.rs`**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/src/script_registry_leaf.rs`

```rust
//! Canonical Plutus / native-script registry leaf encoding.
//!
//! A script-registry leaf is the deterministic serialization of:
//!   (script_hash: 28 bytes) || (deployment_slot: u64 BE) ||
//!   (script_size_bytes: u32 BE) || (language: u8)
//!
//! Total: 41 bytes. The leaf is hashed with Blake2b-256 to produce
//! the leaf hash that goes into the Merkle tree. This sub-tree powers
//! `claim_script` transactions: developers re-anchor a validator hash
//! on the new chain with verifiable lineage. Pure provenance/identity
//! continuity — does NOT re-execute scripts.
//!
//! ## script_hash width
//!
//! Cardano script hashes are 28 bytes (Blake2b-224 of the canonical
//! script bytes), matching the policy-hash width in `token_policy_leaf`.
//! See that module's docstring for the full rationale on why preimage
//! widths can differ from the 32-byte leaf-hash output.
//!
//! ## language byte
//!
//! - `0` = native multi-sig (timelock script)
//! - `1` = Plutus V1
//! - `2` = Plutus V2 (Vasil)
//! - `3` = Plutus V3 (Plomin)
//!
//! Future variants are reserved. The encoding intentionally uses a
//! fixed `u8` slot rather than an open-ended enum so the byte layout
//! stays stable across language additions.

use crate::hash::{blake2b_256, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 28-byte Cardano script hash (Blake2b-224 of the canonical script
/// bytes). Distinct from the 32-byte `Hash` type used for internal
/// Merkle hashing.
pub type ScriptHash = [u8; 28];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ScriptEntry {
    #[serde(with = "hex::serde")]
    pub script_hash: ScriptHash,
    pub deployment_slot: u64,
    pub script_size_bytes: u32,
    pub language: u8,
}

impl ScriptEntry {
    /// Canonical 41-byte serialization.
    pub fn encode(&self) -> [u8; 41] {
        let mut out = [0u8; 41];
        out[0..28].copy_from_slice(&self.script_hash);
        out[28..36].copy_from_slice(&self.deployment_slot.to_be_bytes());
        out[36..40].copy_from_slice(&self.script_size_bytes.to_be_bytes());
        out[40] = self.language;
        out
    }

    /// Compute the leaf hash: Blake2b-256 of canonical encoding.
    pub fn leaf_hash(&self) -> Hash {
        blake2b_256(&self.encode())
    }
}

/// Validate that no `script_hash` appears more than once across the
/// entries. Returns the index of the second occurrence of the first
/// duplicate, or None if all `script_hash`es are unique.
///
/// Cardano script hashes are deterministic Blake2b-224 of the
/// canonical script bytes; duplicates indicate a data error
/// (e.g., overlapping epoch ranges in the input snapshot). This is
/// an OPTIONAL sanity helper; commitment generation does NOT require
/// uniqueness.
pub fn validate_script_hash_uniqueness(entries: &[ScriptEntry]) -> Option<usize> {
    let mut seen: HashSet<ScriptHash> = HashSet::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        if !seen.insert(e.script_hash) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(byte: u8, slot: u64, size: u32, lang: u8) -> ScriptEntry {
        ScriptEntry {
            script_hash: [byte; 28],
            deployment_slot: slot,
            script_size_bytes: size,
            language: lang,
        }
    }

    #[test]
    fn encoding_is_exactly_41_bytes() {
        let s = sample(0x11, 100, 2048, 2);
        assert_eq!(s.encode().len(), 41);
    }

    #[test]
    fn encoding_layout_is_correct() {
        let s = ScriptEntry {
            script_hash: [0xAAu8; 28],
            deployment_slot: 0x0102030405060708,
            script_size_bytes: 0x11223344,
            language: 0x07,
        };
        let bytes = s.encode();
        assert_eq!(&bytes[0..28], &[0xAAu8; 28]);
        assert_eq!(
            &bytes[28..36],
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
        assert_eq!(&bytes[36..40], &[0x11, 0x22, 0x33, 0x44]);
        assert_eq!(bytes[40], 0x07);
    }

    #[test]
    fn script_hash_is_28_bytes() {
        let s = sample(0x11, 100, 0, 0);
        assert_eq!(s.script_hash.len(), 28);
    }

    #[test]
    fn leaf_hash_is_32_bytes() {
        let s = sample(0x11, 100, 0, 0);
        assert_eq!(s.leaf_hash().len(), 32);
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let s = sample(0x11, 100, 1024, 2);
        assert_eq!(s.leaf_hash(), s.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_script_hash_change() {
        let a = sample(0x11, 100, 1024, 2);
        let b = sample(0x12, 100, 1024, 2);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_slot_change() {
        let a = sample(0x11, 100, 1024, 2);
        let b = sample(0x11, 101, 1024, 2);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_size_change() {
        let a = sample(0x11, 100, 1024, 2);
        let b = sample(0x11, 100, 1025, 2);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_language_change() {
        let a = sample(0x11, 100, 1024, 2);
        let b = sample(0x11, 100, 1024, 3);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn all_four_languages_produce_distinct_leaves() {
        let leaves: Vec<Hash> = (0..=3u8)
            .map(|lang| sample(0x11, 100, 1024, lang).leaf_hash())
            .collect();
        // All four leaf hashes pairwise distinct.
        for i in 0..leaves.len() {
            for j in (i + 1)..leaves.len() {
                assert_ne!(leaves[i], leaves[j], "lang {} vs {} collided", i, j);
            }
        }
    }

    #[test]
    fn future_language_bytes_round_trip() {
        // language=255 (reserved) must encode and hash without panic.
        let s = sample(0x11, 100, 1024, 255);
        let bytes = s.encode();
        assert_eq!(bytes[40], 255);
        let _ = s.leaf_hash();
    }

    #[test]
    fn validate_script_hash_uniqueness_accepts_unique() {
        let entries = vec![
            sample(0x01, 1, 100, 0),
            sample(0x02, 2, 200, 1),
            sample(0x03, 3, 300, 2),
        ];
        assert_eq!(validate_script_hash_uniqueness(&entries), None);
    }

    #[test]
    fn validate_script_hash_uniqueness_finds_duplicate() {
        let entries = vec![
            sample(0x01, 1, 100, 0),
            sample(0x02, 2, 200, 1),
            sample(0x01, 5, 999, 3),
        ];
        assert_eq!(validate_script_hash_uniqueness(&entries), Some(2));
    }

    #[test]
    fn validate_script_hash_uniqueness_empty_is_valid() {
        assert_eq!(validate_script_hash_uniqueness(&[]), None);
    }

    #[test]
    fn same_script_hash_different_deploy_slot_distinct_leaves() {
        // Even if upstream data accidentally has the same hash twice
        // with different metadata, leaf hashes diverge — confirming
        // the entire tuple contributes to leaf identity.
        let a = sample(0x11, 100, 1024, 2);
        let b = sample(0x11, 200, 1024, 2);
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
//! inclusion witnesses. v0.5.0 supports five of seven Ω-Commitment sub-trees:
//! UTXO set, block header chain, transaction index, native token policies,
//! and script registry.

pub mod hash;
pub mod serde_helpers;
pub mod tree;
pub mod witness;
pub mod utxo_leaf;
pub mod header_leaf;
pub mod tx_index_leaf;
pub mod token_policy_leaf;
pub mod script_registry_leaf;
```

- [ ] **Step 3: Run tests**

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
cargo test -p omega-commitment-core script_registry_leaf::tests 2>&1 | tail -25
```

Expected: 14 tests pass.

- [ ] **Step 4: Run full workspace + lint + fmt**

```bash
cargo test --workspace 2>&1 | tail -5    # 100 total tests pass (86 prior + 14 new)
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

If `cargo fmt-check` shows diffs, run `cargo fmt --all` and re-verify.

If `cargo lint` reports anything, fix minimally before committing.

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-core/src/script_registry_leaf.rs \
        crates/omega-commitment-core/src/lib.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(script_registry_leaf): canonical 41-byte encoding + uniqueness validator"
```

---

## Task 2: Script-registry integration test

**Files:**
- Create: `crates/omega-commitment-core/tests/fixtures/script_registry_small.json`
- Create: `crates/omega-commitment-core/tests/script_registry_integration.rs`

8-entry synthetic fixture covering all four `language` values (native multisig, Plutus V1/V2/V3) plus one entry per language with a different size and slot, exercising every encoding path.

- [ ] **Step 1: Write fixture**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/tests/fixtures/script_registry_small.json`

```json
{
  "scripts": [
    {
      "script_hash": "11000000000000000000000000000000000000000000000000000000",
      "deployment_slot": 100,
      "script_size_bytes": 256,
      "language": 0
    },
    {
      "script_hash": "22000000000000000000000000000000000000000000000000000000",
      "deployment_slot": 200,
      "script_size_bytes": 1024,
      "language": 1
    },
    {
      "script_hash": "33000000000000000000000000000000000000000000000000000000",
      "deployment_slot": 350,
      "script_size_bytes": 2048,
      "language": 2
    },
    {
      "script_hash": "44000000000000000000000000000000000000000000000000000000",
      "deployment_slot": 400,
      "script_size_bytes": 4096,
      "language": 3
    },
    {
      "script_hash": "55000000000000000000000000000000000000000000000000000000",
      "deployment_slot": 500,
      "script_size_bytes": 512,
      "language": 0
    },
    {
      "script_hash": "66000000000000000000000000000000000000000000000000000000",
      "deployment_slot": 700,
      "script_size_bytes": 8192,
      "language": 2
    },
    {
      "script_hash": "77000000000000000000000000000000000000000000000000000000",
      "deployment_slot": 1000,
      "script_size_bytes": 16384,
      "language": 3
    },
    {
      "script_hash": "88000000000000000000000000000000000000000000000000000000",
      "deployment_slot": 1500,
      "script_size_bytes": 32,
      "language": 1
    }
  ]
}
```

Notes on the fixture:
- 2 entries per language (native, V1, V2, V3) → all 4 language paths exercised
- Sizes range from 32 bytes (tiny) to 16 KiB (large) → exercises u32 size encoding
- Slots are non-contiguous → confirms tree doesn't depend on slot continuity (consistent with sub-tree 2 / sub-tree 3 fixtures)
- All `script_hash`es unique → uniqueness validator should accept

- [ ] **Step 2: Write integration test**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/tests/script_registry_integration.rs`

```rust
//! End-to-end integration test for the script-registry sub-tree.

use omega_commitment_core::{
    script_registry_leaf::{validate_script_hash_uniqueness, ScriptEntry},
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    scripts: Vec<ScriptEntry>,
}

const FIXTURE: &str = include_str!("fixtures/script_registry_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.scripts.len(), 8);

    assert!(
        validate_script_hash_uniqueness(&f.scripts).is_none(),
        "fixture has duplicate script_hashes"
    );

    let leaves: Vec<_> = f.scripts.iter().map(|s| s.leaf_hash()).collect();
    let tree = MerkleTree::build(leaves.clone());
    assert_eq!(tree.leaf_count(), 8);
    assert_eq!(tree.depth(), 3);
    let root = tree.root();
    assert_ne!(root, [0u8; 32]);

    for leaf in leaves {
        let w = InclusionWitness::build(&tree, leaf).expect("leaf is in tree");
        assert!(w.verify(root), "witness verification failed");
    }
}

#[test]
fn root_is_stable_across_runs() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let leaves1: Vec<_> = f.scripts.iter().map(|s| s.leaf_hash()).collect();
    let leaves2: Vec<_> = f.scripts.iter().map(|s| s.leaf_hash()).collect();
    assert_eq!(
        MerkleTree::build(leaves1).root(),
        MerkleTree::build(leaves2).root()
    );
}

#[test]
fn all_four_languages_present_in_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let langs: std::collections::HashSet<u8> =
        f.scripts.iter().map(|s| s.language).collect();
    assert_eq!(langs.len(), 4, "expected all 4 language values");
    for expected in 0..=3u8 {
        assert!(langs.contains(&expected), "missing language={expected}");
    }
}

#[test]
fn duplicate_script_hash_rejected_by_validator() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let mut scripts = f.scripts;
    let dup = scripts[0].clone();
    scripts.push(dup);
    assert_eq!(validate_script_hash_uniqueness(&scripts), Some(8));
}
```

- [ ] **Step 3: Run integration tests**

```bash
cargo test -p omega-commitment-core --test script_registry_integration 2>&1 | tail -10
```

Expected: 4 tests pass.

- [ ] **Step 4: Run full workspace + lint + fmt**

```bash
cargo test --workspace 2>&1 | tail -5    # 104 total
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-core/tests/script_registry_integration.rs \
        crates/omega-commitment-core/tests/fixtures/script_registry_small.json
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test: script-registry sub-tree integration test against synthetic 8-entry fixture"
```

---

## Task 3: CLI `script` arm

**Files:**
- Modify: `crates/omega-commitment-cli/src/main.rs`

Add a fifth variant to `SubTree` (already `#[non_exhaustive]` from v0.4.0), a fifth `Input` struct, a fifth free builder function, and a fifth match arm. No need to re-mark `#[non_exhaustive]` — it's already in place.

- [ ] **Step 1: Add `Script` variant to `SubTree` enum**

Edit `/home/hoskinson/omega-commitment/crates/omega-commitment-cli/src/main.rs`. Find the `SubTree` enum (currently has `Utxo`, `Header`, `TxIndex`, `TokenPolicy`). Replace with:

```rust
#[derive(Copy, Clone, Debug, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
enum SubTree {
    Utxo,
    Header,
    TxIndex,
    TokenPolicy,
    Script,
}
```

The kebab-case rule renders `Script` as `"script"` in JSON, matching the CLI flag spelling.

- [ ] **Step 2: Update the import block**

Find the existing import block and add `script_registry_leaf::ScriptEntry`:

```rust
use omega_commitment_core::{
    hash::{blake2b_256, Hash},
    header_leaf::BlockHeader,
    script_registry_leaf::ScriptEntry,
    token_policy_leaf::TokenPolicy,
    tree::MerkleTree,
    tx_index_leaf::TxIndexEntry,
    utxo_leaf::Utxo,
    witness::InclusionWitness,
};
```

(Order alphabetically to match the existing convention.)

- [ ] **Step 3: Add `ScriptInput` struct**

After the existing `TokenPolicyInput` struct, add:

```rust
#[derive(Deserialize)]
struct ScriptInput {
    scripts: Vec<ScriptEntry>,
}
```

- [ ] **Step 4: Add `build_script_leaves` free function**

After the existing `build_token_policy_leaves` function, add:

```rust
fn build_script_leaves(raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    let parsed: ScriptInput = serde_json::from_str(raw)?;
    let leaves: Vec<Hash> = parsed.scripts.iter().map(|s| s.leaf_hash()).collect();
    let n = parsed.scripts.len();
    Ok((leaves, n))
}
```

- [ ] **Step 5: Add the dispatcher arm**

Find the `match sub_tree { ... }` in `commit()`. Add the new arm:

```rust
    let (leaves, item_count) = match sub_tree {
        SubTree::Utxo => build_utxo_leaves(&raw)?,
        SubTree::Header => build_header_leaves(&raw)?,
        SubTree::TxIndex => build_tx_index_leaves(&raw)?,
        SubTree::TokenPolicy => build_token_policy_leaves(&raw)?,
        SubTree::Script => build_script_leaves(&raw)?,
    };
```

- [ ] **Step 6: Build and run all 5 sub-trees end-to-end**

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
cargo build --release -p omega-commitment-cli 2>&1 | tail -5
```

Script smoke:
```bash
mkdir -p /tmp/o-s && rm -rf /tmp/o-s/*
./target/release/omega-commitment commit --sub-tree script \
  --input crates/omega-commitment-core/tests/fixtures/script_registry_small.json \
  --output /tmp/o-s
cat /tmp/o-s/commitment.json
ls /tmp/o-s/witnesses/ | wc -l   # expect 8
```

Expected: `"sub_tree": "script"`, `"item_count": 8`, 8 witness files.

Sanity: confirm prior sub-trees still work:
```bash
mkdir -p /tmp/o-tp && rm -rf /tmp/o-tp/*
./target/release/omega-commitment commit --sub-tree token-policy \
  --input crates/omega-commitment-core/tests/fixtures/token_policies_small.json \
  --output /tmp/o-tp
cat /tmp/o-tp/commitment.json   # still works, sub_tree=token-policy
```

- [ ] **Step 7: Run all tests + lint + fmt**

```bash
cargo test --workspace 2>&1 | tail -5    # 104 still pass
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

- [ ] **Step 8: Commit**

```bash
git add crates/omega-commitment-cli/src/main.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(cli): add script arm to --sub-tree dispatcher"
```

---

## Task 4: CLI smoke test for script

**Files:**
- Modify: `crates/omega-commitment-cli/tests/cli.rs`

Add one new smoke test parallel to the existing utxo / header / tx-index / token-policy ones.

- [ ] **Step 1: Append to `crates/omega-commitment-cli/tests/cli.rs`**

```rust

#[test]
fn cli_commit_script_smoke() {
    let out = run_commit("script", "script_registry_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(
        body.contains("\"sub_tree\": \"script\""),
        "wrong sub_tree tag: {body}"
    );
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 8"));
    let witness_count = fs::read_dir(out.path().join("witnesses")).unwrap().count();
    assert_eq!(witness_count, 8, "expected 8 script witness files");
}
```

(`run_commit` and `fixture_path` are already defined at the top of the file from prior plans.)

- [ ] **Step 2: Run CLI tests**

```bash
cargo test -p omega-commitment-cli --test cli 2>&1 | tail -10
```

Expected: 9 tests pass (8 prior + 1 new).

- [ ] **Step 3: Run full workspace + lint + fmt**

```bash
cargo test --workspace 2>&1 | tail -5    # 105 total
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-cli/tests/cli.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test(cli): script-registry smoke test"
```

---

## Task 5: Bump to v0.5.0 + extend README

**Files:**
- Modify: `crates/omega-commitment-core/Cargo.toml`
- Modify: `crates/omega-commitment-cli/Cargo.toml`
- Modify: `README.md`

- [ ] **Step 1: Bump core crate version**

In `/home/hoskinson/omega-commitment/crates/omega-commitment-core/Cargo.toml`, change `version = "0.4.0"` to `version = "0.5.0"`.

- [ ] **Step 2: Bump CLI crate version**

In `/home/hoskinson/omega-commitment/crates/omega-commitment-cli/Cargo.toml`, change `version = "0.4.0"` to `version = "0.5.0"`.

- [ ] **Step 3: Verify build + tests + lint + fmt**

```bash
cargo build --workspace 2>&1 | grep -E "warning|error"   # empty
cargo lint 2>&1 | tail -3                                  # clean
cargo fmt-check 2>&1 | tail -3                             # clean
cargo test --workspace 2>&1 | tail -5                       # 105 tests pass
```

- [ ] **Step 4: Append to README.md**

Append to `/home/hoskinson/omega-commitment/README.md`:

```markdown
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
```

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-core/Cargo.toml \
        crates/omega-commitment-cli/Cargo.toml \
        README.md
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "chore: bump to 0.5.0; document script-registry sub-tree"
```

- [ ] **Step 6: Final verification**

```bash
git log --oneline | head -8
cargo test --workspace 2>&1 | tail -5
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

Expected: HEAD is the version-bump commit, 105 tests pass, no lint/fmt issues.

---

## Self-review

**Spec coverage** (from `2026-05-01-ouroboros-omega-design.md` §7, sub-tree 5):
- "Script registry — Merkle tree of all unique Plutus validator hashes deployed via reference scripts or used as outputs, with deployment slot."
  - ✅ Leaf encodes (`script_hash`, `deployment_slot`, `script_size_bytes`, `language`).
  - ✅ Plonky3-friendly tree reused unchanged.
  - ✅ Witness format unchanged.
  - ✅ `claim_script` (spec §9.4) gets a tree to prove against.
  - ✅ The 28-byte `script_hash` matches Cardano on-chain semantics.

The spec mentions "deployment slot" but not size or language. The plan adds `script_size_bytes` and `language` as additional fields — these are useful for fee modeling and circuit ergonomics on the new chain. They don't contradict the spec; they extend the leaf with adjacent metadata. If a stricter reading is wanted, those fields can be dropped in a follow-up plan with no loss of `claim_script` functionality. Documented here so reviewers can object if they think it's overreach.

**Decision honoring:**
- ✅ Decision 7 (PQ-only): Blake2b for leaf hashing, no curves. `script_hash` is upstream Cardano data.
- ✅ Decision 8 (Plonky3-friendly): fixed-width 41-byte leaf encoding, sort-then-pad tree.
- ✅ Decision 3 (everything-provable): adds 5th of 7 sub-trees.
- ✅ Decision 9 (selective dual-track): per-sub-tree tooling stays Blake2b-only — no commitment-format change at the sub-tree layer.

**Placeholder scan:** All code blocks contain runnable code. No "TBD" inside implementation steps. ✅

**Type consistency:**
- `ScriptHash = [u8; 28]` defined once in `script_registry_leaf.rs`; used consistently in struct fields, fixture, integration test, CLI input.
- `ScriptEntry { script_hash, deployment_slot, script_size_bytes, language }` referenced uniformly across module, fixture, integration test, and CLI.
- `validate_script_hash_uniqueness` signature consistent.
- `SubTree::Script` enum variant + serde `kebab-case` renders as `"script"` matching CLI flag spelling and smoke-test substring assertion.
- `build_script_leaves` free function signature matches the pattern of `build_utxo_leaves` / `build_header_leaves` / `build_tx_index_leaves` / `build_token_policy_leaves`.
- ✅ No drift.

**Bite-sized tasks:** 5 tasks, each with 4–8 numbered steps; each step is a single action. ✅

**Net delta:** +14 unit tests + 4 integration tests + 1 CLI smoke test = +19 tests (86 → 105). 5 commits.

---

## What's NOT in this plan (and why)

- **Real Cardano mainnet script ingestion.** Synthetic fixture only. A `cardano-multiplatform-lib` integration is deferred.
- **Cross-sub-tree validation** (e.g., "every UTXO's script-credential references a script that exists in the script-registry sub-tree"). Requires both commitments loaded simultaneously; deferred to a future cross-validation plan.
- **Plutus V4+ language byte.** Not yet defined by Cardano upstream; the `u8` slot in the encoding accommodates future additions without re-encoding.
- **Plonky3 `claim_script` circuit.** Track T2.
- **Sub-trees 6 and 7** (stake state, governance state) — each gets its own plan.

---

## How to execute this plan

Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Five tasks, each independently committable.

Total runway estimate: **2–3 days** for an experienced Rust dev. The architecture is fully mature; sub-tree 5 follows the same pattern as 4. The only new wrinkle is the language-byte field, which is the smallest possible scalar (u8) and adds zero structural complexity.

Expected post-execution state:
- 5 commits added on top of v0.4.0 (currently 43 commits — 44 after the dual-hash README update commit `7207249`)
- ~19 net new tests (105 total)
- Both crates at version 0.5.0
- 5 of 7 sub-trees shipped; 2 remaining (stake state, governance state)

Next plan: `2026-XX-XX-omega-stake-state-plan.md` (sub-tree 6 of 7).
