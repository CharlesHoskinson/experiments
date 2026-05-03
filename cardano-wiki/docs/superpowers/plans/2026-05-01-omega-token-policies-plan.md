# Omega Native Token Policies Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the native token policy sub-tree (sub-tree 4 of 7) so token issuers can re-anchor a minting policy on the new chain with verifiable lineage. Powers `claim_token_policy` transactions per spec §9.2.

**Architecture:** Reuse `tree.rs`, `witness.rs`, `serde_helpers` unchanged (now validated across three prior sub-trees). Add a new `token_policy_leaf.rs` with a fixed-width 52-byte canonical encoding (policy_id ‖ first_issuance_slot ‖ total_supply_at_h). CLI gains a `token-policy` arm. Bump to v0.4.0.

**Tech Stack:** Rust 1.79+, blake2, sha3, serde, clap (no new deps).

**Track:** T1 (Ω-Commitment Tooling), sub-tree 4. See `2026-05-01-ouroboros-omega-program-roadmap.md`.

**Locked design decisions honored (unchanged):**
- Decision 7 (PQ-only crypto): Blake2b-256 leaf hash. The 28-byte `policy_id` is Cardano-native Blake2b-224 — outside our crypto scope, just a 28-byte handle.
- Decision 8 (Plonky3-friendly): same MerkleTree (binary, fixed-arity, sorted-padded). Token-policy leaves sort by leaf hash; verifiers reconstruct from preimage.
- Decision 3 (everything-provable): adding sub-tree 4 of 7. Three remaining: script registry, stake state, governance state.

**First cross-sub-tree asymmetry — documented explicitly:**
- Sub-trees 1–3 used 32-byte hashes for all hash-typed fields.
- Sub-tree 4's `policy_id` is **28 bytes** (Blake2b-224, the canonical Cardano policy-hash size). This is NOT a typo and NOT padded to 32. Verifiers must encode policies as 28-byte values to compute leaf hashes consistent with on-chain Cardano semantics.
- The leaf hash itself is still Blake2b-256 → 32 bytes; only the *preimage* contains a 28-byte field.

