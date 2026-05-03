# Omega Stake State + Governance State Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the final two of seven Ω-Commitment sub-trees in a single release: **stake state** (sub-tree 6) and **governance state** (sub-tree 7). Powers `claim_stake` (spec §9.5) and `claim_governance` (spec §9.6) — port over delegation/pool/DRep history and treasury/CC/gov-action state respectively.

**Architecture:** Same proven pattern from sub-trees 1–5. Two new modules (`stake_state_leaf.rs`, `governance_state_leaf.rs`); two new CLI arms; two integration tests; one CLI smoke test per sub-tree. Reuse `tree.rs`, `witness.rs`, `serde_helpers` unchanged. Single release: bump to **v0.6.0** at the end.

**Tech Stack:** Rust 1.79+, blake2, sha3, serde, clap (no new deps).

**Track:** T1 (Ω-Commitment Tooling), sub-trees 6 + 7 — completing the per-sub-tree leaf-tooling phase.

**Locked design decisions honored (unchanged):**
- Decision 7 (PQ-only crypto): Blake2b-256 leaf hash. Cardano-native hashes in preimages (28-byte Blake2b-224) handled via dedicated `[u8; 28]` aliases, same convention as sub-trees 4 and 5.
- Decision 8 (Plonky3-friendly): same MerkleTree (binary, fixed-arity, sorted-padded). Leaves sort by leaf hash; verifiers reconstruct from preimage.
- Decision 3 (everything-provable): adds 6th and 7th of 7 sub-trees → **the leaf-tooling phase of track T1 is complete after this plan ships.**
- Decision 9 (selective dual-track): per-sub-tree tooling stays Blake2b-only. Bundle layer is a future plan.

---

## Sub-tree 6 design — stake state

**What it commits to:** the per-stake-credential snapshot at fork height. One row per active stake credential, capturing its delegation, pool registration (if any), and DRep registration (if any).

