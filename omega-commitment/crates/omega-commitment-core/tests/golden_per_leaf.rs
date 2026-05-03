//! Per-leaf golden vectors — closes audit finding A4/F002 (2026-05-03).
//!
//! For each of the seven Ω-Commitment sub-trees, this file pins:
//!   1. the canonical encoded bytes of one example leaf, and
//!   2. the v1 domain-separated leaf hash (`leaf_hash_v1(SUB_TREE_ID, 0, &payload)`)
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
//! domain tag (`omega:v1:leaf` || sub_tree_id || canonical_index ||
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
    tree::{leaf_hash_v1, MerkleTree, EMPTY_INDEX_SENTINEL},
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
    let hash = leaf_hash_v1(SUB_TREE_ID_UTXO, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "010101010101010101010101010101010101010101010101010101010101010100000002001d61020202020202020202020202020202020202020202020202020202020000000000002710000000000000",
        "UTXO leaf canonical encoding (sub-tree id=1) drifted; if this is intentional, regenerate per the file's re-pin procedure"
    );
    assert_eq!(
        hex::encode(hash),
        "0be58afe1163514b9e992763983b18dfc1e1fa7ffae5d21d19229c5fddd78c49",
        "UTXO leaf hash drifted under v1 domain tag (sub_tree_id=1, canonical_index=0)"
    );
}

#[test]
fn golden_header_leaf() {
    let header = sample_header();
    let payload = header.commit_to_subtree();
    let hash = leaf_hash_v1(SUB_TREE_ID_HEADER, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "0000000000000001000000000000000111000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "Header leaf canonical encoding (sub-tree id=2) drifted"
    );
    assert_eq!(
        hex::encode(hash),
        "c065534b00b02248d89c5568b5e132a237dcceeba8b34182acfbc7a4b46638c8",
        "Header leaf hash drifted under v1 domain tag (sub_tree_id=2, canonical_index=0)"
    );
}

#[test]
fn golden_tx_index_leaf() {
    let entry = sample_tx_index();
    let payload = entry.commit_to_subtree();
    let hash = leaf_hash_v1(SUB_TREE_ID_TX_INDEX, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "11000000000000000000000000000000000000000000000000000000000000000000000000000001aa0000000000000000000000000000000000000000000000000000000000000000000000",
        "TxIndex leaf canonical encoding (sub-tree id=3) drifted"
    );
    assert_eq!(
        hex::encode(hash),
        "6520d5d63427865c8c94f679cfb876b18f1f2fb16abcf2b9125f06c26d0012ee",
        "TxIndex leaf hash drifted under v1 domain tag (sub_tree_id=3, canonical_index=0)"
    );
}

#[test]
fn golden_token_policy_leaf() {
    let policy = sample_token_policy();
    let payload = policy.commit_to_subtree();
    let hash = leaf_hash_v1(SUB_TREE_ID_TOKEN_POLICY, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "11000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000001",
        "TokenPolicy leaf canonical encoding (sub-tree id=4) drifted"
    );
    assert_eq!(
        hex::encode(hash),
        "9c794d13b28dfd118933b32309853820663c96885256cc807148a9dac3cffe44",
        "TokenPolicy leaf hash drifted under v1 domain tag (sub_tree_id=4, canonical_index=0)"
    );
}

#[test]
fn golden_script_leaf() {
    let script = sample_script();
    let payload = script.commit_to_subtree();
    let hash = leaf_hash_v1(SUB_TREE_ID_SCRIPT, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "1100000000000000000000000000000000000000000000000000000000000000000000010000000102",
        "Script leaf canonical encoding (sub-tree id=5) drifted"
    );
    assert_eq!(
        hex::encode(hash),
        "b0a5f7e63fe7d0798d7f61c9608eebda9648956180e69ab144dbbc7ec8f0b713",
        "Script leaf hash drifted under v1 domain tag (sub_tree_id=5, canonical_index=0)"
    );
}

#[test]
fn golden_stake_leaf() {
    // Stake leaf with KeyHash DRep (most common Conway-era variant).
    let stake = sample_stake_keyhash();
    let payload = stake.commit_to_subtree();
    let hash = leaf_hash_v1(SUB_TREE_ID_STAKE, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "111111111111111111111111111111111111111111111111111111112222222222222222222222222222222222222222222222222222222201cccccccccccccccccccccccccccccccccccccccccccccccccccccccc000000000000007b00",
        "Stake leaf canonical encoding (sub-tree id=6, drep=KeyHash) drifted"
    );
    assert_eq!(
        hex::encode(hash),
        "6608578cc2838b66ca41d030a9b33c252f5a175408611b0c8f42c92e353debd5",
        "Stake leaf hash drifted under v1 domain tag (sub_tree_id=6, canonical_index=0)"
    );
}

#[test]
fn golden_governance_leaf() {
    // Governance leaf for AccountState pots (the new variant added in B2).
    let fact = sample_account_state();
    let payload = fact.commit_to_subtree();
    let hash = leaf_hash_v1(SUB_TREE_ID_GOVERNANCE, 0, &payload);

    assert_eq!(
        hex::encode(&payload),
        "0400000005d21dba00000000000016e360000000000011bef80000000000000bb8",
        "Governance AccountState leaf canonical encoding (sub-tree id=7) drifted"
    );
    assert_eq!(
        hex::encode(hash),
        "27f7d33482f7df615ef08f70120f50b32973f5e107a249e8f2bad1f52364f47c",
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
    let pad_hash = leaf_hash_v1(SUB_TREE_ID_UTXO, EMPTY_INDEX_SENTINEL, &[]);
    assert_eq!(
        hex::encode(pad_hash),
        "140cb8b7b5595a65d3ffd5719fa5baaf5bbf2087885b3b759796240cdb6767f2",
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
    let leaf_hash = leaf_hash_v1(SUB_TREE_ID_UTXO, 0, &payload);
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
        "0be58afe1163514b9e992763983b18dfc1e1fa7ffae5d21d19229c5fddd78c49",
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
    let hash = leaf_hash_v1(SUB_TREE_ID_STAKE, 0, &payload);

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
        "f340075262998f7e71c87f77f0327f9552b4becd90df3cb2b0d4a1db472631e0",
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
        let hash = leaf_hash_v1(sub_tree_id, 0, payload);
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
    let pad_hash = leaf_hash_v1(SUB_TREE_ID_UTXO, EMPTY_INDEX_SENTINEL, &[]);
    println!("--- utxo padding leaf (EMPTY_INDEX_SENTINEL) ---");
    println!("  leaf_hash:   {}", hex::encode(pad_hash));

    // Edge: single-UTXO tree root
    let payload = sample_utxo().commit_to_subtree().unwrap();
    let tree = MerkleTree::build_v1(SUB_TREE_ID_UTXO, vec![payload]).unwrap();
    println!("--- single-UTXO tree root ---");
    println!("  root:        {}", hex::encode(tree.root()));
}
