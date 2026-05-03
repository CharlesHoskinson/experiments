//! Per-leaf golden vectors — closes audit finding A4/F002 (2026-05-03).
//!
//! For each of the seven Ω-Commitment sub-trees, this file pins:
//!   1. the canonical encoded bytes of one example leaf, and
//!   2. the v1 domain-separated leaf hash (`leaf_hash_v2(SUB_TREE_ID, 0, &payload)`)
//!      computed against those exact bytes.
//!
//! The seven main cases are the smallest possible "first leaf" for each
//! sub-tree; the three edge cases (empty UTXO set → padded root, single-
//! leaf UTXO set, AlwaysAbstain DRep stake leaf) are pinned alongside so
//! that future refactors that change leaf encoding will fail loudly here
//! before they reach the per-sub-tree integration goldens.
//!
//! ## Why pinning both bytes AND leaf hash matters
//!
//! Pinning only the leaf hash hides whether the canonical-encoding spec
//! has drifted: a different encoder that happens to produce a different
//! payload could collide on the same hash for a different reason. Pinning
//! the encoded bytes locks the encoder; pinning the hash locks the v1
//! domain tag (`omega:v2:leaf` || sub_tree_id || canonical_index ||
//! payload_len || payload). Failure of either assertion is a regression.
//!
//! ## Edge-case fixture corpus deferral (A4/F003)
//!
//! Full edge-case coverage (Byron addresses, pointer addresses, malformed
//! CBOR, large/deep trees, non-UTF8 asset names, AlwaysNoConfidence DReps,
//! inline datums × all script-language combinations) is a multi-day task
//! tracked as a v1.1 fixture-expansion follow-up. The three edge cases
//! pinned here (empty set, single-leaf set, AlwaysAbstain DRep) are the
//! minimal subset that the existing v0.9.x code can satisfy without
//! enlarging the synthetic fixture corpus. See `tests/fixtures/` and the
//! `EDGE_CASE_FIXTURE_TODO` constant below.
//!
//! ## Re-pinning procedure
//!
//! If a leaf encoding changes intentionally (B5+), regenerate the golden
//! values by running:
//!   `cargo test -p omega-commitment-core --test golden_per_leaf -- --nocapture print_actual_values`
//! and pasting the printed hex into the constants below.

use omega_commitment_core::{
    governance_state_leaf::GovernanceFact,
    header_leaf::BlockHeader,
    script_registry_leaf::ScriptEntry,
    stake_state_leaf::{DrepDelegation, StakeEntry},
    token_policy_leaf::TokenPolicy,
    tree::{leaf_hash_v2, MerkleTree, EMPTY_INDEX_SENTINEL},
    tx_index_leaf::TxIndexEntry,
    utxo_leaf::{DatumOption, Utxo},
    SUB_TREE_ID_GOVERNANCE, SUB_TREE_ID_HEADER, SUB_TREE_ID_SCRIPT, SUB_TREE_ID_STAKE,
    SUB_TREE_ID_TOKEN_POLICY, SUB_TREE_ID_TX_INDEX, SUB_TREE_ID_UTXO,
};

/// Tracker for the deferred edge-case corpus expansion (A4/F003).
/// Listed here so a `grep` for the marker locates the deferred work.
#[allow(dead_code)]
const EDGE_CASE_FIXTURE_TODO: &str = "\
TODO(v1.1): expand tests/fixtures/ corpus with:
- Byron / bootstrap addresses
- Pointer addresses (TxIx > 16 bits)
- Inline datums × {PlutusV1, PlutusV2, PlutusV3, Native}
- Reference scripts × all four script languages
- AlwaysNoConfidence DRep delegation
- Non-UTF-8 asset names (raw byte preservation)
- Maximum-depth trees (≥ 2^16 leaves)
- Malformed CBOR (truncated, indefinite-length, wrong major type)
A4/F003 is partially closed by the three edge cases pinned in this file
(empty set, single-leaf set, AlwaysAbstain DRep); full closure is tracked
as a v1.1 fixture-expansion task.
";

// ---------------------------------------------------------------------------
// Per-sub-tree golden leaves: one example leaf per sub-tree.
// ---------------------------------------------------------------------------

