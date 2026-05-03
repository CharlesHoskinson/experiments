# Omega v0.8.0 — Cardano Ingestion + Golden Vector QA Suite

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lock canonical golden test vectors across every existing Ω-Commitment component (per-sub-tree, bundle), then add a Cardano-aware ingestion crate (`omega-commitment-ingest`) that transforms real Cardano data into the per-sub-tree JSON formats. Ship UTXO ingestion end-to-end against a hand-crafted CBOR fixture; scaffold the other four LedgerState-derivable sub-trees as documented follow-up work.

**Architecture:** Two-phase. Phase 1 (Tasks 1–3): golden-vector regression net for everything already shipped (synthetic + bundle). Phase 2 (Tasks 4–9): new `omega-commitment-ingest` workspace crate using `pallas` for CBOR parsing; UTXO sub-tree fully implemented end-to-end; the four other LedgerState-derivable sub-trees scaffolded with explicit `unimplemented!()` markers and `#[ignore]`d-but-present tests so the structure exists.

**Tech Stack:** Rust 1.79+, blake2, sha3, serde, clap, anyhow, plus new dep **`pallas-codec = "0.30"`** + **`pallas-primitives = "0.30"`** + **`pallas-traverse = "0.30"`** for CBOR parsing. NO `pallas-network` (no chain-follower in this plan).

**Track:** T1 (Ω-Commitment Tooling) — real-data ingestion sub-phase.

**Locked design decisions honored (unchanged):**
- PQ-only crypto, Plonky3-friendly tree layout, selective dual-track at bundle layer, lazy/pull migration.

---

## Honest scope statement

The QA / golden-vector half of this plan is **fully implementable** today — we have all the inputs (existing fixtures + the v0.7.0 smoke-run roots).

The Cardano-ingestion half is **mostly aspirational** for the four non-UTXO sub-trees because the exact `pallas` API surface for navigating Conway/Plomin-era LedgerState CBOR has to be discovered during implementation. This plan ships:
- A real, end-to-end UTXO ingestion path (hand-crafted CBOR fixture → parsed → JSON → leaf hashes).
- Scaffolding (CLI surface, library structure, test harness) for the other four LedgerState-derivable sub-trees, with `unimplemented!()` bodies and `#[ignore]`d test stubs that mark exactly what's left.
- Documentation calling out which paths are real and which are scaffolded.

This is honest about what 7 days of work will produce. Follow-up plans flesh out token-policy / script / stake / governance ingestion against the real pallas API once the UTXO path proves the architecture.

---

## File structure (post-plan)

```
omega-commitment/
├── Cargo.toml                                            (workspace; add omega-commitment-ingest member; add pallas to workspace.dependencies)
├── README.md                                             (extended: v0.8.0 release notes, ingestion + QA docs)
├── .gitignore                                            (add var/snapshots/)
├── scripts/
│   └── download_snapshot.sh                              (NEW — Mithril preview-testnet downloader, human-invoked)
├── var/                                                  (NEW gitignored — for downloaded snapshots)
├── crates/
│   ├── omega-commitment-core/
│   │   ├── Cargo.toml                                    (version 0.8.0)
│   │   └── tests/
│   │       └── golden_vectors.rs                         (NEW — pinned per-sub-tree leaf hashes + roots)
│   ├── omega-commitment-cli/
│   │   └── Cargo.toml                                    (version 0.8.0)
│   ├── omega-commitment-bundle/
│   │   ├── Cargo.toml                                    (version 0.8.0)
│   │   └── tests/
│   │       └── golden_bundle.rs                          (NEW — pinned blake2b + sha3 bundle roots)
│   └── omega-commitment-ingest/                          (NEW workspace member)
│       ├── Cargo.toml                                    (version 0.8.0)
│       ├── src/
│       │   ├── lib.rs                                    (re-exports + crate docs)
│       │   ├── cbor.rs                                   (minimal LedgerState CBOR navigation via pallas)
│       │   ├── utxo.rs                                   (CBOR UTXOs → omega_commitment_core::Utxo → JSON)
│       │   ├── token_policy.rs                           (scaffold: derives policies from UTXO multi-assets — partial)
│       │   ├── script.rs                                 (scaffold: derives scripts from UTXO credentials — partial)
│       │   ├── stake.rs                                  (scaffold: stake snapshot → StakeEntry — unimplemented!)
│       │   ├── governance.rs                             (scaffold: treasury+CC+gov → GovernanceFact — unimplemented!)
│       │   └── main.rs                                   (CLI: per-sub-tree subcommands)
│       └── tests/
│           ├── fixtures/
│           │   └── ledger_state_minimal.cbor             (NEW — hand-crafted minimal LedgerState fixture)
│           ├── utxo_ingest_integration.rs                (NEW — UTXO end-to-end against hand-crafted CBOR)
│           ├── qa_pipeline.rs                            (NEW — full pipeline: CBOR → ingest → commit → bundle)
│           └── golden_ingest.rs                          (NEW — pinned ingestion outputs for the hand-crafted fixture)
```

Each file has one clear responsibility. Pallas integration is contained in `cbor.rs`; per-sub-tree transforms live in their own files; the CLI is thin.

---

## Task 1: Golden vectors for per-sub-tree synthetic fixtures

**Files:**
- Create: `crates/omega-commitment-core/tests/golden_vectors.rs`

Pin canonical leaf hashes and tree roots for the seven existing per-sub-tree fixtures. Any future encoding change breaks these tests loudly.

This task does NOT touch any code — it consumes the existing fixtures and freezes their outputs. The hash values come from running the existing modules against the existing fixtures and recording what they produce.

- [ ] **Step 1: Compute the golden values from the current code**

Run a one-shot helper to print the canonical roots for each fixture (this is just to capture the values; the values themselves go into the static test file in Step 2):

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
cat > /tmp/print_goldens.rs << 'EOF'
use omega_commitment_core::{
    governance_state_leaf::GovernanceFact, header_leaf::BlockHeader,
    script_registry_leaf::ScriptEntry, stake_state_leaf::StakeEntry,
    token_policy_leaf::TokenPolicy, tree::MerkleTree,
    tx_index_leaf::TxIndexEntry, utxo_leaf::Utxo,
};
use serde::Deserialize;
use std::fs;

#[derive(Deserialize)] struct UtxoIn { utxos: Vec<Utxo> }
#[derive(Deserialize)] struct HeaderIn { headers: Vec<BlockHeader> }
#[derive(Deserialize)] struct TxIn { entries: Vec<TxIndexEntry> }
#[derive(Deserialize)] struct PolIn { policies: Vec<TokenPolicy> }
#[derive(Deserialize)] struct ScriptIn { scripts: Vec<ScriptEntry> }
#[derive(Deserialize)] struct StakeIn { stake_entries: Vec<StakeEntry> }
#[derive(Deserialize)] struct GovIn { facts: Vec<GovernanceFact> }

fn main() {
    let dir = "crates/omega-commitment-core/tests/fixtures";
    let raw = fs::read_to_string(format!("{dir}/utxo_set_small.json")).unwrap();
    let f: UtxoIn = serde_json::from_str(&raw).unwrap();
    let leaves: Vec<_> = f.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    println!("UTXO    root: {}", hex::encode(MerkleTree::build(leaves).root()));

    let raw = fs::read_to_string(format!("{dir}/header_chain_small.json")).unwrap();
    let f: HeaderIn = serde_json::from_str(&raw).unwrap();
    let leaves: Vec<_> = f.headers.iter().map(|h| h.leaf_hash()).collect();
    println!("HEADER  root: {}", hex::encode(MerkleTree::build(leaves).root()));

    let raw = fs::read_to_string(format!("{dir}/tx_index_small.json")).unwrap();
    let f: TxIn = serde_json::from_str(&raw).unwrap();
    let leaves: Vec<_> = f.entries.iter().map(|e| e.leaf_hash()).collect();
    println!("TX      root: {}", hex::encode(MerkleTree::build(leaves).root()));

    let raw = fs::read_to_string(format!("{dir}/token_policies_small.json")).unwrap();
    let f: PolIn = serde_json::from_str(&raw).unwrap();
    let leaves: Vec<_> = f.policies.iter().map(|p| p.leaf_hash()).collect();
    println!("POLICY  root: {}", hex::encode(MerkleTree::build(leaves).root()));

    let raw = fs::read_to_string(format!("{dir}/script_registry_small.json")).unwrap();
    let f: ScriptIn = serde_json::from_str(&raw).unwrap();
    let leaves: Vec<_> = f.scripts.iter().map(|s| s.leaf_hash()).collect();
    println!("SCRIPT  root: {}", hex::encode(MerkleTree::build(leaves).root()));

    let raw = fs::read_to_string(format!("{dir}/stake_state_small.json")).unwrap();
    let f: StakeIn = serde_json::from_str(&raw).unwrap();
    let leaves: Vec<_> = f.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    println!("STAKE   root: {}", hex::encode(MerkleTree::build(leaves).root()));

    let raw = fs::read_to_string(format!("{dir}/governance_state_small.json")).unwrap();
    let f: GovIn = serde_json::from_str(&raw).unwrap();
    let leaves: Vec<_> = f.facts.iter().map(|fact| fact.leaf_hash()).collect();
    println!("GOV     root: {}", hex::encode(MerkleTree::build(leaves).root()));
}
EOF
# Run inline as a one-shot — easiest is to drop it into a temp test crate or use cargo --example
# For simplicity: use cargo test with a hidden test that prints + always-fails, then pull values from output.
```

For practical purposes, capture the values by writing a temporary `#[test]` in `golden_vectors.rs` that prints + asserts `false`, run it, copy the printed hashes from output, then replace the test body with `assert_eq!` against those values.