**Leaf type:** `StakeEntry` with:
- `stake_credential_hash: [u8; 28]` — Cardano stake-credential hash (Blake2b-224 of the stake key or stake script). Same width convention as `policy_id` and `script_hash`.
- `delegated_pool: [u8; 28]` — pool-id the credential is delegating to (Blake2b-224 of pool's cold-key VRF). All-zeros means "not delegated."
- `delegated_drep: [u8; 28]` — DRep id the credential is delegating to. All-zeros means "no DRep delegation" (auto-abstain implicit). Two reserved patterns are documented but encoded as the literal 28-byte values: all-`0x00` = no delegation; the canonical "always-abstain" and "always-no-confidence" DRep IDs are external constants set by upstream Cardano governance and stored verbatim.
- `rewards_lovelace: u64` — accumulated rewards balance for this credential at fork height.
- `is_pool_operator: u8` — 1 if this credential is also registered as a pool operator's reward account, 0 otherwise. (One byte; reserves room for future flag bits without re-encoding.)

**Encoding (fixed-width):**
```
stake_credential_hash (28) || delegated_pool (28) || delegated_drep (28) || rewards_lovelace (u64 BE) || is_pool_operator (u8)
= 28 + 28 + 28 + 8 + 1 = 93 bytes
```

Hashed with Blake2b-256 → 32-byte leaf hash.

**Validator helper:** `validate_stake_credential_uniqueness(&[StakeEntry])` returns the index of the first duplicate `stake_credential_hash`, or None.

---

## Sub-tree 7 design — governance state

**What it commits to:** the on-chain governance state at fork height — treasury balance, sitting Constitutional Committee members, in-flight governance actions, and ratified-action history.

**Leaf type:** unlike sub-trees 1–6 which commit to a homogeneous list, governance state is a **heterogeneous mix**: treasury accounts, CC seats, governance action records, etc. The cleanest approach: one entry per **governance fact**, where each fact has a `kind: u8` discriminant and a fixed-width payload. This keeps the encoding canonical and Plonky3-friendly while allowing the tree to commit to multiple fact types in a single sub-tree.

`GovernanceFact` with:
- `kind: u8` — discriminant: `0` = treasury balance, `1` = CC seat, `2` = ratified gov action, `3` = in-flight gov action.
- `key: [u8; 32]` — fact identifier. For treasury, the all-zero hash. For CC seat, the member's credential hash (right-padded from 28 to 32 with zeros). For gov action, the action's tx_id. We use 32 bytes here (not 28) because gov-action tx_ids are full 32-byte Blake2b-256 hashes, and we want the largest-key encoding to hold all four cases without per-kind variance.
- `value: u128` — primary scalar. For treasury kind, the lovelace balance. For CC seat kind, the seat's expiration epoch (small integer, fits trivially). For ratified/in-flight gov action kinds, a packed bitfield encoding the action type (low 16 bits) and the slot at which it was ratified or submitted (next 64 bits); top 48 bits reserved.
- `slot: u64` — slot at fork height H (uniform across all entries; lets a verifier check the "as-of" timestamp without consulting the bundle).

**Encoding (fixed-width):**
```
kind (u8) || key (32) || value (u128 BE) || slot (u64 BE)
= 1 + 32 + 16 + 8 = 57 bytes
```

Hashed with Blake2b-256 → 32-byte leaf hash.

**Validator helper:** `validate_governance_keys_unique_per_kind(&[GovernanceFact])` returns the index of the first `(kind, key)` pair that repeats, or None. Note that the same `key` is allowed across different `kind`s (e.g., a single tx_id could be a ratified gov action AND referenced by an in-flight action — though in practice this is rare).

---

## File structure (post-plan)

```
omega-commitment/
├── Cargo.toml                                           (workspace)
├── README.md                                            (extended: v0.6.0 release notes)
├── crates/
│   ├── omega-commitment-core/
│   │   ├── Cargo.toml                                   (version bump to 0.6.0)
│   │   ├── src/
│   │   │   ├── lib.rs                                   (add stake_state_leaf + governance_state_leaf)
│   │   │   ├── hash.rs                                  (unchanged)
│   │   │   ├── serde_helpers.rs                         (unchanged)
│   │   │   ├── utxo_leaf.rs                             (unchanged)
│   │   │   ├── header_leaf.rs                           (unchanged)
│   │   │   ├── tx_index_leaf.rs                         (unchanged)
│   │   │   ├── token_policy_leaf.rs                     (unchanged)
│   │   │   ├── script_registry_leaf.rs                  (unchanged)
│   │   │   ├── stake_state_leaf.rs                      (NEW)
│   │   │   ├── governance_state_leaf.rs                 (NEW)
│   │   │   ├── tree.rs                                  (unchanged)
│   │   │   └── witness.rs                               (unchanged)
│   │   ├── tests/
│   │   │   ├── fixtures/
│   │   │   │   ├── ... (existing 5 fixtures unchanged)
│   │   │   │   ├── stake_state_small.json               (NEW)
│   │   │   │   └── governance_state_small.json          (NEW)
│   │   │   ├── ... (existing 5 integration tests unchanged)
│   │   │   ├── stake_state_integration.rs               (NEW)
│   │   │   └── governance_state_integration.rs          (NEW)
│   │   └── benches/tree.rs                              (unchanged)
│   └── omega-commitment-cli/
│       ├── Cargo.toml                                   (version bump to 0.6.0)
│       ├── src/main.rs                                  (modify: add Stake + Governance variants + 2 input structs + 2 builder functions + 2 match arms)
│       └── tests/cli.rs                                 (extend with 2 smoke tests)
```

---

## Task 1: `stake_state_leaf.rs` — canonical stake-state encoding

**Files:**
- Create: `crates/omega-commitment-core/src/stake_state_leaf.rs`
- Modify: `crates/omega-commitment-core/src/lib.rs`

- [ ] **Step 1: Create the module**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/src/stake_state_leaf.rs`

```rust
//! Canonical stake-state leaf encoding.
//!
//! A stake-state leaf is the deterministic serialization of:
//!   (stake_credential_hash: 28 bytes) || (delegated_pool: 28 bytes) ||
//!   (delegated_drep: 28 bytes) || (rewards_lovelace: u64 BE) ||
//!   (is_pool_operator: u8)
//!
//! Total: 93 bytes. The leaf is hashed with Blake2b-256 to produce
//! the leaf hash that goes into the Merkle tree. This sub-tree powers
//! `claim_stake` transactions: users port over delegation, pool, and
//! DRep history with verifiable lineage.
//!
//! ## Reserved values
//!
//! - `delegated_pool == [0u8; 28]` means the credential is not delegating
//!   to any pool.
//! - `delegated_drep == [0u8; 28]` means no active DRep delegation.
//!   The canonical "always-abstain" and "always-no-confidence" DRep IDs
//!   are upstream Cardano constants and are stored as their literal
//!   28-byte values (NOT encoded as zero).

use crate::hash::{blake2b_256, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 28-byte Cardano credential hash (Blake2b-224 of a stake key or
/// stake script). Used for `stake_credential_hash`, `delegated_pool`,
/// and `delegated_drep`.
pub type CredentialHash = [u8; 28];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct StakeEntry {
    #[serde(with = "hex::serde")]
    pub stake_credential_hash: CredentialHash,
    #[serde(with = "hex::serde")]
    pub delegated_pool: CredentialHash,
    #[serde(with = "hex::serde")]
    pub delegated_drep: CredentialHash,
    pub rewards_lovelace: u64,
    pub is_pool_operator: u8,
}

impl StakeEntry {
    /// Canonical 93-byte serialization.
    pub fn encode(&self) -> [u8; 93] {
        let mut out = [0u8; 93];
        out[0..28].copy_from_slice(&self.stake_credential_hash);
        out[28..56].copy_from_slice(&self.delegated_pool);
        out[56..84].copy_from_slice(&self.delegated_drep);
        out[84..92].copy_from_slice(&self.rewards_lovelace.to_be_bytes());
        out[92] = self.is_pool_operator;
        out
    }

    /// Compute the leaf hash: Blake2b-256 of canonical encoding.
    pub fn leaf_hash(&self) -> Hash {
        blake2b_256(&self.encode())
    }
}

/// Validate that no `stake_credential_hash` appears more than once
/// across the entries. Returns the index of the second occurrence
/// of the first duplicate, or None if all are unique.
///
/// Cardano stake credentials are deterministic; duplicates indicate
/// a data error. Optional sanity helper; commitment generation does
/// NOT require uniqueness.
pub fn validate_stake_credential_uniqueness(entries: &[StakeEntry]) -> Option<usize> {
    let mut seen: HashSet<CredentialHash> = HashSet::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        if !seen.insert(e.stake_credential_hash) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(byte: u8, rewards: u64, op: u8) -> StakeEntry {
        StakeEntry {
            stake_credential_hash: [byte; 28],
            delegated_pool: [byte.wrapping_add(1); 28],
            delegated_drep: [byte.wrapping_add(2); 28],
            rewards_lovelace: rewards,
            is_pool_operator: op,
        }
    }

    #[test]
    fn encoding_is_exactly_93_bytes() {
        let s = sample(0x11, 100, 0);
        assert_eq!(s.encode().len(), 93);
    }

    #[test]
    fn encoding_layout_is_correct() {
        let s = StakeEntry {
            stake_credential_hash: [0xAAu8; 28],
            delegated_pool: [0xBBu8; 28],
            delegated_drep: [0xCCu8; 28],
            rewards_lovelace: 0x0102030405060708,
            is_pool_operator: 0x09,
        };
        let bytes = s.encode();
        assert_eq!(&bytes[0..28], &[0xAAu8; 28]);
        assert_eq!(&bytes[28..56], &[0xBBu8; 28]);
        assert_eq!(&bytes[56..84], &[0xCCu8; 28]);
        assert_eq!(
            &bytes[84..92],
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
        assert_eq!(bytes[92], 0x09);
    }

    #[test]
    fn leaf_hash_is_32_bytes() {
        let s = sample(0x11, 100, 0);
        assert_eq!(s.leaf_hash().len(), 32);
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let s = sample(0x11, 100, 1);
        assert_eq!(s.leaf_hash(), s.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_credential_change() {
        let a = sample(0x11, 100, 0);
        let b = sample(0x12, 100, 0);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_pool_change() {
        let a = sample(0x11, 100, 0);
        let mut b = a.clone();
        b.delegated_pool = [0xFF; 28];
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_drep_change() {
        let a = sample(0x11, 100, 0);
        let mut b = a.clone();
        b.delegated_drep = [0xEE; 28];
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_rewards_change() {
        let a = sample(0x11, 100, 0);
        let b = sample(0x11, 101, 0);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_pool_operator_flag() {
        let a = sample(0x11, 100, 0);
        let b = sample(0x11, 100, 1);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn zero_pool_means_undelegated() {
        let s = StakeEntry {
            stake_credential_hash: [0x11; 28],
            delegated_pool: [0u8; 28],
            delegated_drep: [0u8; 28],
            rewards_lovelace: 0,
            is_pool_operator: 0,
        };
        let bytes = s.encode();
        assert_eq!(&bytes[28..56], &[0u8; 28]);
    }

    #[test]
    fn validate_stake_credential_uniqueness_accepts_unique() {
        let entries = vec![sample(0x01, 100, 0), sample(0x02, 200, 0), sample(0x03, 300, 1)];
        assert_eq!(validate_stake_credential_uniqueness(&entries), None);
    }

    #[test]
    fn validate_stake_credential_uniqueness_finds_duplicate() {
        let entries = vec![sample(0x01, 100, 0), sample(0x02, 200, 0), sample(0x01, 999, 0)];
        assert_eq!(validate_stake_credential_uniqueness(&entries), Some(2));
    }

    #[test]
    fn validate_stake_credential_uniqueness_empty_is_valid() {
        assert_eq!(validate_stake_credential_uniqueness(&[]), None);
    }
}
```

- [ ] **Step 2: Update `lib.rs`** — add `pub mod stake_state_leaf;` (governance comes in Task 4):

```rust
//! omega-commitment-core: Ω-Commitment sub-tree library.
//!
//! Provides canonical leaf encodings, a Plonky3-friendly Merkle tree, and
//! inclusion witnesses. v0.6.0 supports six of seven Ω-Commitment sub-trees.

pub mod hash;
pub mod serde_helpers;
pub mod tree;
pub mod witness;
pub mod utxo_leaf;
pub mod header_leaf;
pub mod tx_index_leaf;
pub mod token_policy_leaf;
pub mod script_registry_leaf;
pub mod stake_state_leaf;
```

(Sub-tree 7 will land in Task 4 and updates this file again to add `governance_state_leaf`. The intermediate state is fine — the workspace builds cleanly with just stake_state_leaf added.)

- [ ] **Step 3: Run tests + lint + fmt**

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
cargo test -p omega-commitment-core stake_state_leaf::tests 2>&1 | tail -20
cargo test --workspace 2>&1 | tail -5    # 119 total (106 prior + 13 new)
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

If fmt-check fails, run `cargo fmt --all`. If clippy flags anything, fix minimally.

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-core/src/stake_state_leaf.rs \
        crates/omega-commitment-core/src/lib.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(stake_state_leaf): canonical 93-byte encoding + uniqueness validator"
```

---

## Task 2: Stake-state integration test

**Files:**
- Create: `crates/omega-commitment-core/tests/fixtures/stake_state_small.json`
- Create: `crates/omega-commitment-core/tests/stake_state_integration.rs`

8-entry fixture covering: undelegated credentials (zero pool), delegated to multiple pools, with and without DRep delegations, pool-operator flag set, varied rewards.

- [ ] **Step 1: Write fixture**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/tests/fixtures/stake_state_small.json`

```json
{
  "stake_entries": [
    {
      "stake_credential_hash": "11000000000000000000000000000000000000000000000000000000",
      "delegated_pool": "00000000000000000000000000000000000000000000000000000000",
      "delegated_drep": "00000000000000000000000000000000000000000000000000000000",
      "rewards_lovelace": 0,
      "is_pool_operator": 0
    },
    {
      "stake_credential_hash": "22000000000000000000000000000000000000000000000000000000",
      "delegated_pool": "aa000000000000000000000000000000000000000000000000000000",
      "delegated_drep": "00000000000000000000000000000000000000000000000000000000",
      "rewards_lovelace": 1000000,
      "is_pool_operator": 0
    },
    {
      "stake_credential_hash": "33000000000000000000000000000000000000000000000000000000",
      "delegated_pool": "aa000000000000000000000000000000000000000000000000000000",
      "delegated_drep": "bb000000000000000000000000000000000000000000000000000000",
      "rewards_lovelace": 5000000,
      "is_pool_operator": 0
    },
    {
      "stake_credential_hash": "44000000000000000000000000000000000000000000000000000000",
      "delegated_pool": "cc000000000000000000000000000000000000000000000000000000",
      "delegated_drep": "bb000000000000000000000000000000000000000000000000000000",
      "rewards_lovelace": 25000000,
      "is_pool_operator": 0
    },
    {
      "stake_credential_hash": "55000000000000000000000000000000000000000000000000000000",
      "delegated_pool": "dd000000000000000000000000000000000000000000000000000000",
      "delegated_drep": "ee000000000000000000000000000000000000000000000000000000",
      "rewards_lovelace": 100000000,
      "is_pool_operator": 1
    },
    {
      "stake_credential_hash": "66000000000000000000000000000000000000000000000000000000",
      "delegated_pool": "00000000000000000000000000000000000000000000000000000000",
      "delegated_drep": "ff000000000000000000000000000000000000000000000000000000",
      "rewards_lovelace": 0,
      "is_pool_operator": 0
    },
    {
      "stake_credential_hash": "77000000000000000000000000000000000000000000000000000000",
      "delegated_pool": "aa000000000000000000000000000000000000000000000000000000",
      "delegated_drep": "ff000000000000000000000000000000000000000000000000000000",
      "rewards_lovelace": 18446744073709551614,
      "is_pool_operator": 0
    },
    {
      "stake_credential_hash": "88000000000000000000000000000000000000000000000000000000",
      "delegated_pool": "cc000000000000000000000000000000000000000000000000000000",
      "delegated_drep": "00000000000000000000000000000000000000000000000000000000",
      "rewards_lovelace": 12345678,
      "is_pool_operator": 1
    }
  ]
}
```

The 7th entry has rewards = `u64::MAX - 1` to stress the upper bound of the u64 path through serde.

- [ ] **Step 2: Write integration test**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/tests/stake_state_integration.rs`

```rust
//! End-to-end integration test for the stake-state sub-tree.

use omega_commitment_core::{
    stake_state_leaf::{validate_stake_credential_uniqueness, StakeEntry},
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    stake_entries: Vec<StakeEntry>,
}

const FIXTURE: &str = include_str!("fixtures/stake_state_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.stake_entries.len(), 8);

    assert!(
        validate_stake_credential_uniqueness(&f.stake_entries).is_none(),
        "fixture has duplicate stake_credential_hashes"
    );

    let leaves: Vec<_> = f.stake_entries.iter().map(|s| s.leaf_hash()).collect();
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
    let leaves1: Vec<_> = f.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    let leaves2: Vec<_> = f.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    assert_eq!(
        MerkleTree::build(leaves1).root(),
        MerkleTree::build(leaves2).root()
    );
}

#[test]
fn pool_operator_flag_changes_leaf() {
    // Sanity: verify a pool-operator entry hashes differently than the
    // same entry with the flag flipped (consistency with leaf-encoding tests).
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let op_entries: Vec<_> = f.stake_entries.iter()
        .filter(|s| s.is_pool_operator == 1)
        .collect();
    assert!(!op_entries.is_empty(), "fixture should include pool operators");
    for entry in op_entries {
        let mut flipped = entry.clone();
        flipped.is_pool_operator = 0;
        assert_ne!(entry.leaf_hash(), flipped.leaf_hash());
    }
}

#[test]
fn duplicate_stake_credential_rejected_by_validator() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let mut entries = f.stake_entries;
    let dup = entries[0].clone();
    entries.push(dup);
    assert_eq!(validate_stake_credential_uniqueness(&entries), Some(8));
}
```

- [ ] **Step 3: Verify**

```bash
cargo test -p omega-commitment-core --test stake_state_integration 2>&1 | tail -10   # 4 tests pass
cargo test --workspace 2>&1 | tail -5    # 123 total
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-core/tests/stake_state_integration.rs \
        crates/omega-commitment-core/tests/fixtures/stake_state_small.json
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test: stake-state sub-tree integration test against synthetic 8-entry fixture"
```

---

## Task 3: CLI `stake` arm

**Files:**
- Modify: `crates/omega-commitment-cli/src/main.rs`

Add a `Stake` variant + `StakeInput` struct + `build_stake_leaves` function + match arm. (The governance arm comes in Task 6.)

- [ ] **Step 1: Update `SubTree` enum**

In `/home/hoskinson/omega-commitment/crates/omega-commitment-cli/src/main.rs`, replace the `SubTree` enum with:

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
    Stake,
}
```

- [ ] **Step 2: Update import block**

Add `stake_state_leaf::StakeEntry` to the import:

```rust
use omega_commitment_core::{
    hash::{blake2b_256, Hash},
    header_leaf::BlockHeader,
    script_registry_leaf::ScriptEntry,
    stake_state_leaf::StakeEntry,
    token_policy_leaf::TokenPolicy,
    tree::MerkleTree,
    tx_index_leaf::TxIndexEntry,
    utxo_leaf::Utxo,
    witness::InclusionWitness,
};
```

- [ ] **Step 3: Add `StakeInput` struct**

After the existing `ScriptInput` struct, add:

```rust
#[derive(Deserialize)]
struct StakeInput {
    stake_entries: Vec<StakeEntry>,
}
```

- [ ] **Step 4: Add `build_stake_leaves` free function**

After `build_script_leaves`, add:

```rust
fn build_stake_leaves(raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    let parsed: StakeInput = serde_json::from_str(raw)?;
    let leaves: Vec<Hash> = parsed.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    let n = parsed.stake_entries.len();
    Ok((leaves, n))
}
```

- [ ] **Step 5: Add the dispatcher arm**

```rust
    let (leaves, item_count) = match sub_tree {
        SubTree::Utxo => build_utxo_leaves(&raw)?,
        SubTree::Header => build_header_leaves(&raw)?,
        SubTree::TxIndex => build_tx_index_leaves(&raw)?,
        SubTree::TokenPolicy => build_token_policy_leaves(&raw)?,
        SubTree::Script => build_script_leaves(&raw)?,
        SubTree::Stake => build_stake_leaves(&raw)?,
    };
```

- [ ] **Step 6: Build and run smoke test for the new arm**

```bash
cargo build --release -p omega-commitment-cli 2>&1 | tail -3
mkdir -p /tmp/o-st && rm -rf /tmp/o-st/*
./target/release/omega-commitment commit --sub-tree stake \
  --input crates/omega-commitment-core/tests/fixtures/stake_state_small.json \
  --output /tmp/o-st
cat /tmp/o-st/commitment.json
ls /tmp/o-st/witnesses/ | wc -l   # 8
```

Expected: `"sub_tree": "stake"`, `"item_count": 8`, 8 witness files.

- [ ] **Step 7: Run all tests + lint + fmt**

```bash
cargo test --workspace 2>&1 | tail -5    # 123 still pass
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

- [ ] **Step 8: Commit**

```bash
git add crates/omega-commitment-cli/src/main.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(cli): add stake arm to --sub-tree dispatcher"
```

---

## Task 4: `governance_state_leaf.rs` — canonical governance-state encoding

**Files:**
- Create: `crates/omega-commitment-core/src/governance_state_leaf.rs`
- Modify: `crates/omega-commitment-core/src/lib.rs`

Heterogeneous facts of four kinds (treasury, CC seat, ratified gov action, in-flight gov action) packed into a uniform 57-byte leaf.

- [ ] **Step 1: Create the module**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/src/governance_state_leaf.rs`

```rust
//! Canonical governance-state leaf encoding.
//!
//! A governance-state leaf is the deterministic serialization of:
//!   (kind: u8) || (key: 32 bytes) || (value: u128 BE) || (slot: u64 BE)
//!
//! Total: 57 bytes. The leaf is hashed with Blake2b-256 to produce
//! the leaf hash that goes into the Merkle tree. This sub-tree powers
//! `claim_governance` transactions: users port over treasury, CC seat,
//! and governance-action history.
//!
//! ## Heterogeneity vs. uniformity
//!
//! Governance state is intrinsically heterogeneous (treasury balance,
//! CC seats, gov-action records). Rather than building seven inner
//! trees, we commit to one tree of "governance facts" where each fact
//! has a `kind` discriminant and a fixed-width payload. This keeps
//! the encoding canonical and Plonky3-friendly.
//!
//! ## Kind discriminants
//!
//! - `0` — Treasury balance. `key` = all-zero. `value` = lovelace balance.
//! - `1` — CC seat. `key` = member's credential hash (right-padded from
//!   28 bytes to 32 with zeros). `value` = expiration epoch.
//! - `2` — Ratified gov action. `key` = action's tx_id (full 32 bytes).
//!   `value` = packed `(action_type:u16 << 0) | (slot_ratified:u64 << 16)`;
//!   top 48 bits reserved.
//! - `3` — In-flight gov action. `key` = action's tx_id. `value` = packed
//!   `(action_type:u16 << 0) | (slot_submitted:u64 << 16)`; top 48 bits
//!   reserved.
//! - Future variants reserved.
//!
//! A verifier reading a leaf's preimage MUST consult `kind` to interpret
//! `key` and `value`; the encoding does not self-describe beyond that.

use crate::hash::{blake2b_256, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct GovernanceFact {
    pub kind: u8,
    #[serde(with = "hex::serde")]
    pub key: [u8; 32],
    pub value: u128,
    pub slot: u64,
}

impl GovernanceFact {
    /// Canonical 57-byte serialization.
    pub fn encode(&self) -> [u8; 57] {
        let mut out = [0u8; 57];
        out[0] = self.kind;
        out[1..33].copy_from_slice(&self.key);
        out[33..49].copy_from_slice(&self.value.to_be_bytes());
        out[49..57].copy_from_slice(&self.slot.to_be_bytes());
        out
    }

    /// Compute the leaf hash: Blake2b-256 of canonical encoding.
    pub fn leaf_hash(&self) -> Hash {
        blake2b_256(&self.encode())
    }
}

/// Validate that no `(kind, key)` pair appears more than once across
/// the entries. Returns the index of the second occurrence of the
/// first duplicate, or None if all `(kind, key)` pairs are unique.
///
/// The same `key` is allowed across different `kind`s (e.g., a
/// gov-action tx_id could appear as both ratified and in-flight in
/// theory, though rare in practice). Optional sanity helper;
/// commitment generation does NOT require uniqueness.
pub fn validate_governance_keys_unique_per_kind(entries: &[GovernanceFact]) -> Option<usize> {
    let mut seen: HashSet<(u8, [u8; 32])> = HashSet::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        if !seen.insert((e.kind, e.key)) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(kind: u8, key_byte: u8, value: u128, slot: u64) -> GovernanceFact {
        GovernanceFact {
            kind,
            key: [key_byte; 32],
            value,
            slot,
        }
    }

    #[test]
    fn encoding_is_exactly_57_bytes() {
        let f = fact(0, 0, 1_000_000, 100);
        assert_eq!(f.encode().len(), 57);
    }

    #[test]
    fn encoding_layout_is_correct() {
        let f = GovernanceFact {
            kind: 0x07,
            key: [0xAAu8; 32],
            value: 0x1112131415161718_2122232425262728,
            slot: 0x3132333435363738,
        };
        let bytes = f.encode();
        assert_eq!(bytes[0], 0x07);
        assert_eq!(&bytes[1..33], &[0xAAu8; 32]);
        assert_eq!(
            &bytes[33..49],
            &[
                0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
                0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28,
            ]
        );
        assert_eq!(
            &bytes[49..57],
            &[0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38]
        );
    }

    #[test]
    fn leaf_hash_is_32_bytes() {
        let f = fact(0, 0, 0, 0);
        assert_eq!(f.leaf_hash().len(), 32);
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let f = fact(1, 0x11, 100, 200);
        assert_eq!(f.leaf_hash(), f.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_kind_change() {
        let a = fact(0, 0x11, 100, 200);
        let b = fact(1, 0x11, 100, 200);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_key_change() {
        let a = fact(0, 0x11, 100, 200);
        let b = fact(0, 0x12, 100, 200);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_value_change() {
        let a = fact(0, 0x11, 100, 200);
        let b = fact(0, 0x11, 101, 200);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_slot_change() {
        let a = fact(0, 0x11, 100, 200);
        let b = fact(0, 0x11, 100, 201);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn all_four_kinds_distinct_leaves() {
        let leaves: Vec<Hash> = (0..=3u8)
            .map(|k| fact(k, 0x11, 100, 200).leaf_hash())
            .collect();
        for i in 0..leaves.len() {
            for j in (i + 1)..leaves.len() {
                assert_ne!(leaves[i], leaves[j], "kind {} vs {} collided", i, j);
            }
        }
    }

    #[test]
    fn future_kind_byte_round_trips() {
        // kind=255 (reserved) must encode and hash without panic.
        let f = fact(255, 0, 0, 0);
        assert_eq!(f.encode()[0], 255);
        let _ = f.leaf_hash();
    }

    #[test]
    fn u128_max_value_encodes_correctly() {
        let f = fact(0, 0, u128::MAX, 0);
        let bytes = f.encode();
        assert_eq!(&bytes[33..49], &[0xFFu8; 16]);
    }

    #[test]
    fn validate_keys_unique_per_kind_accepts_unique() {
        let entries = vec![
            fact(0, 0x01, 100, 200),
            fact(1, 0x02, 100, 200),
            fact(2, 0x03, 100, 200),
        ];
        assert_eq!(validate_governance_keys_unique_per_kind(&entries), None);
    }

    #[test]
    fn validate_keys_unique_per_kind_finds_duplicate() {
        let entries = vec![
            fact(0, 0x01, 100, 200),
            fact(1, 0x01, 200, 300),
            fact(0, 0x01, 999, 999),  // duplicate (kind=0, key=0x01)
        ];
        assert_eq!(validate_governance_keys_unique_per_kind(&entries), Some(2));
    }

    #[test]
    fn validate_keys_unique_per_kind_allows_same_key_different_kind() {
        // Same key 0x01 used for kind=0 (treasury) and kind=2 (ratified
        // gov action) — should be allowed.
        let entries = vec![
            fact(0, 0x01, 100, 200),
            fact(2, 0x01, 200, 300),
        ];
        assert_eq!(validate_governance_keys_unique_per_kind(&entries), None);
    }

    #[test]
    fn validate_keys_unique_per_kind_empty_is_valid() {
        assert_eq!(validate_governance_keys_unique_per_kind(&[]), None);
    }
}
```

- [ ] **Step 2: Update `lib.rs`** — add `pub mod governance_state_leaf;`

Replace the contents:

```rust
//! omega-commitment-core: Ω-Commitment sub-tree library.
//!
//! Provides canonical leaf encodings, a Plonky3-friendly Merkle tree, and
//! inclusion witnesses. v0.6.0 supports all seven Ω-Commitment sub-trees:
//! UTXO set, block header chain, transaction index, native token policies,
//! script registry, stake state, and governance state.

pub mod hash;
pub mod serde_helpers;
pub mod tree;
pub mod witness;
pub mod utxo_leaf;
pub mod header_leaf;
pub mod tx_index_leaf;
pub mod token_policy_leaf;
pub mod script_registry_leaf;
pub mod stake_state_leaf;
pub mod governance_state_leaf;
```

- [ ] **Step 3: Verify**

```bash
cargo test -p omega-commitment-core governance_state_leaf::tests 2>&1 | tail -25
cargo test --workspace 2>&1 | tail -5    # 138 total (123 prior + 15 new)
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-core/src/governance_state_leaf.rs \
        crates/omega-commitment-core/src/lib.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(governance_state_leaf): canonical 57-byte fact encoding + per-kind uniqueness"
```

---

## Task 5: Governance-state integration test

**Files:**
- Create: `crates/omega-commitment-core/tests/fixtures/governance_state_small.json`
- Create: `crates/omega-commitment-core/tests/governance_state_integration.rs`

8-entry fixture with all 4 `kind` values represented, plus a stress test entry with `value = u128::MAX - 1`.

- [ ] **Step 1: Write fixture**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/tests/fixtures/governance_state_small.json`

```json
{
  "facts": [
    {
      "kind": 0,
      "key": "0000000000000000000000000000000000000000000000000000000000000000",
      "value": 1700000000000000,
      "slot": 100000
    },
    {
      "kind": 1,
      "key": "11000000000000000000000000000000000000000000000000000000aabbccdd",
      "value": 500,
      "slot": 100000
    },
    {
      "kind": 1,
      "key": "22000000000000000000000000000000000000000000000000000000aabbccdd",
      "value": 502,
      "slot": 100000
    },
    {
      "kind": 1,
      "key": "33000000000000000000000000000000000000000000000000000000aabbccdd",
      "value": 510,
      "slot": 100000
    },
    {
      "kind": 2,
      "key": "44440000000000000000000000000000000000000000000000000000000000aa",
      "value": 5497558138886,
      "slot": 100000
    },
    {
      "kind": 2,
      "key": "55550000000000000000000000000000000000000000000000000000000000bb",
      "value": 6597069766657,
      "slot": 100000
    },
    {
      "kind": 3,
      "key": "66660000000000000000000000000000000000000000000000000000000000cc",
      "value": 6597069766658,
      "slot": 100000
    },
    {
      "kind": 3,
      "key": "77770000000000000000000000000000000000000000000000000000000000dd",
      "value": 340282366920938463463374607431768211454,
      "slot": 100000
    }
  ]
}
```

Notes:
- 1× treasury (kind=0)
- 3× CC seats (kind=1)
- 2× ratified gov actions (kind=2)
- 2× in-flight gov actions (kind=3)
- Last entry has `value = u128::MAX - 1` for serde stress
- All `(kind, key)` pairs unique

- [ ] **Step 2: Write integration test**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/tests/governance_state_integration.rs`

```rust
//! End-to-end integration test for the governance-state sub-tree.

use omega_commitment_core::{
    governance_state_leaf::{validate_governance_keys_unique_per_kind, GovernanceFact},
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    facts: Vec<GovernanceFact>,
}

const FIXTURE: &str = include_str!("fixtures/governance_state_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.facts.len(), 8);

    assert!(
        validate_governance_keys_unique_per_kind(&f.facts).is_none(),
        "fixture has duplicate (kind, key) pairs"
    );

    let leaves: Vec<_> = f.facts.iter().map(|fact| fact.leaf_hash()).collect();
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
    let leaves1: Vec<_> = f.facts.iter().map(|fact| fact.leaf_hash()).collect();
    let leaves2: Vec<_> = f.facts.iter().map(|fact| fact.leaf_hash()).collect();
    assert_eq!(
        MerkleTree::build(leaves1).root(),
        MerkleTree::build(leaves2).root()
    );
}

#[test]
fn all_four_kinds_present_in_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let kinds: std::collections::HashSet<u8> = f.facts.iter().map(|x| x.kind).collect();
    assert_eq!(kinds.len(), 4, "expected all 4 kinds");
    for expected in 0..=3u8 {
        assert!(kinds.contains(&expected), "missing kind={expected}");
    }
}

#[test]
fn large_u128_value_round_trips() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let last = f.facts.last().unwrap();
    assert_eq!(last.value, u128::MAX - 1);
}

#[test]
fn duplicate_kind_key_pair_rejected_by_validator() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let mut facts = f.facts;
    let dup = facts[0].clone();
    facts.push(dup);
    assert_eq!(validate_governance_keys_unique_per_kind(&facts), Some(8));
}
```

- [ ] **Step 3: Verify**

```bash
cargo test -p omega-commitment-core --test governance_state_integration 2>&1 | tail -10   # 5 tests pass
cargo test --workspace 2>&1 | tail -5    # 143 total
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-core/tests/governance_state_integration.rs \
        crates/omega-commitment-core/tests/fixtures/governance_state_small.json
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test: governance-state sub-tree integration test against synthetic 8-fact fixture"
```

---

## Task 6: CLI `governance` arm

**Files:**
- Modify: `crates/omega-commitment-cli/src/main.rs`

Add `Governance` variant + `GovernanceInput` struct + `build_governance_leaves` function + match arm.

- [ ] **Step 1: Update `SubTree` enum**

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
    Stake,
    Governance,
}
```

- [ ] **Step 2: Update import block**

Add `governance_state_leaf::GovernanceFact`:

```rust
use omega_commitment_core::{
    governance_state_leaf::GovernanceFact,
    hash::{blake2b_256, Hash},
    header_leaf::BlockHeader,
    script_registry_leaf::ScriptEntry,
    stake_state_leaf::StakeEntry,
    token_policy_leaf::TokenPolicy,
    tree::MerkleTree,
    tx_index_leaf::TxIndexEntry,
    utxo_leaf::Utxo,
    witness::InclusionWitness,
};
```

- [ ] **Step 3: Add `GovernanceInput` struct**

After `StakeInput`:

```rust
#[derive(Deserialize)]
struct GovernanceInput {
    facts: Vec<GovernanceFact>,
}
```

- [ ] **Step 4: Add `build_governance_leaves` function**

After `build_stake_leaves`:

```rust
fn build_governance_leaves(raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    let parsed: GovernanceInput = serde_json::from_str(raw)?;
    let leaves: Vec<Hash> = parsed.facts.iter().map(|f| f.leaf_hash()).collect();
    let n = parsed.facts.len();
    Ok((leaves, n))
}
```

- [ ] **Step 5: Add the dispatcher arm**

```rust
    let (leaves, item_count) = match sub_tree {
        SubTree::Utxo => build_utxo_leaves(&raw)?,
        SubTree::Header => build_header_leaves(&raw)?,
        SubTree::TxIndex => build_tx_index_leaves(&raw)?,
        SubTree::TokenPolicy => build_token_policy_leaves(&raw)?,
        SubTree::Script => build_script_leaves(&raw)?,
        SubTree::Stake => build_stake_leaves(&raw)?,
        SubTree::Governance => build_governance_leaves(&raw)?,
    };
```

- [ ] **Step 6: Build and run smoke for governance**

```bash
cargo build --release -p omega-commitment-cli 2>&1 | tail -3
mkdir -p /tmp/o-gv && rm -rf /tmp/o-gv/*
./target/release/omega-commitment commit --sub-tree governance \
  --input crates/omega-commitment-core/tests/fixtures/governance_state_small.json \
  --output /tmp/o-gv
cat /tmp/o-gv/commitment.json
ls /tmp/o-gv/witnesses/ | wc -l   # 8
```

Expected: `"sub_tree": "governance"`, `"item_count": 8`, 8 witness files.

- [ ] **Step 7: Run all tests + lint + fmt**

```bash
cargo test --workspace 2>&1 | tail -5    # 143 still pass
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

- [ ] **Step 8: Commit**

```bash
git add crates/omega-commitment-cli/src/main.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(cli): add governance arm to --sub-tree dispatcher"
```

---

## Task 7: CLI smoke tests for stake + governance

**Files:**
- Modify: `crates/omega-commitment-cli/tests/cli.rs`

Two new smoke tests parallel to the existing five.

- [ ] **Step 1: Append to `crates/omega-commitment-cli/tests/cli.rs`**

```rust

#[test]
fn cli_commit_stake_smoke() {
    let out = run_commit("stake", "stake_state_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(
        body.contains("\"sub_tree\": \"stake\""),
        "wrong sub_tree tag: {body}"
    );
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 8"));
    let witness_count = fs::read_dir(out.path().join("witnesses")).unwrap().count();
    assert_eq!(witness_count, 8, "expected 8 stake witness files");
}

#[test]
fn cli_commit_governance_smoke() {
    let out = run_commit("governance", "governance_state_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(
        body.contains("\"sub_tree\": \"governance\""),
        "wrong sub_tree tag: {body}"
    );
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 8"));
    let witness_count = fs::read_dir(out.path().join("witnesses")).unwrap().count();
    assert_eq!(witness_count, 8, "expected 8 governance witness files");
}
```

- [ ] **Step 2: Run**

```bash
cargo test -p omega-commitment-cli --test cli 2>&1 | tail -10   # 11 tests pass (9 prior + 2 new)
cargo test --workspace 2>&1 | tail -5                            # 145 total
cargo lint 2>&1 | tail -3                                          # clean
cargo fmt-check 2>&1 | tail -3                                     # clean
```

- [ ] **Step 3: Commit**

```bash
git add crates/omega-commitment-cli/tests/cli.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test(cli): stake and governance smoke tests"
```

---

## Task 8: Bump to v0.6.0 + extend README (track T1 leaf-tooling phase complete)

**Files:**
- Modify: `crates/omega-commitment-core/Cargo.toml`
- Modify: `crates/omega-commitment-cli/Cargo.toml`
- Modify: `README.md`

- [ ] **Step 1: Bump versions**

Both `Cargo.toml` files: `version = "0.5.0"` → `version = "0.6.0"`.

- [ ] **Step 2: Verify**

```bash
cargo build --workspace 2>&1 | grep -E "warning|error"   # empty
cargo lint 2>&1 | tail -3                                  # clean
cargo fmt-check 2>&1 | tail -3                             # clean
cargo test --workspace 2>&1 | tail -5                       # 145 tests pass
```

- [ ] **Step 3: Append to README.md**

```markdown
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
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-core/Cargo.toml \
        crates/omega-commitment-cli/Cargo.toml \
        README.md
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "chore: bump to 0.6.0; document stake+governance sub-trees and T1 leaf-tooling completion"
```

- [ ] **Step 5: Final verification**

```bash
git log --oneline | head -10
cargo test --workspace 2>&1 | tail -5
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

Expected: HEAD is the version-bump commit, 145 tests pass, lint+fmt clean.

---

## Self-review

**Spec coverage** (from `2026-05-01-ouroboros-omega-design.md` §7, sub-trees 6 and 7):

Sub-tree 6 (stake state):
> "Stake state — Merkle tree of (delegation, pool registration, DRep registration) tuples at H."
- ✅ Each `StakeEntry` row carries delegation (`delegated_pool`), pool-operator flag (`is_pool_operator`), and DRep registration (`delegated_drep`). Plus `rewards_lovelace` for completeness. Per-stake-credential keying matches the spec's "tuples at H" framing.
- ✅ Plonky3-friendly fixed-width encoding.
- ✅ `claim_stake` (spec §9.5) gets a tree to prove against.

Sub-tree 7 (governance state):
> "Governance state — Merkle tree of (treasury balance, DRep IDs, CC member positions, past gov action history, ratified proposals) at H."
- ✅ Heterogeneous facts represented as a `(kind, key, value, slot)` tuple — covers treasury balance, CC seats, ratified gov actions, and in-flight gov actions.
- 🟡 The spec also mentions "DRep IDs" in this sub-tree. In the current design, DRep registration is captured in the **stake-state** sub-tree (via `delegated_drep` per credential, plus a stake credential being a registered DRep is implicitly findable by joining stake-state and governance-state). Adding a 5th `kind` for "DRep registration" is straightforward if reviewers prefer; flagging here as a small open interpretation question, not a defect. A future minor release can add `kind=4` without breaking anything (encoding is fixed-width and validators allow any u8 value).
- ✅ `claim_governance` (spec §9.6) gets a tree to prove against.

**Decision honoring:**
- ✅ Decision 7 (PQ-only): Blake2b for leaf hashing throughout.
- ✅ Decision 8 (Plonky3-friendly): fixed-width 93-byte (stake) and 57-byte (governance) leaf encodings, sort-then-pad tree.
- ✅ Decision 3 (everything-provable): adds 6th and 7th of 7 sub-trees → **leaf-tooling phase of T1 complete.**
- ✅ Decision 9 (selective dual-track): per-sub-tree tooling stays Blake2b-only.

**Placeholder scan:** All code blocks runnable. ✅

**Type consistency:**
- `CredentialHash = [u8; 28]` defined in `stake_state_leaf.rs`; used uniformly in struct fields and validator.
- `StakeEntry` and `GovernanceFact` referenced consistently across module / fixture / integration / CLI input.
- `validate_stake_credential_uniqueness` and `validate_governance_keys_unique_per_kind` signatures consistent.
- `SubTree::Stake` → `"stake"`, `SubTree::Governance` → `"governance"` via kebab-case rule, both matching CLI flag spellings and smoke-test substring assertions.
- `build_stake_leaves` and `build_governance_leaves` follow the established naming pattern.
- ✅ No drift.

**Bite-sized tasks:** 8 tasks, each independently committable, each with 3–8 numbered steps. ✅

**Net delta:** +13 unit tests (stake) + 4 integration (stake) + 15 unit (governance) + 5 integration (governance) + 2 CLI smoke = **+39 tests** (106 → 145). 8 commits.

---

## What's NOT in this plan (and why)

- **Real Cardano mainnet ingestion** — synthetic fixtures only. `cardano-multiplatform-lib` integration deferred.
- **Cross-sub-tree validation** (e.g., "every CC seat in governance-state corresponds to a known credential in stake-state") — requires loading multiple commitments simultaneously; deferred to a future cross-validation plan.
- **DRep-registration kind in governance-state** — open interpretation of the spec; can be added as `kind=4` in a minor release without breaking anything.
- **Bundle-assembly tool** — separate plan, comes next per the dual-hash decision.
- **Plonky3 `claim_stake` and `claim_governance` circuits** — track T2.

---

## How to execute this plan

Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Eight tasks, each independently committable.

Total runway estimate: **3–5 days** for an experienced Rust dev. The architecture is fully mature; this plan is two more applications of the established sub-tree pattern, plus a CLI release-note update marking T1 leaf-tooling as complete.

Expected post-execution state:
- 8 commits added on top of v0.5.0 (currently 49 commits)
- ~39 net new tests (145 total)
- Both crates at version 0.6.0
- **All 7 of 7 sub-trees shipped** — leaf-tooling phase of track T1 complete.

Next plan in this track: bundle-assembly tool (aggregates the 7 Blake2b sub-tree roots + 7 SHA3 sub-tree roots into the canonical Ω-Commitment tuple).