#[test]
fn golden_utxo_leaf() {
    let utxo = sample_utxo();
    let payload = utxo.commit_to_subtree().expect("encode utxo");
    let hash = leaf_hash_v2(SUB_TREE_ID_UTXO, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "010101010101010101010101010101010101010101010101010101010101010100000002001d61020202020202020202020202020202020202020202020202020202020000000000002710000000000000",
        "UTXO leaf canonical encoding (sub-tree id=1) drifted; if this is intentional, regenerate per the file's re-pin procedure"
    );
    assert_eq!(
        hex::encode(hash),
        "374dfc378126879ad1932f7e76a8f700c7b3a8317ff2b57e77a00d7b902ec1f9",
        "UTXO leaf hash drifted under v1 domain tag (sub_tree_id=1, canonical_index=0)"
    );
}

#[test]
fn golden_header_leaf() {
    let header = sample_header();
    let payload = header.commit_to_subtree();
    let hash = leaf_hash_v2(SUB_TREE_ID_HEADER, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "0000000000000001000000000000000111000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "Header leaf canonical encoding (sub-tree id=2) drifted"
    );
    assert_eq!(
        hex::encode(hash),
        "bfc2cb4622f0547b193ce4037583092e67609e77d8d327c580fa54acab55bf34",
        "Header leaf hash drifted under v1 domain tag (sub_tree_id=2, canonical_index=0)"
    );
}

#[test]
fn golden_tx_index_leaf() {
    let entry = sample_tx_index();
    let payload = entry.commit_to_subtree();
    let hash = leaf_hash_v2(SUB_TREE_ID_TX_INDEX, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "11000000000000000000000000000000000000000000000000000000000000000000000000000001aa0000000000000000000000000000000000000000000000000000000000000000000000",
        "TxIndex leaf canonical encoding (sub-tree id=3) drifted"
    );
    assert_eq!(
        hex::encode(hash),
        "dc77fc6f46b708cf90e36b59b79d47ad7ccacc1aceb8066e4cdeed32bcf9c82c",
        "TxIndex leaf hash drifted under v1 domain tag (sub_tree_id=3, canonical_index=0)"
    );
}

#[test]
fn golden_token_policy_leaf() {
    let policy = sample_token_policy();
    let payload = policy.commit_to_subtree();
    let hash = leaf_hash_v2(SUB_TREE_ID_TOKEN_POLICY, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "11000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000001",
        "TokenPolicy leaf canonical encoding (sub-tree id=4) drifted"
    );
    assert_eq!(
        hex::encode(hash),
        "ca34d8906b7b478a7c24ba8c39bbd3472ecba7e007ffd5c644b4cdeba067e2bc",
        "TokenPolicy leaf hash drifted under v1 domain tag (sub_tree_id=4, canonical_index=0)"
    );
}

#[test]
fn golden_script_leaf() {
    let script = sample_script();
    let payload = script.commit_to_subtree();
    let hash = leaf_hash_v2(SUB_TREE_ID_SCRIPT, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "1100000000000000000000000000000000000000000000000000000000000000000000010000000102",
        "Script leaf canonical encoding (sub-tree id=5) drifted"
    );
    assert_eq!(
        hex::encode(hash),
        "e871f79d895d85723956f49348258946add01a0a2767442ed1dd3ff5be4d1204",
        "Script leaf hash drifted under v1 domain tag (sub_tree_id=5, canonical_index=0)"
    );
}

#[test]
fn golden_stake_leaf() {
    // Stake leaf with KeyHash DRep (most common Conway-era variant).
    let stake = sample_stake_keyhash();
    let payload = stake.commit_to_subtree();
    let hash = leaf_hash_v2(SUB_TREE_ID_STAKE, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "111111111111111111111111111111111111111111111111111111112222222222222222222222222222222222222222222222222222222201cccccccccccccccccccccccccccccccccccccccccccccccccccccccc000000000000007b00",
        "Stake leaf canonical encoding (sub-tree id=6, drep=KeyHash) drifted"
    );
    assert_eq!(
        hex::encode(hash),
        "c0b0a45b23f3fc962dfa4aa5670ee2d51f319691ac5b42bf71aaed8479f6eca1",
        "Stake leaf hash drifted under v1 domain tag (sub_tree_id=6, canonical_index=0)"
    );
}

#[test]
fn golden_governance_leaf() {
    // Governance leaf for AccountState pots (the new variant added in B2).
    let fact = sample_account_state();
    let payload = fact.commit_to_subtree();
    let hash = leaf_hash_v2(SUB_TREE_ID_GOVERNANCE, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "0400000005d21dba00000000000016e360000000000011bef80000000000000bb8",
        "Governance AccountState leaf canonical encoding (sub-tree id=7) drifted"
    );
    assert_eq!(
        hex::encode(hash),
        "0e0153fd18192852fd46b077be9b584d6068bee0130fffb2258586fd51cba418",
        "Governance AccountState leaf hash drifted under v1 domain tag (sub_tree_id=7, canonical_index=0)"
    );
}