If the v0.7.0 smoke run printed roots for some sub-trees, reuse those. From the v0.7.0 smoke we have one known anchor: the **bundle-level** roots are `blake2b=ee308b53...0186aebd712`, `sha3=189826cf...e5461638b77`. Per-sub-tree roots will be filled in here.

- [ ] **Step 2: Write the golden vectors test file**

After capturing the seven per-sub-tree roots in Step 1, write the test file:

`/home/hoskinson/omega-commitment/crates/omega-commitment-core/tests/golden_vectors.rs`:

```rust
//! Golden vectors for per-sub-tree canonical roots against the seven
//! shipped synthetic fixtures.
//!
//! These hashes are pinned constants. If a code change causes any of
//! them to drift, the test fails — and that's the point. A failure
//! here means either:
//!   - a bug was introduced in encoding logic (revert), or
//!   - encoding logic was deliberately changed (regenerate vectors as
//!     a SemVer-major change with a recorded decision).

use omega_commitment_core::{
    governance_state_leaf::GovernanceFact, header_leaf::BlockHeader,
    script_registry_leaf::ScriptEntry, stake_state_leaf::StakeEntry,
    token_policy_leaf::TokenPolicy, tree::MerkleTree,
    tx_index_leaf::TxIndexEntry, utxo_leaf::Utxo,
};
use serde::Deserialize;

const FIXTURES: &str = "tests/fixtures";

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURES}/{name}")).unwrap()
}

#[derive(Deserialize)] struct UtxoIn { utxos: Vec<Utxo> }
#[derive(Deserialize)] struct HeaderIn { headers: Vec<BlockHeader> }
#[derive(Deserialize)] struct TxIn { entries: Vec<TxIndexEntry> }
#[derive(Deserialize)] struct PolIn { policies: Vec<TokenPolicy> }
#[derive(Deserialize)] struct ScriptIn { scripts: Vec<ScriptEntry> }
#[derive(Deserialize)] struct StakeIn { stake_entries: Vec<StakeEntry> }
#[derive(Deserialize)] struct GovIn { facts: Vec<GovernanceFact> }

#[test]
fn golden_utxo_root() {
    let f: UtxoIn = serde_json::from_str(&read_fixture("utxo_set_small.json")).unwrap();
    let leaves: Vec<_> = f.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    let root = MerkleTree::build(leaves).root();
    // GOLDEN: regenerate via Step 1 if encoding semantics change.
    assert_eq!(
        hex::encode(root),
        "<INSERT_UTXO_ROOT_HEX_HERE>",
        "UTXO sub-tree root drifted"
    );
}

#[test]
fn golden_header_root() {
    let f: HeaderIn = serde_json::from_str(&read_fixture("header_chain_small.json")).unwrap();
    let leaves: Vec<_> = f.headers.iter().map(|h| h.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "<INSERT_HEADER_ROOT_HEX_HERE>",
        "Header sub-tree root drifted"
    );
}

#[test]
fn golden_tx_index_root() {
    let f: TxIn = serde_json::from_str(&read_fixture("tx_index_small.json")).unwrap();
    let leaves: Vec<_> = f.entries.iter().map(|e| e.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "<INSERT_TX_INDEX_ROOT_HEX_HERE>",
        "Tx-index sub-tree root drifted"
    );
}

#[test]
fn golden_token_policy_root() {
    let f: PolIn = serde_json::from_str(&read_fixture("token_policies_small.json")).unwrap();
    let leaves: Vec<_> = f.policies.iter().map(|p| p.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "<INSERT_TOKEN_POLICY_ROOT_HEX_HERE>",
        "Token-policy sub-tree root drifted"
    );
}

#[test]
fn golden_script_root() {
    let f: ScriptIn = serde_json::from_str(&read_fixture("script_registry_small.json")).unwrap();
    let leaves: Vec<_> = f.scripts.iter().map(|s| s.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "<INSERT_SCRIPT_ROOT_HEX_HERE>",
        "Script-registry sub-tree root drifted"
    );
}

#[test]
fn golden_stake_root() {
    let f: StakeIn = serde_json::from_str(&read_fixture("stake_state_small.json")).unwrap();
    let leaves: Vec<_> = f.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "<INSERT_STAKE_ROOT_HEX_HERE>",
        "Stake-state sub-tree root drifted"
    );
}

#[test]
fn golden_governance_root() {
    let f: GovIn = serde_json::from_str(&read_fixture("governance_state_small.json")).unwrap();
    let leaves: Vec<_> = f.facts.iter().map(|fact| fact.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "<INSERT_GOV_ROOT_HEX_HERE>",
        "Governance-state sub-tree root drifted"
    );
}
```

**Practical workflow for filling in the seven hex placeholders:**
1. Replace each `"<INSERT_..._HERE>"` with `"deadbeef"` (any wrong value).
2. Run `cargo test --workspace golden_ -- --nocapture`. Each test will fail with a message like `assertion failed: 'deadbeef' != '<actual hex>'`.
3. From the assertion output, read each actual hex string.
4. Edit the file again, replacing each `"deadbeef"` with the actual hex.
5. Re-run the tests — they should all pass.

This bootstrap "fail-once-then-pin" pattern is the standard way to capture golden vectors deterministically.

- [ ] **Step 3: Verify**

```bash
cargo test --workspace 2>&1 | tail -10    # 174 total (167 prior + 7 new)
cargo lint 2>&1 | tail -3                   # clean
cargo fmt-check 2>&1 | tail -3              # clean
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-core/tests/golden_vectors.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test(qa): pin per-sub-tree golden roots for seven synthetic fixtures"
```

---

## Task 2: Golden vectors for the bundle root tuple

**Files:**
- Create: `crates/omega-commitment-bundle/tests/golden_bundle.rs`

Pin the bundle root tuple from the v0.7.0 smoke run against the seven synthetic fixtures. We already know these values:

- `blake2b_bundle_root = ee308b538b26e6d87b115ffac5676f39d0059f75dd8c79221b6b80186aebd712`
- `sha3_bundle_root    = 189826cfa4be57615db0ac4e5fab2602291921d54365198847927e5461638b77`

- [ ] **Step 1: Write the golden bundle test**

`/home/hoskinson/omega-commitment/crates/omega-commitment-bundle/tests/golden_bundle.rs`:

```rust
//! Golden vector for the canonical Ω-Commitment bundle root tuple
//! against the seven shipped synthetic fixtures.
//!
//! These two hashes are the canonical "synthetic-corpus" Ω-Commitment.
//! Pinned at v0.7.0 smoke run and frozen here to catch any drift in:
//!   - per-sub-tree leaf encodings
//!   - per-sub-tree root aggregation (Blake2b)
//!   - the SHA3 root parallel computation
//!   - bundle root aggregation (Blake2b + SHA3 over concatenated roots)
//!   - canonical sub-tree ordering
//!
//! Any failure means SOMETHING in the dual-track commitment path
//! changed. Investigate before regenerating these constants.

use omega_commitment_bundle::bundle::assemble;
use std::{fs, path::PathBuf};

/// Path to the omega-commitment-core fixtures dir.
fn core_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("omega-commitment-core/tests/fixtures")
}

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
        fs::copy(src.join(src_name), dest.join(dest_name)).unwrap();
    }
}

#[test]
fn golden_bundle_blake2b_root() {
    let dir = tempfile::tempdir().unwrap();
    populate_input_dir(dir.path());
    let bundle = assemble(dir.path()).unwrap();
    assert_eq!(
        hex::encode(bundle.blake2b_bundle_root),
        "ee308b538b26e6d87b115ffac5676f39d0059f75dd8c79221b6b80186aebd712",
        "blake2b_bundle_root drifted from v0.7.0 pin"
    );
}

#[test]
fn golden_bundle_sha3_root() {
    let dir = tempfile::tempdir().unwrap();
    populate_input_dir(dir.path());
    let bundle = assemble(dir.path()).unwrap();
    assert_eq!(
        hex::encode(bundle.sha3_bundle_root),
        "189826cfa4be57615db0ac4e5fab2602291921d54365198847927e5461638b77",
        "sha3_bundle_root drifted from v0.7.0 pin"
    );
}

#[test]
fn golden_bundle_canonical_order_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    populate_input_dir(dir.path());
    let bundle = assemble(dir.path()).unwrap();
    let labels: Vec<&str> = bundle.sub_trees.iter().map(|s| s.sub_tree.as_str()).collect();
    assert_eq!(
        labels,
        vec!["utxo", "header", "tx-index", "token-policy", "script", "stake", "governance"],
        "canonical sub-tree order changed"
    );
}
```

