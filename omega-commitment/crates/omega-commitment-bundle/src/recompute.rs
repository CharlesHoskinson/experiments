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
        // Note: at depth 0, both roots equal the single leaf hash, so we
        // do not assert they differ here. The divergence invariant is
        // checked in `sha3_and_blake2b_roots_diverge_on_same_input`.
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