// ---------------------------------------------------------------------------
// Edge cases (partial closure of A4/F003).
// ---------------------------------------------------------------------------

/// Empty set → tree pads to one EMPTY_INDEX_SENTINEL leaf. Pin the
/// padding-leaf hash for the UTXO sub-tree.
#[test]
fn golden_empty_utxo_set_padding_leaf() {
    let pad_hash = leaf_hash_v2(SUB_TREE_ID_UTXO, EMPTY_INDEX_SENTINEL, &[]);
    assert_eq!(
        hex::encode(pad_hash),
        "45c92fb25ad1892b5f4f9af8213ff7bcb9dab7d7d2b375b6f2e92c21dc6ad983",
        "Padding-leaf hash for the empty UTXO set drifted"
    );
    // Building an empty UTXO sub-tree yields a depth-0 root equal to the
    // single padding leaf.
    let tree = MerkleTree::build_v1(SUB_TREE_ID_UTXO, vec![]).unwrap();
    assert_eq!(
        tree.root(),
        pad_hash,
        "Empty UTXO sub-tree root must equal the v1 padding-leaf hash"
    );
}

/// Single-UTXO set → tree has one leaf, no padding (next_power_of_two(1) == 1),
/// depth 0, root == leaf hash.
#[test]
fn golden_single_utxo_tree() {
    let utxo = sample_utxo();
    let payload = utxo.commit_to_subtree().expect("encode utxo");
    let leaf_hash = leaf_hash_v2(SUB_TREE_ID_UTXO, 0, &payload);
    let tree = MerkleTree::build_v1(SUB_TREE_ID_UTXO, vec![payload]).unwrap();
    assert_eq!(tree.leaf_count(), 1, "single-UTXO tree has 1 leaf");
    assert_eq!(
        tree.depth(),
        0,
        "single-UTXO tree is depth 0 (root == leaf)"
    );
    assert_eq!(
        tree.root(),
        leaf_hash,
        "single-UTXO root must equal the v1 leaf hash"
    );
    // Pin the actual root so any silent change to the leaf or builder
    // is caught here as well as in the per-sub-tree integration golden.
    assert_eq!(
        hex::encode(tree.root()),
        "374dfc378126879ad1932f7e76a8f700c7b3a8317ff2b57e77a00d7b902ec1f9",
        "Single-UTXO tree root drifted"
    );
}

/// AlwaysAbstain DRep stake leaf (Conway-era predefined DRep, not a hash).
/// Distinct from the KeyHash sample above; encoded with the no-payload
/// shape (66 bytes total instead of 94).
#[test]
fn golden_stake_leaf_always_abstain_drep() {
    let stake = sample_stake_always_abstain();
    let payload = stake.commit_to_subtree();
    let hash = leaf_hash_v2(SUB_TREE_ID_STAKE, 0, &payload);

    // 66-byte encoding: no DRep payload after the 0x03 tag byte.
    assert_eq!(
        payload.len(),
        66,
        "AlwaysAbstain stake leaf must be 66 bytes (no DRep payload)"
    );
    assert_eq!(
        hex::encode(&payload),
        "111111111111111111111111111111111111111111111111111111112222222222222222222222222222222222222222222222222222222203000000000000007b00",
        "AlwaysAbstain stake leaf canonical encoding drifted"
    );
    assert_eq!(
        hex::encode(hash),
        "59bca0fee8e310f2fda99300d1b324ae5755c8ed13133d992f73d69956ca2b72",
        "AlwaysAbstain stake leaf hash drifted"
    );
}

// ---------------------------------------------------------------------------
// Sample fixtures (deterministic; do not modify without re-pinning).
// ---------------------------------------------------------------------------

fn sample_utxo() -> Utxo {
    Utxo {
        tx_id: [1u8; 32],
        output_index: 2,
        // Canonical Shelley mainnet payment-key address: header byte 0x61
        // (PaymentKeyHash, mainnet) + 28-byte payment-key hash.
        address: {
            let mut a = vec![0x61u8];
            a.extend_from_slice(&[0x02u8; 28]);
            a
        },
        value_lovelace: 10_000,
        assets: vec![],
        datum_option: DatumOption::None,
        script_ref: None,
    }
}

fn sample_header() -> BlockHeader {
    BlockHeader {
        slot: 1,
        block_height: 1,
        block_hash: {
            let mut h = [0u8; 32];
            h[0] = 0x11;
            h
        },
        prev_hash: [0u8; 32],
    }
}