- [ ] **Step 2: Verify**

```bash
cargo test -p omega-commitment-bundle --test golden_bundle 2>&1 | tail -10   # 3 tests pass
cargo test --workspace 2>&1 | tail -5                                          # 177 total
cargo lint 2>&1 | tail -3                                                      # clean
cargo fmt-check 2>&1 | tail -3                                                 # clean
```

If a test fails because the actual hash doesn't match the pinned value, that is **not** a bug in this task — it means encoding logic drifted between v0.7.0 and now. Investigate before changing the pin.

- [ ] **Step 3: Commit**

```bash
git add crates/omega-commitment-bundle/tests/golden_bundle.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test(qa): pin canonical Ω-Commitment bundle root tuple from v0.7.0"
```

---

## Task 3: Witness round-trip golden vectors

**Files:**
- Modify: `crates/omega-commitment-core/tests/golden_vectors.rs`

Add witness round-trip tests for one sub-tree (UTXO) — proves the witness format is also deterministic, not just the roots. Three tests:
1. Witness for a known UTXO leaf has a known shape (sibling count, leaf_index).
2. Witness verifies against the golden root.
3. Witness JSON serialization is stable.

- [ ] **Step 1: Append to `golden_vectors.rs`**

Append at the end of the file (after the seven golden root tests):

```rust

#[test]
fn golden_utxo_witness_round_trip() {
    use omega_commitment_core::witness::InclusionWitness;

    let f: UtxoIn = serde_json::from_str(&read_fixture("utxo_set_small.json")).unwrap();
    let leaves: Vec<_> = f.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    let tree = MerkleTree::build(leaves.clone());
    let root = tree.root();

    // For each leaf, build a witness and verify it.
    for leaf in &leaves {
        let w = InclusionWitness::build(&tree, *leaf).expect("leaf in tree");
        assert!(w.verify(root), "witness must verify against pinned golden root");
    }

    // Serialize the first leaf's witness; confirm it round-trips.
    let w = InclusionWitness::build(&tree, leaves[0]).unwrap();
    let json = serde_json::to_string(&w).unwrap();
    let w2: InclusionWitness = serde_json::from_str(&json).unwrap();
    assert_eq!(w, w2, "witness JSON round-trip diverged");

    // Witness shape: sibling count must equal tree depth.
    assert_eq!(
        w.siblings.len() as u32,
        tree.depth() as u32,
        "witness siblings count != tree depth"
    );
}
```

- [ ] **Step 2: Verify**

```bash
cargo test -p omega-commitment-core --test golden_vectors 2>&1 | tail -10   # 8 tests pass
cargo test --workspace 2>&1 | tail -5                                         # 178 total
cargo lint 2>&1 | tail -3                                                     # clean
cargo fmt-check 2>&1 | tail -3                                                # clean
```

- [ ] **Step 3: Commit**

```bash
git add crates/omega-commitment-core/tests/golden_vectors.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test(qa): pin UTXO witness round-trip and shape invariants"
```

---

## Task 4: New `omega-commitment-ingest` crate scaffold + workspace update

**Files:**
- Modify: `Cargo.toml` (workspace)
- Create: `crates/omega-commitment-ingest/Cargo.toml`
- Create: `crates/omega-commitment-ingest/src/lib.rs` (stub)
- Create: `crates/omega-commitment-ingest/src/main.rs` (stub)
- Modify: `.gitignore`

- [ ] **Step 1: Make directories**

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
mkdir -p crates/omega-commitment-ingest/src
mkdir -p crates/omega-commitment-ingest/tests/fixtures
mkdir -p var/snapshots
mkdir -p scripts
```

- [ ] **Step 2: Write the crate `Cargo.toml`**

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/Cargo.toml`:

```toml
[package]
name = "omega-commitment-ingest"
version = "0.8.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lib]
name = "omega_commitment_ingest"
path = "src/lib.rs"

[[bin]]
name = "omega-ingest"
path = "src/main.rs"

[dependencies]
omega-commitment-core = { path = "../omega-commitment-core" }
clap.workspace = true
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
hex.workspace = true
thiserror.workspace = true
anyhow = "1"
pallas-codec = "0.30"
pallas-primitives = "0.30"
pallas-traverse = "0.30"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Add the new crate to the workspace**

Edit `/home/hoskinson/omega-commitment/Cargo.toml`. Update the `members = [...]` list to include the fourth entry:

```toml
members = [
  "crates/omega-commitment-core",
  "crates/omega-commitment-cli",
  "crates/omega-commitment-bundle",
  "crates/omega-commitment-ingest",
]
```

- [ ] **Step 4: Write stubs**

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/lib.rs`:

```rust
//! omega-commitment-ingest: transforms real Cardano data (CBOR
//! LedgerState snapshots, chain history) into the per-sub-tree JSON
//! formats consumed by `omega-commitment commit`.
//!
//! v0.8.0 implements UTXO ingestion end-to-end and scaffolds the
//! other four LedgerState-derivable sub-trees (token-policy, script,
//! stake, governance). Header and tx-index ingestion is documented
//! as future work requiring a chain-follower.
```

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/main.rs`:

```rust
fn main() {}
```

- [ ] **Step 5: Update `.gitignore`**

Append to `/home/hoskinson/omega-commitment/.gitignore`:

```

# Downloaded snapshots (Mithril testnet, etc.) — not tracked
var/snapshots/
```

- [ ] **Step 6: Verify**

```bash
cargo build --workspace 2>&1 | tail -10
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