**Carry-over from v0.3.1 final state:**
- Dual-track shadow hash decision STILL deferred. v0.4.0 continues to publish only the Blake2b root. Plonky3 circuit authors must NOT lock to v0.4.0 single-root format until decided.
- `SubTree` enum gets `#[non_exhaustive]` in this plan (good moment, since we're adding a new variant).

---

## File structure (post-plan)

```
omega-commitment/
├── Cargo.toml                                           (workspace)
├── README.md                                            (extended: token-policy section, breaking-changes note, sub-trees status table)
├── crates/
│   ├── omega-commitment-core/
│   │   ├── Cargo.toml                                   (version bump to 0.4.0)
│   │   ├── src/
│   │   │   ├── lib.rs                                   (add `pub mod token_policy_leaf`)
│   │   │   ├── hash.rs                                  (unchanged)
│   │   │   ├── serde_helpers.rs                         (unchanged)
│   │   │   ├── utxo_leaf.rs                             (unchanged)
│   │   │   ├── header_leaf.rs                           (unchanged)
│   │   │   ├── tx_index_leaf.rs                         (unchanged)
│   │   │   ├── token_policy_leaf.rs                     (NEW)
│   │   │   ├── tree.rs                                  (unchanged)
│   │   │   └── witness.rs                               (unchanged)
│   │   ├── tests/
│   │   │   ├── fixtures/
│   │   │   │   ├── utxo_set_small.json                  (existing)
│   │   │   │   ├── header_chain_small.json              (existing)
│   │   │   │   ├── tx_index_small.json                  (existing)
│   │   │   │   └── token_policies_small.json            (NEW)
│   │   │   ├── utxo_integration.rs                      (existing)
│   │   │   ├── header_integration.rs                    (existing)
│   │   │   ├── tx_index_integration.rs                  (existing)
│   │   │   └── token_policy_integration.rs              (NEW)
│   │   └── benches/tree.rs                              (unchanged)
│   └── omega-commitment-cli/
│       ├── Cargo.toml                                   (version bump to 0.4.0)
│       ├── src/main.rs                                  (modify: add TokenPolicy arm + non_exhaustive + free function)
│       └── tests/cli.rs                                 (extend with token-policy smoke test)
```

Each file has one clear responsibility:
- `token_policy_leaf.rs`: token-policy-specific canonical encoding (NEW).
- `tree.rs`, `witness.rs`, `hash.rs`, `serde_helpers.rs`: sub-tree agnostic, no changes.
- `lib.rs`: re-exports the new module.
- CLI `main.rs`: gains one variant + one free function + one match arm + `#[non_exhaustive]`.

---

## Task 1: `token_policy_leaf.rs` — canonical token policy encoding

**Files:**
- Create: `crates/omega-commitment-core/src/token_policy_leaf.rs`
- Modify: `crates/omega-commitment-core/src/lib.rs`

A token policy leaf is the deterministic serialization of:
```
policy_id (28 bytes) || first_issuance_slot (u64 BE) || total_supply_at_h (u128 BE)
```
Total: 52 bytes. Hashed with Blake2b-256 to produce the 32-byte leaf hash.

- [ ] **Step 1: Create `token_policy_leaf.rs`**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/src/token_policy_leaf.rs`

```rust
//! Canonical native-token policy leaf encoding.
//!
//! A token-policy leaf is the deterministic serialization of:
//!   (policy_id: 28 bytes) || (first_issuance_slot: u64 BE) ||
//!   (total_supply_at_h: u128 BE)
//!
//! Total: 52 bytes. The leaf is hashed with Blake2b-256 to produce
//! the leaf hash that goes into the Merkle tree. This sub-tree powers
//! `claim_token_policy` transactions: token issuers can re-anchor a
//! minting policy on the new chain with verifiable lineage.
//!
//! ## Note on policy_id width
//!
//! Cardano policy hashes are **28 bytes** (Blake2b-224 of the minting
//! script), not 32. This is the first cross-sub-tree asymmetry in the
//! Ω-Commitment library. The 28-byte size is canonical Cardano
//! ledger semantics; verifiers must encode policies as 28-byte values
//! to compute leaf hashes consistent with on-chain identifiers.
//!
//! Note that the leaf hash itself remains Blake2b-256 → 32 bytes;
//! only the preimage contains a 28-byte field.

use crate::hash::{blake2b_256, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 28-byte Cardano native-token policy hash (Blake2b-224 of the
/// minting script). Distinct from the 32-byte `Hash` type used for
/// internal Merkle hashing.
pub type PolicyId = [u8; 28];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TokenPolicy {
    #[serde(with = "hex::serde")]
    pub policy_id: PolicyId,
    pub first_issuance_slot: u64,
    pub total_supply_at_h: u128,
}

impl TokenPolicy {
    /// Canonical 52-byte serialization.
    pub fn encode(&self) -> [u8; 52] {
        let mut out = [0u8; 52];
        out[0..28].copy_from_slice(&self.policy_id);
        out[28..36].copy_from_slice(&self.first_issuance_slot.to_be_bytes());
        out[36..52].copy_from_slice(&self.total_supply_at_h.to_be_bytes());
        out
    }

    /// Compute the leaf hash: Blake2b-256 of canonical encoding.
    pub fn leaf_hash(&self) -> Hash {
        blake2b_256(&self.encode())
    }
}

/// Validate that no `policy_id` appears more than once across the
/// entries. Returns the index of the second occurrence of the first
/// duplicate found, or None if all `policy_id`s are unique.
///
/// Cardano policy hashes are deterministic functions of the minting
/// script and should be unique. Duplicate input is a data error
/// (e.g., overlapping epoch ranges). This is an OPTIONAL sanity helper;
/// commitment generation does NOT require uniqueness.
pub fn validate_policy_id_uniqueness(entries: &[TokenPolicy]) -> Option<usize> {
    let mut seen: HashSet<PolicyId> = HashSet::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        if !seen.insert(e.policy_id) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(byte: u8, slot: u64, supply: u128) -> TokenPolicy {
        TokenPolicy {
            policy_id: [byte; 28],
            first_issuance_slot: slot,
            total_supply_at_h: supply,
        }
    }

    #[test]
    fn encoding_is_exactly_52_bytes() {
        let p = sample(0x11, 100, 1_000_000);
        assert_eq!(p.encode().len(), 52);
    }

    #[test]
    fn encoding_layout_is_correct() {
        let p = TokenPolicy {
            policy_id: [0xAAu8; 28],
            first_issuance_slot: 0x0102030405060708,
            total_supply_at_h: 0x1112131415161718_2122232425262728,
        };
        let bytes = p.encode();
        assert_eq!(&bytes[0..28], &[0xAAu8; 28]);
        assert_eq!(
            &bytes[28..36],
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
        assert_eq!(
            &bytes[36..52],
            &[
                0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
                0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28,
            ]
        );
    }

    #[test]
    fn policy_id_is_28_bytes() {
        let p = sample(0x11, 100, 0);
        assert_eq!(p.policy_id.len(), 28);
    }

    #[test]
    fn leaf_hash_is_32_bytes() {
        let p = sample(0x11, 100, 0);
        let h = p.leaf_hash();
        assert_eq!(h.len(), 32);
    }

    #[test]
    fn leaf_hash_is_deterministic() {
        let p = sample(0x11, 100, 1000);
        assert_eq!(p.leaf_hash(), p.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_policy_id_change() {
        let a = sample(0x11, 100, 1000);
        let b = sample(0x12, 100, 1000);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_slot_change() {
        let a = sample(0x11, 100, 1000);
        let b = sample(0x11, 101, 1000);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn leaf_hash_differs_on_supply_change() {
        let a = sample(0x11, 100, 1000);
        let b = sample(0x11, 100, 1001);
        assert_ne!(a.leaf_hash(), b.leaf_hash());
    }

    #[test]
    fn supply_at_u128_max_encodes_correctly() {
        let p = TokenPolicy {
            policy_id: [0x11; 28],
            first_issuance_slot: 0,
            total_supply_at_h: u128::MAX,
        };
        let bytes = p.encode();
        assert_eq!(&bytes[36..52], &[0xFFu8; 16]);
    }

    #[test]
    fn validate_policy_id_uniqueness_accepts_unique() {
        let entries = vec![
            sample(0x01, 1, 100),
            sample(0x02, 2, 200),
            sample(0x03, 3, 300),
        ];
        assert_eq!(validate_policy_id_uniqueness(&entries), None);
    }

    #[test]
    fn validate_policy_id_uniqueness_finds_duplicate() {
        let entries = vec![
            sample(0x01, 1, 100),
            sample(0x02, 2, 200),
            sample(0x01, 5, 999),
        ];
        assert_eq!(validate_policy_id_uniqueness(&entries), Some(2));
    }

    #[test]
    fn validate_policy_id_uniqueness_empty_is_valid() {
        assert_eq!(validate_policy_id_uniqueness(&[]), None);
    }

    #[test]
    fn same_policy_id_different_slot_still_distinct_leaves() {
        let a = sample(0x11, 100, 1000);
        let b = sample(0x11, 200, 1000);
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
//! inclusion witnesses. v0.4.0 supports four of seven Ω-Commitment sub-trees:
//! UTXO set, block header chain, transaction index, and native token policies.

pub mod hash;
pub mod serde_helpers;
pub mod tree;
pub mod witness;
pub mod utxo_leaf;
pub mod header_leaf;
pub mod tx_index_leaf;
pub mod token_policy_leaf;
```

- [ ] **Step 3: Run tests**

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
cargo test -p omega-commitment-core token_policy_leaf::tests 2>&1 | tail -20
```

Expected: 13 tests pass.

- [ ] **Step 4: Run full workspace + lint**

```bash
cargo test --workspace 2>&1 | tail -5
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

Expected: 81 tests pass (68 prior + 13 new). Lint and fmt-check clean.

If lint or fmt-check produce diffs, fix inline before committing. The `cargo fmt --all` command will auto-fix formatting.

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-core/src/token_policy_leaf.rs \
        crates/omega-commitment-core/src/lib.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(token_policy_leaf): canonical 52-byte encoding + uniqueness validator"
```

---

## Task 2: Token-policy integration test

**Files:**
- Create: `crates/omega-commitment-core/tests/fixtures/token_policies_small.json`
- Create: `crates/omega-commitment-core/tests/token_policy_integration.rs`

8-entry synthetic fixture with varied policy IDs, slot ranges, and supply values (including a stablecoin-scale large supply and a one-shot NFT with supply=1).

- [ ] **Step 1: Write fixture**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/tests/fixtures/token_policies_small.json`

```json
{
  "policies": [
    {
      "policy_id": "11000000000000000000000000000000000000000000000000000000",
      "first_issuance_slot": 100,
      "total_supply_at_h": 1000000000
    },
    {
      "policy_id": "22000000000000000000000000000000000000000000000000000000",
      "first_issuance_slot": 200,
      "total_supply_at_h": 50000000
    },
    {
      "policy_id": "33000000000000000000000000000000000000000000000000000000",
      "first_issuance_slot": 350,
      "total_supply_at_h": 1
    },
    {
      "policy_id": "44000000000000000000000000000000000000000000000000000000",
      "first_issuance_slot": 400,
      "total_supply_at_h": 21000000
    },
    {
      "policy_id": "55000000000000000000000000000000000000000000000000000000",
      "first_issuance_slot": 500,
      "total_supply_at_h": 100000000000
    },
    {
      "policy_id": "66000000000000000000000000000000000000000000000000000000",
      "first_issuance_slot": 700,
      "total_supply_at_h": 1
    },
    {
      "policy_id": "77000000000000000000000000000000000000000000000000000000",
      "first_issuance_slot": 1000,
      "total_supply_at_h": 7000000000000
    },
    {
      "policy_id": "88000000000000000000000000000000000000000000000000000000",
      "first_issuance_slot": 1500,
      "total_supply_at_h": 999999999999999999999999999999999999
    }
  ]
}
```

Note: the last entry's `total_supply_at_h` is a near-`u128::MAX` value (39 nines) to stress-test the u128 path through serde.

- [ ] **Step 2: Write integration test**

Path: `/home/hoskinson/omega-commitment/crates/omega-commitment-core/tests/token_policy_integration.rs`

```rust
//! End-to-end integration test for the native-token-policy sub-tree.

use omega_commitment_core::{
    token_policy_leaf::{validate_policy_id_uniqueness, TokenPolicy},
    tree::MerkleTree,
    witness::InclusionWitness,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    policies: Vec<TokenPolicy>,
}

const FIXTURE: &str = include_str!("fixtures/token_policies_small.json");

#[test]
fn full_pipeline_against_fixture() {
    let f: Fixture = serde_json::from_str(FIXTURE).expect("fixture parses");
    assert_eq!(f.policies.len(), 8);

    assert!(
        validate_policy_id_uniqueness(&f.policies).is_none(),
        "fixture has duplicate policy_ids"
    );

    let leaves: Vec<_> = f.policies.iter().map(|p| p.leaf_hash()).collect();
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
    let leaves1: Vec<_> = f.policies.iter().map(|p| p.leaf_hash()).collect();
    let leaves2: Vec<_> = f.policies.iter().map(|p| p.leaf_hash()).collect();
    assert_eq!(
        MerkleTree::build(leaves1).root(),
        MerkleTree::build(leaves2).root()
    );
}

#[test]
fn duplicate_policy_id_rejected_by_validator() {
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let mut policies = f.policies;
    let dup = policies[0].clone();
    policies.push(dup);
    assert_eq!(validate_policy_id_uniqueness(&policies), Some(8));
}

#[test]
fn large_u128_supply_round_trips_through_json() {
    // The 8th fixture entry has a 39-digit supply value; this test
    // confirms serde_json correctly parses it through the u128 path
    // and the encoded bytes match the expected layout.
    let f: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let last = f.policies.last().unwrap();
    assert_eq!(last.total_supply_at_h, 999_999_999_999_999_999_999_999_999_999_999_999u128);
    let bytes = last.encode();
    assert_eq!(bytes.len(), 52);
}
```

- [ ] **Step 3: Run integration tests**

```bash
cargo test -p omega-commitment-core --test token_policy_integration 2>&1 | tail -10
```

Expected: 4 tests pass.

- [ ] **Step 4: Run full workspace + lint**

```bash
cargo test --workspace 2>&1 | tail -5    # 85 total
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-core/tests/token_policy_integration.rs \
        crates/omega-commitment-core/tests/fixtures/token_policies_small.json
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test: token-policy sub-tree integration test against synthetic 8-entry fixture"
```

---

## Task 3: CLI `token-policy` arm + `#[non_exhaustive]` on `SubTree`

**Files:**
- Modify: `crates/omega-commitment-cli/src/main.rs`

Add a fourth variant to `SubTree`, a fourth `Input` struct, a fourth free builder function, and a fourth match arm. Also mark `SubTree` `#[non_exhaustive]` since we're adding a variant — this protects against SemVer breaks in any future external callers (none today, but pre-emptive is cheaper than retrofit).

- [ ] **Step 1: Update `SubTree` enum and `commit()` dispatcher**

Edit `/home/hoskinson/omega-commitment/crates/omega-commitment-cli/src/main.rs`. Find the `SubTree` enum and replace with:

```rust
#[derive(Copy, Clone, Debug, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
enum SubTree {
    Utxo,
    Header,
    TxIndex,
    TokenPolicy,
}
```

- [ ] **Step 2: Add `TokenPolicyInput` struct**

After the existing `TxIndexInput` struct, add:

```rust
#[derive(Deserialize)]
struct TokenPolicyInput {
    policies: Vec<TokenPolicy>,
}
```

Make sure `TokenPolicy` is in the import block at the top:

```rust
use omega_commitment_core::{
    hash::{blake2b_256, Hash},
    header_leaf::BlockHeader,
    token_policy_leaf::TokenPolicy,
    tree::MerkleTree,
    tx_index_leaf::TxIndexEntry,
    utxo_leaf::Utxo,
    witness::InclusionWitness,
};
```

- [ ] **Step 3: Add `build_token_policy_leaves` free function**

After the existing `build_tx_index_leaves` function, add:

```rust
fn build_token_policy_leaves(raw: &str) -> anyhow::Result<(Vec<Hash>, usize)> {
    let parsed: TokenPolicyInput = serde_json::from_str(raw)?;
    let leaves: Vec<Hash> = parsed.policies.iter().map(|p| p.leaf_hash()).collect();
    let n = parsed.policies.len();
    Ok((leaves, n))
}
```

- [ ] **Step 4: Add the dispatcher arm**

Find the `match sub_tree { ... }` in `commit()`. Add the new arm:

```rust
    let (leaves, item_count) = match sub_tree {
        SubTree::Utxo => build_utxo_leaves(&raw)?,
        SubTree::Header => build_header_leaves(&raw)?,
        SubTree::TxIndex => build_tx_index_leaves(&raw)?,
        SubTree::TokenPolicy => build_token_policy_leaves(&raw)?,
    };
```

- [ ] **Step 5: Build and run all 4 sub-trees end-to-end**

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
cargo build --release -p omega-commitment-cli 2>&1 | tail -5
```

Token-policy smoke:
```bash
mkdir -p /tmp/o-tp && rm -rf /tmp/o-tp/*
./target/release/omega-commitment commit --sub-tree token-policy \
  --input crates/omega-commitment-core/tests/fixtures/token_policies_small.json \
  --output /tmp/o-tp
cat /tmp/o-tp/commitment.json
ls /tmp/o-tp/witnesses/ | wc -l   # expect 8
```

Expected: `"sub_tree": "token-policy"`, `"item_count": 8`, 8 witness files.

Sanity: re-run other sub-trees to confirm no regression.
```bash
mkdir -p /tmp/o-u && rm -rf /tmp/o-u/*
./target/release/omega-commitment commit --sub-tree utxo \
  --input crates/omega-commitment-core/tests/fixtures/utxo_set_small.json \
  --output /tmp/o-u
cat /tmp/o-u/commitment.json   # still works, sub_tree=utxo
```

- [ ] **Step 6: Run all tests + lint**

```bash
cargo test --workspace 2>&1 | tail -5    # 85 still pass
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

- [ ] **Step 7: Commit**

```bash
git add crates/omega-commitment-cli/src/main.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(cli): add token-policy arm + mark SubTree #[non_exhaustive]"
```

---

## Task 4: CLI smoke test for token-policy

**Files:**
- Modify: `crates/omega-commitment-cli/tests/cli.rs`

Add one new smoke test parallel to the existing utxo / header / tx-index ones.

- [ ] **Step 1: Append to `crates/omega-commitment-cli/tests/cli.rs`**

```rust

#[test]
fn cli_commit_token_policy_smoke() {
    let out = run_commit("token-policy", "token_policies_small.json");
    let body = fs::read_to_string(out.path().join("commitment.json")).unwrap();
    assert!(
        body.contains("\"sub_tree\": \"token-policy\""),
        "wrong sub_tree tag: {body}"
    );
    assert!(body.contains("\"input_digest\":"));
    assert!(body.contains("\"root\":"));
    assert!(body.contains("\"item_count\": 8"));
    let witness_count = fs::read_dir(out.path().join("witnesses")).unwrap().count();
    assert_eq!(witness_count, 8, "expected 8 token-policy witness files");
}
```

(`run_commit` and `fixture_path` are already defined at the top of the file from prior plans.)

- [ ] **Step 2: Run CLI tests**

```bash
cargo test -p omega-commitment-cli --test cli 2>&1 | tail -10
```

Expected: 8 tests pass (7 prior + 1 new).

- [ ] **Step 3: Run full workspace + lint**

```bash
cargo test --workspace 2>&1 | tail -5    # 86 total
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-cli/tests/cli.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test(cli): token-policy smoke test"
```

---

## Task 5: Bump to v0.4.0 + extend README

**Files:**
- Modify: `crates/omega-commitment-core/Cargo.toml`
- Modify: `crates/omega-commitment-cli/Cargo.toml`
- Modify: `README.md`

- [ ] **Step 1: Bump core crate version**

In `/home/hoskinson/omega-commitment/crates/omega-commitment-core/Cargo.toml`, change `version = "0.3.1"` to `version = "0.4.0"`.

- [ ] **Step 2: Bump CLI crate version**

In `/home/hoskinson/omega-commitment/crates/omega-commitment-cli/Cargo.toml`, change `version = "0.3.1"` to `version = "0.4.0"`.

- [ ] **Step 3: Verify build + tests + lint**

```bash
cargo build --workspace 2>&1 | grep -E "warning|error"   # empty
cargo lint 2>&1 | tail -3                                  # clean
cargo fmt-check 2>&1 | tail -3                             # clean
cargo test --workspace 2>&1 | tail -5                       # 86 tests pass
```

- [ ] **Step 4: Append to README.md**

Append to `/home/hoskinson/omega-commitment/README.md`:

```markdown
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

- **Dual-track shadow hash** — program-level pending decision. Plonky3 circuit authors must NOT lock to v0.4.0 single-root format until decided.
```

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-core/Cargo.toml \
        crates/omega-commitment-cli/Cargo.toml \
        README.md
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "chore: bump to 0.4.0; document token-policy sub-tree"
```

- [ ] **Step 6: Final verification**

```bash
git log --oneline | head -8
cargo test --workspace 2>&1 | tail -5
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

Expected: HEAD is the version-bump commit, 86 tests pass, no lint/fmt issues.

---

## Self-review

**Spec coverage** (from `2026-05-01-ouroboros-omega-design.md` §7, sub-tree 4):
- "Native token policy registry — Merkle tree of all minting policy hashes that ever issued a token on the old chain, with their first-issuance slot and total supply at H."
  - ✅ Leaf encodes (`policy_id`, `first_issuance_slot`, `total_supply_at_h`) — exact spec match.
  - ✅ Plonky3-friendly tree reused unchanged.
  - ✅ Witness format unchanged from sub-trees 1, 2, 3.
  - ✅ `claim_token_policy` (spec §9.2) gets a tree to prove against.
  - ✅ The 28-byte `policy_id` matches Cardano on-chain semantics (Blake2b-224).

**Decision honoring:**
- ✅ Decision 7 (PQ-only): Blake2b for leaf hashing, no curves. `policy_id` is upstream Cardano data.
- ✅ Decision 8 (Plonky3-friendly): fixed-width 52-byte leaf encoding, sort-then-pad tree.
- ✅ Decision 3 (everything-provable): adds 4th of 7 sub-trees.
- ✅ Dual-hash decision still deferred — no commitment-format change.

**Carry-over status from v0.3.1:**
- ✅ `#[non_exhaustive]` on `SubTree` enum — Task 3 Step 1.
- ⏸️ Dual-hash decision — program-level, not addressed in this plan.
- All other v0.3.1 hardening items remain closed (path traversal, size cap, atomic write, dispatcher refactor, hex codec consolidation, CI, layer cloning).

**Placeholder scan:** All code blocks contain runnable code. No "TBD", no "fill in later". ✅

**Type consistency:**
- `PolicyId = [u8; 28]` defined once in `token_policy_leaf.rs`; used consistently in struct fields, fixture, integration test, CLI input.
- `TokenPolicy { policy_id, first_issuance_slot, total_supply_at_h }` referenced uniformly across module, fixture, integration test, and CLI.
- `validate_policy_id_uniqueness` signature consistent.
- `SubTree::TokenPolicy` enum variant + serde `kebab-case` rendering as `"token-policy"` matches CLI flag spelling and smoke-test substring assertion.
- `build_token_policy_leaves` free function signature matches the pattern of `build_utxo_leaves` / `build_header_leaves` / `build_tx_index_leaves`.
- ✅ No drift.

**Bite-sized tasks:** 5 tasks, each with 4–7 numbered steps; each step is a single action. ✅

**Net delta:** +13 unit tests + 4 integration tests + 1 CLI smoke test = +18 tests (68 → 86). 5 commits.

---

## What's NOT in this plan (and why)

- **Real Cardano mainnet token-policy ingestion.** Synthetic fixture only. A `cardano-multiplatform-lib` integration is deferred.
- **Cross-sub-tree validation** (e.g., "every UTXO's native asset references a policy that exists in the token-policy sub-tree"). Requires both commitments loaded simultaneously; deferred to a future cross-validation plan.
- **Asset-name registry.** Cardano native tokens are `(policy_id, asset_name)` tuples. This sub-tree commits only to `policy_id`. A separate per-asset registry could be added later if needed; the spec does not require it.
- **Plonky3 `claim_token_policy` circuit.** Track T2.
- **Dual-track shadow hash.** Program-level pending decision.
- **Sub-trees 5/6/7** (script registry, stake state, governance state). Each gets its own plan.

---

## How to execute this plan

Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Five tasks, each independently committable.

Total runway estimate: **2–3 days** for an experienced Rust dev (smaller than v0.3.0 because the architecture is fully mature and the v0.3.1 dispatcher refactor reduced the CLI work to one new function + one match arm).

Expected post-execution state:
- 5 commits added on top of v0.3.1 (currently 38 commits)
- ~18 net new tests (86 total)
- Both crates at version 0.4.0
- 4 of 7 sub-trees shipped; 3 remaining (script registry, stake state, governance state)

Next plan: either the program-level **dual-track shadow hash decision** or `2026-XX-XX-omega-script-registry-plan.md` (sub-tree 5 of 7).