fn sample_tx_index() -> TxIndexEntry {
    TxIndexEntry {
        tx_id: {
            let mut h = [0u8; 32];
            h[0] = 0x11;
            h
        },
        slot: 1,
        block_hash: {
            let mut h = [0u8; 32];
            h[0] = 0xAA;
            h
        },
        tx_position: 0,
    }
}

fn sample_token_policy() -> TokenPolicy {
    TokenPolicy {
        policy_id: {
            let mut p = [0u8; 28];
            p[0] = 0x11;
            p
        },
        first_issuance_slot: 1,
        total_supply_at_h: 1,
    }
}

fn sample_script() -> ScriptEntry {
    ScriptEntry {
        script_hash: {
            let mut s = [0u8; 28];
            s[0] = 0x11;
            s
        },
        deployment_slot: 1,
        script_size_bytes: 1,
        language: 2, // PlutusV2
    }
}

fn sample_stake_keyhash() -> StakeEntry {
    StakeEntry {
        stake_credential_hash: [0x11u8; 28],
        delegated_pool: [0x22u8; 28],
        delegated_drep: DrepDelegation::KeyHash { hash: [0xCCu8; 28] },
        rewards_lovelace: 123,
        is_pool_operator: 0,
    }
}

fn sample_stake_always_abstain() -> StakeEntry {
    StakeEntry {
        stake_credential_hash: [0x11u8; 28],
        delegated_pool: [0x22u8; 28],
        delegated_drep: DrepDelegation::AlwaysAbstain,
        rewards_lovelace: 123,
        is_pool_operator: 0,
    }
}

fn sample_account_state() -> GovernanceFact {
    GovernanceFact::AccountState {
        reserves: 25_000_000_000, // ~25k ADA in lovelace
        treasury: 1_500_000,
        deposits: 1_163_000,
        fee_pot: 3_000,
    }
}

// ---------------------------------------------------------------------------
// Helper: print the actual encodings + hashes for re-pinning.
//
// Usage:
//   cargo test -p omega-commitment-core --test golden_per_leaf \
//     -- --ignored --nocapture print_actual_values
//
// `#[ignore]` so it does not fire under `cargo test`. It exists only as
// a developer aid when a leaf encoding intentionally changes and the
// goldens must be rotated.
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn print_actual_values() {
    fn dump_leaf(label: &str, sub_tree_id: u8, payload: &[u8]) {
        let hash = leaf_hash_v2(sub_tree_id, 0, payload);
        println!("--- {label} (sub_tree_id={sub_tree_id}) ---");
        println!("  payload_hex: {}", hex::encode(payload));
        println!("  leaf_hash:   {}", hex::encode(hash));
    }
    dump_leaf(
        "utxo",
        SUB_TREE_ID_UTXO,
        &sample_utxo().commit_to_subtree().unwrap(),
    );
    dump_leaf(
        "header",
        SUB_TREE_ID_HEADER,
        &sample_header().commit_to_subtree(),
    );
    dump_leaf(
        "tx_index",
        SUB_TREE_ID_TX_INDEX,
        &sample_tx_index().commit_to_subtree(),
    );
    dump_leaf(
        "token_policy",
        SUB_TREE_ID_TOKEN_POLICY,
        &sample_token_policy().commit_to_subtree(),
    );
    dump_leaf(
        "script",
        SUB_TREE_ID_SCRIPT,
        &sample_script().commit_to_subtree(),
    );
    dump_leaf(
        "stake[KeyHash]",
        SUB_TREE_ID_STAKE,
        &sample_stake_keyhash().commit_to_subtree(),
    );
    dump_leaf(
        "stake[AlwaysAbstain]",
        SUB_TREE_ID_STAKE,
        &sample_stake_always_abstain().commit_to_subtree(),
    );
    dump_leaf(
        "governance[AccountState]",
        SUB_TREE_ID_GOVERNANCE,
        &sample_account_state().commit_to_subtree(),
    );

    // Edge: empty padding leaf
    let pad_hash = leaf_hash_v2(SUB_TREE_ID_UTXO, EMPTY_INDEX_SENTINEL, &[]);
    println!("--- utxo padding leaf (EMPTY_INDEX_SENTINEL) ---");
    println!("  leaf_hash:   {}", hex::encode(pad_hash));

    // Edge: single-UTXO tree root
    let payload = sample_utxo().commit_to_subtree().unwrap();
    let tree = MerkleTree::build_v1(SUB_TREE_ID_UTXO, vec![payload]).unwrap();
    println!("--- single-UTXO tree root ---");
    println!("  root:        {}", hex::encode(tree.root()));
}