The build will pull pallas crates from crates.io — first build will be slow (fetching deps, compiling pallas's dependency tree). If pallas 0.30 is unavailable, fall back to the latest 0.x version available; record the version actually used. If pallas 0.30+ has API changes from prior versions that break this plan's assumed API surface, document deviations in the per-task notes and proceed.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml .gitignore crates/omega-commitment-ingest/
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "chore(ingest): scaffold omega-commitment-ingest workspace crate with pallas dep"
```

---

## Task 5: Hand-crafted minimal CBOR fixture

**Files:**
- Create: `crates/omega-commitment-ingest/tests/fixtures/ledger_state_minimal.cbor` (binary)
- Create: `crates/omega-commitment-ingest/tests/fixtures/ledger_state_minimal.cbor.md` (documentation of fixture contents)

We need a deterministic, in-tree CBOR fixture that exercises UTXO parsing without requiring a real Mithril snapshot. The fixture is hand-crafted with documented byte-by-byte semantics so future maintainers can regenerate it.

The fixture is a minimal subset of the LedgerState CBOR shape — just enough for UTXO ingestion. We focus on the UTXO portion only; other LedgerState components are simulated as empty / minimal.

- [ ] **Step 1: Write a Rust generator that produces the fixture**

Create a one-shot helper. Write `/tmp/gen_fixture.rs`:

```rust
use std::fs;

// Minimal Conway-era UTXO encoding (CBOR):
//   CBOR map: { tx_id_bytes => [output_index, output_record] }
// where output_record is: [address_bytes, value, datum_option]
//
// We'll write a SIMPLIFIED CBOR with three UTXOs that omega-commitment
// can recover via a custom parser. We deliberately use a simplified
// format that doesn't require full Conway-era LedgerState parsing
// machinery — the fixture is self-describing and the parser only
// needs to handle this shape.
//
// CBOR top-level: array of 3 UTXO records.
// Each UTXO: array of [tx_id (32 bytes), output_index (u32), address (32 bytes),
//                       value_lovelace (u64)].

fn cbor_array_header(len: usize) -> Vec<u8> {
    if len < 24 { vec![(0x80u8 | len as u8)] }
    else if len < 256 { vec![0x98, len as u8] }
    else { vec![0x99, (len >> 8) as u8, len as u8] }
}

fn cbor_bytes_header(len: usize) -> Vec<u8> {
    if len < 24 { vec![(0x40u8 | len as u8)] }
    else if len < 256 { vec![0x58, len as u8] }
    else { vec![0x59, (len >> 8) as u8, len as u8] }
}

fn cbor_uint(v: u64) -> Vec<u8> {
    if v < 24 { vec![v as u8] }
    else if v <= 0xff { vec![0x18, v as u8] }
    else if v <= 0xffff { vec![0x19, (v >> 8) as u8, v as u8] }
    else if v <= 0xffff_ffff {
        let mut o = vec![0x1a];
        o.extend_from_slice(&(v as u32).to_be_bytes());
        o
    } else {
        let mut o = vec![0x1b];
        o.extend_from_slice(&v.to_be_bytes());
        o
    }
}

fn cbor_bytes(b: &[u8]) -> Vec<u8> {
    let mut o = cbor_bytes_header(b.len());
    o.extend_from_slice(b);
    o
}

fn utxo(tx_id: [u8; 32], out_idx: u64, addr: [u8; 32], val: u64) -> Vec<u8> {
    let mut o = cbor_array_header(4);
    o.extend(cbor_bytes(&tx_id));
    o.extend(cbor_uint(out_idx));
    o.extend(cbor_bytes(&addr));
    o.extend(cbor_uint(val));
    o
}

fn main() {
    let mut buf = Vec::new();
    buf.extend(cbor_array_header(3));
    buf.extend(utxo([0x11; 32], 0, [0xAA; 32], 1_000_000));
    buf.extend(utxo([0x22; 32], 1, [0xBB; 32], 5_000_000));
    buf.extend(utxo([0x33; 32], 2, [0xCC; 32], 250_000_000));
    fs::write("crates/omega-commitment-ingest/tests/fixtures/ledger_state_minimal.cbor", &buf).unwrap();
    println!("wrote {} bytes", buf.len());
}
```

Run it:
```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
cargo new --bin /tmp/gen_fixture
cp /tmp/gen_fixture.rs /tmp/gen_fixture/src/main.rs
cd /tmp/gen_fixture && cargo run --release
cd /home/hoskinson/omega-commitment
ls -la crates/omega-commitment-ingest/tests/fixtures/
```

Expected: ~210-byte file.

The fixture is intentionally a SIMPLIFIED CBOR (not full Conway LedgerState) — this lets us exercise the ingestion → JSON → leaf-hash pipeline without depending on pallas's full LedgerState parser. Pallas is still pulled in for the eventual real-snapshot work, but for the in-tree QA fixture we use a custom-parser approach.

- [ ] **Step 2: Document the fixture**

Write `/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_minimal.cbor.md`:

```markdown
# `ledger_state_minimal.cbor`

A hand-crafted, deterministic CBOR fixture containing a 3-UTXO synthetic
ledger state. NOT a real Cardano LedgerState snapshot — uses a
simplified `(tx_id, output_index, address, value_lovelace)` array
encoding that exercises the ingestion→JSON→leaf-hash pipeline without
requiring the full Conway-era LedgerState parsing machinery.

## Contents

| tx_id (hex) | output_index | address (hex) | value_lovelace |
|---|---|---|---|
| `1111...11` (32×0x11) | 0 | `aaaa...aa` | 1_000_000 |
| `2222...22` | 1 | `bbbb...bb` | 5_000_000 |
| `3333...33` | 2 | `cccc...cc` | 250_000_000 |

## Encoding

CBOR array of 3 UTXOs. Each UTXO is a 4-element array:

```
[ tx_id_bytes(32), output_index_u64, address_bytes(32), value_lovelace_u64 ]
```

## Regeneration

The fixture is generated by the Rust helper documented in Task 5,
Step 1 of `2026-05-01-omega-cardano-ingestion-and-qa-plan.md`.

If you need to regenerate, copy that helper into a temp Cargo project,
run it, and `cargo fmt`/lint will not be affected (the fixture is
binary, gitignored from those tools).
```

- [ ] **Step 3: Verify the fixture is committed correctly**

```bash
file crates/omega-commitment-ingest/tests/fixtures/ledger_state_minimal.cbor
xxd crates/omega-commitment-ingest/tests/fixtures/ledger_state_minimal.cbor | head -5
```

Expected: ~210-byte binary file. xxd should show the leading `0x83` (CBOR array of 3) followed by the first UTXO's `0x84` (CBOR array of 4) and then `0x58 0x20 0x11 0x11 ...` (32-byte byte string of 0x11s).

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-ingest/tests/fixtures/
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test(ingest): add hand-crafted minimal CBOR fixture for UTXO ingestion QA"
```

---

## Task 6: UTXO ingestion (CBOR → omega_commitment_core::Utxo → JSON)

**Files:**
- Create: `crates/omega-commitment-ingest/src/cbor.rs`
- Create: `crates/omega-commitment-ingest/src/utxo.rs`
- Modify: `crates/omega-commitment-ingest/src/lib.rs`

Implement the UTXO ingestion path against the simplified CBOR fixture from Task 5. Uses `pallas-codec` for CBOR decoding (we use the simple decoder since our fixture is plain CBOR, not the full LedgerState shape).

- [ ] **Step 1: Write `cbor.rs` — minimal CBOR decoder helpers**

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/cbor.rs`:

```rust
//! Minimal CBOR navigation helpers.
//!
//! For v0.8.0 we parse a hand-crafted simplified CBOR fixture
//! (see `tests/fixtures/ledger_state_minimal.cbor.md`). When real
//! Mithril/LedgerState snapshot ingestion lands in a later release,
//! this module will be expanded with `pallas-traverse`-based readers
//! for the full Conway-era LedgerState shape.

use anyhow::{anyhow, Result};
use pallas_codec::minicbor::{decode, Decoder};

/// Read a 32-byte fixed-length byte string from a `Decoder` cursor.
/// Returns Err if the next item is not exactly 32 bytes.
pub fn read_32_bytes<'b>(d: &mut Decoder<'b>) -> Result<[u8; 32]> {
    let bytes = d.bytes().map_err(|e| anyhow!("cbor: expected bytes ({e})"))?;
    if bytes.len() != 32 {
        return Err(anyhow!("cbor: expected 32-byte string, got {}", bytes.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(bytes);
    Ok(out)
}

/// Read a u64 from a `Decoder` cursor.
pub fn read_u64<'b>(d: &mut Decoder<'b>) -> Result<u64> {
    d.u64().map_err(|e| anyhow!("cbor: expected u64 ({e})"))
}

/// Read a u32 (encoded as u64 in CBOR) from a `Decoder` cursor.
pub fn read_u32<'b>(d: &mut Decoder<'b>) -> Result<u32> {
    let v = read_u64(d)?;
    u32::try_from(v).map_err(|_| anyhow!("cbor: u64 value {v} too large for u32"))
}

/// Expect an array of length `expected` next on the cursor.
pub fn expect_array<'b>(d: &mut Decoder<'b>, expected: usize) -> Result<()> {
    let actual = d.array().map_err(|e| anyhow!("cbor: expected array ({e})"))?
        .ok_or_else(|| anyhow!("cbor: expected definite-length array"))?;
    if actual as usize != expected {
        return Err(anyhow!("cbor: expected array of {expected}, got {actual}"));
    }
    Ok(())
}

/// Read an array header and return the length (definite-length only).
pub fn read_array_len<'b>(d: &mut Decoder<'b>) -> Result<usize> {
    let len = d.array().map_err(|e| anyhow!("cbor: expected array ({e})"))?
        .ok_or_else(|| anyhow!("cbor: expected definite-length array"))?;
    Ok(len as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_32_bytes_succeeds_on_correct_length() {
        // CBOR for 32-byte string of 0x11s: 0x58 0x20 [32 × 0x11]
        let mut buf = vec![0x58, 0x20];
        buf.extend_from_slice(&[0x11; 32]);
        let mut d = Decoder::new(&buf);
        let out = read_32_bytes(&mut d).unwrap();
        assert_eq!(out, [0x11; 32]);
    }

    #[test]
    fn read_32_bytes_fails_on_wrong_length() {
        // 4-byte string instead of 32.
        let buf = vec![0x44, 0xDE, 0xAD, 0xBE, 0xEF];
        let mut d = Decoder::new(&buf);
        assert!(read_32_bytes(&mut d).is_err());
    }

    #[test]
    fn read_u64_handles_small_int() {
        let buf = vec![0x05]; // CBOR uint 5
        let mut d = Decoder::new(&buf);
        assert_eq!(read_u64(&mut d).unwrap(), 5);
    }

    #[test]
    fn read_array_len_reads_array_header() {
        let buf = vec![0x83]; // CBOR array of 3
        let mut d = Decoder::new(&buf);
        assert_eq!(read_array_len(&mut d).unwrap(), 3);
    }
}
```

Note: pallas-codec re-exports `minicbor`. If the import path differs at the version we end up with, adjust to `use minicbor::Decoder;` or whatever the actual public re-export is. The implementer should check the pallas-codec docs once the dep is available.

- [ ] **Step 2: Write `utxo.rs` — UTXO ingestion**

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/utxo.rs`:

```rust
//! UTXO sub-tree ingestion: simplified CBOR fixture → omega-commitment
//! UTXO list → JSON output for `omega-commitment commit --sub-tree utxo`.
//!
//! Reads the hand-crafted fixture format documented in
//! `tests/fixtures/ledger_state_minimal.cbor.md`. Real Mithril/Cardano
//! LedgerState parsing is future work; the simplified format proves
//! the ingestion → JSON → leaf-hash pipeline.

use crate::cbor::{expect_array, read_32_bytes, read_array_len, read_u64};
use anyhow::Result;
use omega_commitment_core::utxo_leaf::Utxo;
use pallas_codec::minicbor::Decoder;
use serde::Serialize;

/// JSON output shape that matches the input format consumed by
/// `omega-commitment commit --sub-tree utxo`.
#[derive(Debug, Clone, Serialize)]
pub struct UtxoOutput {
    pub utxos: Vec<Utxo>,
}

/// Ingest UTXOs from the simplified CBOR fixture.
///
/// Fixture format (Conway-era LedgerState parsing is future work):
///   CBOR array of N UTXOs, each a 4-element array of:
///     [ tx_id (32 bytes), output_index (u64), address (32 bytes),
///       value_lovelace (u64) ]
pub fn ingest_utxos(cbor: &[u8]) -> Result<UtxoOutput> {
    let mut d = Decoder::new(cbor);
    let n = read_array_len(&mut d)?;
    let mut utxos = Vec::with_capacity(n);
    for _ in 0..n {
        expect_array(&mut d, 4)?;
        let tx_id = read_32_bytes(&mut d)?;
        let output_index = u32::try_from(read_u64(&mut d)?)
            .map_err(|_| anyhow::anyhow!("output_index too large for u32"))?;
        let address_hash = read_32_bytes(&mut d)?;
        let value_lovelace = read_u64(&mut d)?;
        utxos.push(Utxo {
            tx_id,
            output_index,
            address_hash,
            value_lovelace,
            assets: Vec::new(),
            datum_hash: None,
        });
    }
    Ok(UtxoOutput { utxos })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_minimal_cbor() -> Vec<u8> {
        // Same encoding as the test fixture (Task 5).
        // Three UTXOs with deterministic content.
        fn cbor_array_header(len: usize) -> Vec<u8> {
            if len < 24 { vec![0x80u8 | len as u8] }
            else if len < 256 { vec![0x98, len as u8] }
            else { vec![0x99, (len >> 8) as u8, len as u8] }
        }
        fn cbor_bytes_header(len: usize) -> Vec<u8> {
            if len < 24 { vec![0x40u8 | len as u8] }
            else if len < 256 { vec![0x58, len as u8] }
            else { vec![0x59, (len >> 8) as u8, len as u8] }
        }
        fn cbor_uint(v: u64) -> Vec<u8> {
            if v < 24 { vec![v as u8] }
            else if v <= 0xff { vec![0x18, v as u8] }
            else if v <= 0xffff { vec![0x19, (v >> 8) as u8, v as u8] }
            else if v <= 0xffff_ffff {
                let mut o = vec![0x1a];
                o.extend_from_slice(&(v as u32).to_be_bytes());
                o
            } else {
                let mut o = vec![0x1b];
                o.extend_from_slice(&v.to_be_bytes());
                o
            }
        }
        fn cbor_bytes(b: &[u8]) -> Vec<u8> {
            let mut o = cbor_bytes_header(b.len());
            o.extend_from_slice(b);
            o
        }
        fn utxo(tx_id: [u8; 32], oi: u64, addr: [u8; 32], v: u64) -> Vec<u8> {
            let mut o = cbor_array_header(4);
            o.extend(cbor_bytes(&tx_id));
            o.extend(cbor_uint(oi));
            o.extend(cbor_bytes(&addr));
            o.extend(cbor_uint(v));
            o
        }
        let mut buf = Vec::new();
        buf.extend(cbor_array_header(3));
        buf.extend(utxo([0x11; 32], 0, [0xAA; 32], 1_000_000));
        buf.extend(utxo([0x22; 32], 1, [0xBB; 32], 5_000_000));
        buf.extend(utxo([0x33; 32], 2, [0xCC; 32], 250_000_000));
        buf
    }

    #[test]
    fn ingest_minimal_fixture() {
        let cbor = make_minimal_cbor();
        let out = ingest_utxos(&cbor).unwrap();
        assert_eq!(out.utxos.len(), 3);
        assert_eq!(out.utxos[0].tx_id, [0x11; 32]);
        assert_eq!(out.utxos[0].output_index, 0);
        assert_eq!(out.utxos[0].value_lovelace, 1_000_000);
        assert_eq!(out.utxos[2].value_lovelace, 250_000_000);
        assert!(out.utxos.iter().all(|u| u.assets.is_empty()));
        assert!(out.utxos.iter().all(|u| u.datum_hash.is_none()));
    }

    #[test]
    fn ingest_then_leaf_hashes_are_deterministic() {
        let cbor = make_minimal_cbor();
        let out1 = ingest_utxos(&cbor).unwrap();
        let out2 = ingest_utxos(&cbor).unwrap();
        let h1: Vec<_> = out1.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
        let h2: Vec<_> = out2.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
        assert_eq!(h1, h2);
    }

    #[test]
    fn ingest_truncated_input_fails() {
        let cbor = make_minimal_cbor();
        let truncated = &cbor[..cbor.len() / 2];
        assert!(ingest_utxos(truncated).is_err());
    }
}
```

- [ ] **Step 3: Update `lib.rs`**

Replace `/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/lib.rs` contents with:

```rust
//! omega-commitment-ingest: transforms real Cardano data (CBOR
//! LedgerState snapshots, chain history) into the per-sub-tree JSON
//! formats consumed by `omega-commitment commit`.
//!
//! v0.8.0 implements UTXO ingestion end-to-end and scaffolds the
//! other four LedgerState-derivable sub-trees (token-policy, script,
//! stake, governance). Header and tx-index ingestion is documented
//! as future work requiring a chain-follower.

pub mod cbor;
pub mod utxo;
```

- [ ] **Step 4: Verify**

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
cargo test -p omega-commitment-ingest 2>&1 | tail -10   # 7 tests pass (4 cbor + 3 utxo)
cargo test --workspace 2>&1 | tail -5                     # 185 total
cargo lint 2>&1 | tail -3                                 # clean
cargo fmt-check 2>&1 | tail -3                            # clean
```

If pallas-codec's minicbor re-export path differs from `pallas_codec::minicbor::Decoder`, adjust the imports in `cbor.rs` and `utxo.rs` to match what's actually exposed (try `pallas_codec::minicbor` first; if that fails, try direct dep on `minicbor` with a workspace dep `minicbor = "0.20"` or whatever pallas-codec uses internally).

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-ingest/src/cbor.rs \
        crates/omega-commitment-ingest/src/utxo.rs \
        crates/omega-commitment-ingest/src/lib.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(ingest): UTXO ingestion from simplified CBOR fixture (pallas-codec)"
```

---

## Task 7: Scaffold the four other LedgerState-derivable sub-tree ingestion modules

**Files:**
- Create: `crates/omega-commitment-ingest/src/token_policy.rs`
- Create: `crates/omega-commitment-ingest/src/script.rs`
- Create: `crates/omega-commitment-ingest/src/stake.rs`
- Create: `crates/omega-commitment-ingest/src/governance.rs`
- Modify: `crates/omega-commitment-ingest/src/lib.rs`

Establish the function signatures, JSON output shapes, and `#[ignore]`d test stubs for the other four ingestion paths. Each function returns `unimplemented!()` with a clear message pointing to the follow-up plan. The structure is in place; the bodies are honest about being future work.

- [ ] **Step 1: Write the four scaffold files**

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/token_policy.rs`:

```rust
//! Token-policy sub-tree ingestion (SCAFFOLD — future work).
//!
//! Will derive token policies from the multi-asset bundles in
//! UTXOs. Requires real Conway LedgerState parsing, which is gated
//! on the follow-up `omega-commitment-ingest-mainnet` plan.

use anyhow::Result;
use omega_commitment_core::token_policy_leaf::TokenPolicy;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TokenPolicyOutput {
    pub policies: Vec<TokenPolicy>,
}

pub fn ingest_token_policies(_cbor: &[u8]) -> Result<TokenPolicyOutput> {
    unimplemented!(
        "Token-policy ingestion not yet implemented. \
         Requires real Conway LedgerState parsing — gated on the \
         follow-up `omega-commitment-ingest-mainnet` plan. \
         The simplified fixture format used in v0.8.0 does not \
         carry multi-asset data."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "scaffold: requires real Conway LedgerState parsing"]
    fn ingest_token_policies_minimal_fixture() {
        let _ = ingest_token_policies(&[0x80]);
    }
}
```

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/script.rs`:

```rust
//! Script-registry sub-tree ingestion (SCAFFOLD — future work).
//!
//! Will derive script entries from UTXO script credentials and
//! reference scripts. Requires real Conway LedgerState parsing.

use anyhow::Result;
use omega_commitment_core::script_registry_leaf::ScriptEntry;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ScriptOutput {
    pub scripts: Vec<ScriptEntry>,
}

pub fn ingest_scripts(_cbor: &[u8]) -> Result<ScriptOutput> {
    unimplemented!(
        "Script ingestion not yet implemented. \
         Requires real Conway LedgerState parsing — gated on the \
         follow-up `omega-commitment-ingest-mainnet` plan."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "scaffold: requires real Conway LedgerState parsing"]
    fn ingest_scripts_minimal_fixture() {
        let _ = ingest_scripts(&[0x80]);
    }
}
```

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/stake.rs`:

```rust
//! Stake-state sub-tree ingestion (SCAFFOLD — future work).
//!
//! Will derive stake entries from the LedgerState stake snapshot,
//! pool registrations, and DRep delegations. Requires real Conway
//! LedgerState parsing.

use anyhow::Result;
use omega_commitment_core::stake_state_leaf::StakeEntry;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct StakeOutput {
    pub stake_entries: Vec<StakeEntry>,
}

pub fn ingest_stake(_cbor: &[u8]) -> Result<StakeOutput> {
    unimplemented!(
        "Stake ingestion not yet implemented. \
         Requires real Conway LedgerState parsing — gated on the \
         follow-up `omega-commitment-ingest-mainnet` plan."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "scaffold: requires real Conway LedgerState parsing"]
    fn ingest_stake_minimal_fixture() {
        let _ = ingest_stake(&[0x80]);
    }
}
```

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/governance.rs`:

```rust
//! Governance-state sub-tree ingestion (SCAFFOLD — future work).
//!
//! Will derive governance facts from the LedgerState treasury,
//! Constitutional Committee, and gov-action records. Requires real
//! Conway LedgerState parsing.

use anyhow::Result;
use omega_commitment_core::governance_state_leaf::GovernanceFact;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GovernanceOutput {
    pub facts: Vec<GovernanceFact>,
}

pub fn ingest_governance(_cbor: &[u8]) -> Result<GovernanceOutput> {
    unimplemented!(
        "Governance ingestion not yet implemented. \
         Requires real Conway LedgerState parsing — gated on the \
         follow-up `omega-commitment-ingest-mainnet` plan."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "scaffold: requires real Conway LedgerState parsing"]
    fn ingest_governance_minimal_fixture() {
        let _ = ingest_governance(&[0x80]);
    }
}
```

- [ ] **Step 2: Update `lib.rs`**

Replace `/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/lib.rs` contents with:

```rust
//! omega-commitment-ingest: transforms real Cardano data (CBOR
//! LedgerState snapshots, chain history) into the per-sub-tree JSON
//! formats consumed by `omega-commitment commit`.
//!
//! v0.8.0 implements UTXO ingestion end-to-end and scaffolds the
//! other four LedgerState-derivable sub-trees (token-policy, script,
//! stake, governance) — those four return `unimplemented!()` with a
//! pointer to the follow-up `omega-commitment-ingest-mainnet` plan.
//! Header and tx-index ingestion is documented as future work
//! requiring a chain-follower.

pub mod cbor;
pub mod utxo;
pub mod token_policy;
pub mod script;
pub mod stake;
pub mod governance;
```

- [ ] **Step 3: Verify**

```bash
cargo test -p omega-commitment-ingest 2>&1 | tail -15
```

Expected output should mention 7 passing tests (cbor + utxo from Task 6) plus 4 ignored tests (one per scaffold module). Total workspace count: still 185 (ignored tests don't change the pass count).

```bash
cargo test --workspace 2>&1 | tail -5    # 185 total, with 4 ignored
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-ingest/src/token_policy.rs \
        crates/omega-commitment-ingest/src/script.rs \
        crates/omega-commitment-ingest/src/stake.rs \
        crates/omega-commitment-ingest/src/governance.rs \
        crates/omega-commitment-ingest/src/lib.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(ingest): scaffold token_policy/script/stake/governance modules (unimplemented)"
```

---

## Task 8: CLI binary + UTXO subcommand + end-to-end QA pipeline test

**Files:**
- Modify: `crates/omega-commitment-ingest/src/main.rs`
- Create: `crates/omega-commitment-ingest/tests/utxo_ingest_integration.rs`
- Create: `crates/omega-commitment-ingest/tests/qa_pipeline.rs`

The `omega-ingest` binary with one working subcommand (`utxo`) and four scaffolded ones that fail loudly. End-to-end test: hand-crafted CBOR → ingest → JSON → leaf hashes → root.

- [ ] **Step 1: Write `main.rs`**

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/main.rs`:

```rust
//! omega-ingest CLI.
//!
//! Per-sub-tree subcommands that take Cardano source data (CBOR) and
//! emit the JSON format consumed by `omega-commitment commit`.
//!
//! v0.8.0: only `utxo` is fully implemented; the other four are
//! scaffolded and will fail with a clear message pointing to the
//! follow-up plan.

use anyhow::Result;
use clap::{Parser, Subcommand};
use omega_commitment_ingest::{
    governance::ingest_governance, script::ingest_scripts, stake::ingest_stake,
    token_policy::ingest_token_policies, utxo::ingest_utxos,
};
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(name = "omega-ingest", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Ingest UTXOs from a CBOR snapshot and emit the JSON format
    /// consumed by `omega-commitment commit --sub-tree utxo`.
    Utxo {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// SCAFFOLD: token policy ingestion is not yet implemented.
    TokenPolicy {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// SCAFFOLD: script ingestion is not yet implemented.
    Script {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// SCAFFOLD: stake ingestion is not yet implemented.
    Stake {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// SCAFFOLD: governance ingestion is not yet implemented.
    Governance {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Utxo { input, output } => run_utxo(input, output),
        Cmd::TokenPolicy { input, output } => run_token_policy(input, output),
        Cmd::Script { input, output } => run_script(input, output),
        Cmd::Stake { input, output } => run_stake(input, output),
        Cmd::Governance { input, output } => run_governance(input, output),
    }
}

fn run_utxo(input: PathBuf, output: PathBuf) -> Result<()> {
    let cbor = fs::read(&input)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {}", input.display(), e))?;
    let out = ingest_utxos(&cbor)?;
    fs::write(&output, serde_json::to_string_pretty(&out)?)?;
    println!("ok: ingested {} utxos -> {}", out.utxos.len(), output.display());
    Ok(())
}

fn run_token_policy(input: PathBuf, _output: PathBuf) -> Result<()> {
    let cbor = fs::read(&input)?;
    let _ = ingest_token_policies(&cbor)?;
    Ok(())
}

fn run_script(input: PathBuf, _output: PathBuf) -> Result<()> {
    let cbor = fs::read(&input)?;
    let _ = ingest_scripts(&cbor)?;
    Ok(())
}

fn run_stake(input: PathBuf, _output: PathBuf) -> Result<()> {
    let cbor = fs::read(&input)?;
    let _ = ingest_stake(&cbor)?;
    Ok(())
}

fn run_governance(input: PathBuf, _output: PathBuf) -> Result<()> {
    let cbor = fs::read(&input)?;
    let _ = ingest_governance(&cbor)?;
    Ok(())
}
```

- [ ] **Step 2: Write `utxo_ingest_integration.rs`**

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/utxo_ingest_integration.rs`:

```rust
//! UTXO ingestion end-to-end against the hand-crafted CBOR fixture.

use omega_commitment_core::tree::MerkleTree;
use omega_commitment_ingest::utxo::ingest_utxos;
use std::{fs, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ledger_state_minimal.cbor")
}

#[test]
fn ingest_minimal_cbor_fixture() {
    let cbor = fs::read(fixture_path()).expect("fixture readable");
    let out = ingest_utxos(&cbor).unwrap();
    assert_eq!(out.utxos.len(), 3);
    assert_eq!(out.utxos[0].tx_id, [0x11; 32]);
    assert_eq!(out.utxos[1].tx_id, [0x22; 32]);
    assert_eq!(out.utxos[2].tx_id, [0x33; 32]);
}

#[test]
fn ingest_then_build_tree_succeeds() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_utxos(&cbor).unwrap();
    let leaves: Vec<_> = out.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    let tree = MerkleTree::build(leaves);
    // 3 utxos pads to 4 leaves at depth 2.
    assert_eq!(tree.leaf_count(), 4);
    assert_eq!(tree.depth(), 2);
    assert_ne!(tree.root(), [0u8; 32]);
}

#[test]
fn ingest_to_json_matches_per_sub_tree_cli_format() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_utxos(&cbor).unwrap();
    let json = serde_json::to_string(&out).unwrap();
    // The JSON shape must match what `omega-commitment commit --sub-tree utxo`
    // accepts (namely `{"utxos": [...]}`).
    assert!(json.contains("\"utxos\":"), "JSON should have utxos field: {json}");
    // Round-trip back through serde to confirm the format is stable.
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["utxos"].is_array());
}
```

- [ ] **Step 3: Write `qa_pipeline.rs`**

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/qa_pipeline.rs`:

```rust
//! End-to-end QA pipeline: hand-crafted CBOR → omega-ingest → JSON →
//! omega-commitment commit (in-process) → leaf hashes + root.
//!
//! Proves the entire ingestion → commitment pipeline works on a
//! CBOR-shaped input. Validates that the CBOR fixture, the ingestion
//! library, and the commitment-core library all agree.

use omega_commitment_core::tree::MerkleTree;
use omega_commitment_ingest::utxo::ingest_utxos;
use std::{fs, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ledger_state_minimal.cbor")
}

#[test]
fn full_pipeline_cbor_to_root() {
    // 1) Read CBOR fixture.
    let cbor = fs::read(fixture_path()).unwrap();

    // 2) Ingest into the per-sub-tree JSON shape.
    let out = ingest_utxos(&cbor).unwrap();
    assert_eq!(out.utxos.len(), 3);

    // 3) Compute leaf hashes (what `omega-commitment commit` would do).
    let leaves: Vec<_> = out.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();

    // 4) Build the Merkle tree.
    let tree = MerkleTree::build(leaves.clone());
    let root = tree.root();
    assert_ne!(root, [0u8; 32]);

    // 5) Round-trip via JSON: write to a temp dir, then parse back, then verify
    //    the round-tripped UTXOs hash to the same leaves.
    use omega_commitment_core::utxo_leaf::Utxo;
    use serde::Deserialize;
    #[derive(Deserialize)]
    struct Roundtrip { utxos: Vec<Utxo> }
    let json = serde_json::to_string(&out).unwrap();
    let parsed: Roundtrip = serde_json::from_str(&json).unwrap();
    let leaves_after: Vec<_> = parsed.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    assert_eq!(leaves, leaves_after, "JSON round-trip must preserve leaf hashes");

    // 6) Tree built from the round-tripped UTXOs has the same root.
    let tree_after = MerkleTree::build(leaves_after);
    assert_eq!(tree.root(), tree_after.root());
}
```

- [ ] **Step 4: Verify + manual smoke**

```bash
cargo test -p omega-commitment-ingest 2>&1 | tail -15
cargo test --workspace 2>&1 | tail -5    # 188 total (185 prior + 3 utxo_ingest_integration + 1 qa_pipeline; verify exact count)
cargo lint 2>&1 | tail -3                  # clean
cargo fmt-check 2>&1 | tail -3             # clean
```

Manual CLI smoke:
```bash
cargo build --release -p omega-commitment-ingest
mkdir -p /tmp/o-ingest && rm -rf /tmp/o-ingest/*
./target/release/omega-ingest utxo \
  --input crates/omega-commitment-ingest/tests/fixtures/ledger_state_minimal.cbor \
  --output /tmp/o-ingest/utxos.json
cat /tmp/o-ingest/utxos.json
# Pipe through omega-commitment commit:
./target/release/omega-commitment commit --sub-tree utxo \
  --input /tmp/o-ingest/utxos.json --output /tmp/o-ingest/commit
cat /tmp/o-ingest/commit/commitment.json
```

Expected:
- `omega-ingest utxo` prints `ok: ingested 3 utxos -> /tmp/o-ingest/utxos.json`
- `utxos.json` contains 3 entries.
- `omega-commitment commit` prints `ok: ...` with a non-zero root.

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-ingest/src/main.rs \
        crates/omega-commitment-ingest/tests/utxo_ingest_integration.rs \
        crates/omega-commitment-ingest/tests/qa_pipeline.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(ingest-cli): omega-ingest binary + UTXO end-to-end QA pipeline"
```

---

## Task 9: Mithril snapshot download script

**Files:**
- Create: `scripts/download_snapshot.sh`

A human-invoked script that downloads the latest Mithril preview-testnet snapshot. NOT invoked by tests. Documented as part of manual real-data QA workflow.

- [ ] **Step 1: Write the script**

`/home/hoskinson/omega-commitment/scripts/download_snapshot.sh`:

```bash
#!/usr/bin/env bash
#
# Download a recent Mithril-attested Cardano snapshot for manual QA.
#
# Usage:
#   ./scripts/download_snapshot.sh [aggregator-url]
#
# Default: pre-release-preview Mithril aggregator (testnet, smaller
# than mainnet — appropriate for ingestion experiments).
#
# Output:
#   var/snapshots/<digest>/  — extracted snapshot directory
#
# This script is HUMAN-INVOKED only. Tests do NOT call it; they use
# the in-tree hand-crafted CBOR fixture.

set -euo pipefail

AGGREGATOR_URL="${1:-https://aggregator.pre-release-preview.api.mithril.network/aggregator}"
DEST_DIR="$(cd "$(dirname "$0")/.." && pwd)/var/snapshots"

echo "Mithril aggregator: $AGGREGATOR_URL"
echo "Destination:        $DEST_DIR"
mkdir -p "$DEST_DIR"

echo
echo "== Fetching snapshot list =="
SNAPSHOT_JSON="$(curl -fsSL "$AGGREGATOR_URL/snapshots")"

# Pick the most recent snapshot.
DIGEST="$(echo "$SNAPSHOT_JSON" | python3 -c "import json,sys; data=json.load(sys.stdin); print(data[0]['digest'])")"
SIZE_GB="$(echo "$SNAPSHOT_JSON" | python3 -c "import json,sys; data=json.load(sys.stdin); print(round(data[0]['size']/1024/1024/1024,2))")"

echo "Latest digest: $DIGEST"
echo "Approx size:   ${SIZE_GB} GB"

if [[ -d "$DEST_DIR/$DIGEST" ]]; then
  echo
  echo "Snapshot $DIGEST already present at $DEST_DIR/$DIGEST"
  echo "Skipping download. To re-download: rm -rf $DEST_DIR/$DIGEST"
  exit 0
fi

echo
echo "== Downloading snapshot =="
DOWNLOAD_URL="$(echo "$SNAPSHOT_JSON" | python3 -c "import json,sys; data=json.load(sys.stdin); print(data[0]['locations'][0])")"
echo "URL: $DOWNLOAD_URL"

mkdir -p "$DEST_DIR/$DIGEST"
curl -fSL --progress-bar "$DOWNLOAD_URL" -o "$DEST_DIR/$DIGEST/snapshot.tar.gz"

echo
echo "== Extracting =="
tar -xzf "$DEST_DIR/$DIGEST/snapshot.tar.gz" -C "$DEST_DIR/$DIGEST/"
rm "$DEST_DIR/$DIGEST/snapshot.tar.gz"
echo "Extracted to $DEST_DIR/$DIGEST/"

echo
echo "== Done =="
echo "Snapshot ready at: $DEST_DIR/$DIGEST/"
echo
echo "NOTE: omega-ingest's UTXO subcommand currently parses a"
echo "      simplified hand-crafted CBOR fixture, NOT the real"
echo "      Conway LedgerState shape. Real LedgerState ingestion"
echo "      lands in the follow-up omega-commitment-ingest-mainnet plan."
```

- [ ] **Step 2: Make it executable**

```bash
chmod +x scripts/download_snapshot.sh
```

- [ ] **Step 3: Verify the script syntax (bash -n) without running it**

```bash
bash -n scripts/download_snapshot.sh
echo "syntax check: $?"
```

Expected: exit 0.

DO NOT actually run the script in CI / planning execution — it downloads multi-GB data over the network. Manual invocation only.

- [ ] **Step 4: Commit**

```bash
git add scripts/download_snapshot.sh
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "tools: human-invoked Mithril snapshot downloader for manual real-data QA"
```

---

## Task 10: Bump workspace to v0.8.0 + extend README

**Files:**
- Modify: `crates/omega-commitment-core/Cargo.toml`
- Modify: `crates/omega-commitment-cli/Cargo.toml`
- Modify: `crates/omega-commitment-bundle/Cargo.toml`
- Modify: `crates/omega-commitment-ingest/Cargo.toml` (already 0.8.0; confirm)
- Modify: `README.md`

- [ ] **Step 1: Bump existing crate versions**

In each of the first three crate `Cargo.toml` files: change `version = "0.7.0"` to `version = "0.8.0"`.

In `crates/omega-commitment-ingest/Cargo.toml`: confirm it already says `version = "0.8.0"` (set in Task 4).

- [ ] **Step 2: Verify**

```bash
cargo build --workspace 2>&1 | grep -E "warning|error"   # empty
cargo lint 2>&1 | tail -3                                  # clean
cargo fmt-check 2>&1 | tail -3                             # clean
cargo test --workspace 2>&1 | tail -5                       # 188 tests pass
```

- [ ] **Step 3: Append to README.md**

Append to `/home/hoskinson/omega-commitment/README.md`:

```markdown
## v0.8.0 — Cardano ingestion + Golden Vector QA suite

Adds the **`omega-commitment-ingest`** crate (4th workspace member) with binary `omega-ingest` for transforming Cardano data into the per-sub-tree JSON formats. Also locks **canonical golden vectors** across every existing component so future encoding regressions are caught immediately.

### Honest scope

- ✅ **UTXO ingestion** — fully implemented end-to-end against a hand-crafted minimal CBOR fixture committed in-tree. Pipeline: CBOR → `omega-ingest utxo` → JSON → `omega-commitment commit --sub-tree utxo` → root.
- ✅ **Golden vectors** — pinned per-sub-tree roots for all seven synthetic fixtures, plus the canonical Ω-Commitment bundle root tuple.
- 🟡 **Other 4 LedgerState-derivable sub-trees** (token-policy, script, stake, governance) — scaffolded with `unimplemented!()` and `#[ignore]`d test stubs. Real implementation requires Conway-era LedgerState parsing against `pallas` and is gated on the follow-up `omega-commitment-ingest-mainnet` plan.
- ❌ **Header + tx-index ingestion** — requires a chain-follower (multi-day operation, separate from LedgerState parsing). Out of scope for v0.8.0.

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
omega-ingest token-policy --input ...   # SCAFFOLD — unimplemented
omega-ingest script --input ...         # SCAFFOLD — unimplemented
omega-ingest stake --input ...          # SCAFFOLD — unimplemented
omega-ingest governance --input ...     # SCAFFOLD — unimplemented
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
| Per-sub-tree leaf encoders + Merkle trees + CLIs | ✅ Shipped (v0.1.0–v0.6.0) |
| Bundle assembly + verification | ✅ Shipped (v0.7.0) |
| Golden vector QA suite (synthetic + bundle) | ✅ Shipped (v0.8.0) |
| UTXO ingestion from hand-crafted CBOR | ✅ Shipped (v0.8.0) |
| Other 4 LedgerState-derivable sub-tree ingestion | 🟡 Scaffolded — follow-up plan |
| Header + tx-index ingestion (chain-follower) | ❌ Future plan |
| Real Mithril snapshot end-to-end | 🟡 Manual via download script |

### Sub-trees status (unchanged from v0.7.0)

| # | Sub-tree | Status |
|---|---|---|
| 1 | UTXO set | Shipped (v0.1.0) + UTXO ingestion v0.8.0 |
| 2 | Block header chain | Shipped (v0.2.0) — ingestion: future |
| 3 | Transaction index | Shipped (v0.3.0) — ingestion: future |
| 4 | Native token policies | Shipped (v0.4.0) — ingestion: scaffolded |
| 5 | Script registry | Shipped (v0.5.0) — ingestion: scaffolded |
| 6 | Stake state | Shipped (v0.6.0) — ingestion: scaffolded |
| 7 | Governance state | Shipped (v0.6.0) — ingestion: scaffolded |
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-core/Cargo.toml \
        crates/omega-commitment-cli/Cargo.toml \
        crates/omega-commitment-bundle/Cargo.toml \
        crates/omega-commitment-ingest/Cargo.toml \
        README.md
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "chore: bump workspace to 0.8.0; document Cardano ingestion + Golden Vector QA"
```

- [ ] **Step 5: Final verification**

```bash
git log --oneline | head -12
cargo test --workspace 2>&1 | tail -5
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

Expected: HEAD is the version-bump commit, 188 tests pass, lint+fmt clean.

---

## Self-review

**Coverage of the user's ask:**
- ✅ Real Cardano ingestion (option C) — UTXO end-to-end, four others scaffolded.
- ✅ Download snapshots for testing — `download_snapshot.sh` provided.
- ✅ QA each component — golden vectors pinned across `omega-commitment-core` (per-sub-tree), `omega-commitment-bundle` (canonical bundle root tuple), and `omega-commitment-ingest` (CBOR pipeline).
- ✅ Golden test vectors — pinned in three dedicated test files.
- 🟡 The 4 non-UTXO ingestion paths are scaffolded, not fully implemented. This is the honest scope for one execution pass; pretending otherwise sets up false expectations and untestable code.

**Decision honoring:**
- ✅ PQ-only crypto, Plonky3-friendly tree, selective dual-track, lazy/pull migration — all unchanged.

**Placeholder scan:** All code blocks runnable. The hex placeholders in Task 1 Step 2 are explicitly part of a documented "fail-once-then-pin" workflow; the value extraction process is concrete. No "TBD" / "fill in later" in implementation steps. ✅

**Type consistency:**
- `UtxoOutput` / `TokenPolicyOutput` / `ScriptOutput` / `StakeOutput` / `GovernanceOutput` — consistent suffix, consistent shape (single-field struct matching the per-sub-tree CLI's expected JSON top-level field).
- `ingest_utxos`, `ingest_token_policies`, `ingest_scripts`, `ingest_stake`, `ingest_governance` — consistent function naming.
- `omega-ingest` subcommand names match per-sub-tree CLI conventions (utxo, token-policy, script, stake, governance).
- Pallas dep version pinned to 0.30 across all three pallas crates; if 0.30 isn't available, the executor should pick the latest stable 0.x and document the choice.
- ✅ No drift.

**Bite-sized tasks:** 10 tasks, each independently committable, each with 2–7 numbered steps.

**Net delta:** +14 tests (7 sub-tree golden + 1 witness golden + 3 bundle golden + cbor unit + utxo unit + 3 utxo integration + 1 qa pipeline) (~167 → ~188). 10 commits.

---

## What's NOT in this plan (and why)

- **Real Conway-era LedgerState CBOR parsing.** The four non-UTXO ingestion paths are scaffolded; their real implementation is gated on `pallas`'s LedgerState support (which is partial as of pallas 0.30) and is the immediate next plan in this track (`omega-commitment-ingest-mainnet`).
- **Chain-follower for header + tx-index sub-trees.** Requires running a Cardano node or pallas-network synchronization — multi-day, multi-GB. Separate plan in a v1.0 cycle.
- **Auto-downloading snapshots in CI.** Multi-GB and would make tests flaky. The download script is human-invoked only.
- **Mainnet snapshot ingestion** (vs. testnet). Mainnet is even bigger; testnet is the right starting point for QA.
- **Mithril certificate verification.** The download script downloads but does NOT verify the Mithril attestation. Adding verification is a future hardening task.

---

## How to execute this plan

Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Ten tasks, each independently committable.

Total runway estimate: **5–8 days** for an experienced Rust dev. The pallas integration adds discovery cost in Task 6 (CBOR API surface); other tasks are mechanical.

Expected post-execution state:
- 10 commits added on top of v0.7.0 (currently 64 commits)
- ~21 net new tests (188 total, 4 ignored as scaffolds)
- All four crates at version 0.8.0
- New `omega-ingest` binary alongside `omega-commitment` and `omega-bundle`
- `scripts/download_snapshot.sh` ready for manual real-data QA
- Three golden-vector test files locking canonical outputs
- Track T1 ingestion sub-phase 25%-complete (1 of 4 LedgerState-derivable sub-trees fully done; other 3 scaffolded; chain-follower-required ones documented as future)

Next plan in this track: `2026-XX-XX-omega-ingest-mainnet-plan.md` — implement the four scaffolded ingestion paths against real Conway-era LedgerState CBOR via pallas-traverse, lock golden vectors against a real Mithril snapshot.
