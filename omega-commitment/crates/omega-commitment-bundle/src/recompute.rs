//! Per-sub-tree dual-hash root computation.
//!
//! Given raw input bytes for a sub-tree (the JSON file the per-sub-tree
//! CLI consumes), recompute:
//!   - the v1 domain-separated Blake3 leaf set (one
//!     `leaf_hash_v2(SUB_TREE_ID, canonical_index, payload)` per item)
//!   - the v1 Blake3 Merkle root via `MerkleTree::build_v1`
//!   - the SHA3 Merkle root over the SAME v1 Blake3 leaf hashes
//!     (drift-detection — see Batch 1 framing in `bundle.rs`)
//!   - the input digest (Blake3-256 of raw input bytes)
//!   - leaf_count, tree_depth, item_count
//!
//! Per the dual-hash decision (2026-05-01) and the 2026-05-03 audit
//! reframing: the SHA3 path is NOT a Blake3-break hedge — both trees
//! are built from identical v1 Blake3 leaf hashes, so a leaf-level
//! Blake3 break would defeat both roots. The SHA3 root catches drift
//! in the bundle aggregation step (and pre-pays the v2.0 fully-
//! independent SHA3 tree).

use crate::sub_tree_id::SubTreeId;
use omega_commitment_core::{
    governance_state_leaf::GovernanceFact,
    hash::{blake3_256, sha3_256, Hash},
    header_leaf::BlockHeader,
    script_registry_leaf::ScriptEntry,
    stake_state_leaf::StakeEntry,
    token_policy_leaf::TokenPolicy,
    tree::{leaf_hash_v2, MerkleTree, EMPTY_INDEX_SENTINEL},
    tx_index_leaf::TxIndexEntry,
    utxo_leaf::Utxo,
    SUB_TREE_ID_GOVERNANCE, SUB_TREE_ID_HEADER, SUB_TREE_ID_SCRIPT, SUB_TREE_ID_STAKE,
    SUB_TREE_ID_TOKEN_POLICY, SUB_TREE_ID_TX_INDEX, SUB_TREE_ID_UTXO,
};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubTreeRoots {
    pub blake3_root: Hash,
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
/// Returns Err if the JSON cannot be parsed for the given sub-tree shape,
/// if a leaf cannot encode (UTXO over u32 asset count, etc.), or if the
/// v1 Merkle builder rejects duplicate canonical payloads.
pub fn recompute(sub_tree: SubTreeId, raw: &str) -> anyhow::Result<SubTreeRoots> {
    let input_digest = blake3_256(raw.as_bytes());
    let (sub_tree_id, payloads, item_count) = build_payloads(sub_tree, raw)?;
    let tree = MerkleTree::build_v1(sub_tree_id, payloads.clone())
        .map_err(|e| anyhow::anyhow!("v1 build rejected sub-tree input: {e}"))?;
    let blake3_root = tree.root();
    let sha3_root = sha3_root_of_v1(sub_tree_id, payloads);
    Ok(SubTreeRoots {
        blake3_root,
        sha3_root,
        input_digest,
        leaf_count: tree.leaf_count(),
        tree_depth: tree.depth(),
        item_count,
    })
}

fn build_payloads(sub_tree: SubTreeId, raw: &str) -> anyhow::Result<(u8, Vec<Vec<u8>>, usize)> {
    match sub_tree {
        SubTreeId::Utxo => {
            let parsed: UtxoInput = serde_json::from_str(raw)?;
            let payloads: Vec<Vec<u8>> = parsed
                .utxos
                .iter()
                .map(|u| u.commit_to_subtree())
                .collect::<Result<Vec<_>, _>>()?;
            let n = parsed.utxos.len();
            Ok((SUB_TREE_ID_UTXO, payloads, n))
        }
        SubTreeId::Header => {
            let parsed: HeaderInput = serde_json::from_str(raw)?;
            let payloads: Vec<Vec<u8>> = parsed
                .headers
                .iter()
                .map(|h| h.commit_to_subtree())
                .collect();
            let n = parsed.headers.len();
            Ok((SUB_TREE_ID_HEADER, payloads, n))
        }
        SubTreeId::TxIndex => {
            let parsed: TxIndexInput = serde_json::from_str(raw)?;
            let payloads: Vec<Vec<u8>> = parsed
                .entries
                .iter()
                .map(|e| e.commit_to_subtree())
                .collect();
            let n = parsed.entries.len();
            Ok((SUB_TREE_ID_TX_INDEX, payloads, n))
        }
        SubTreeId::TokenPolicy => {
            let parsed: TokenPolicyInput = serde_json::from_str(raw)?;
            let payloads: Vec<Vec<u8>> = parsed
                .policies
                .iter()
                .map(|p| p.commit_to_subtree())
                .collect();
            let n = parsed.policies.len();
            Ok((SUB_TREE_ID_TOKEN_POLICY, payloads, n))
        }
        SubTreeId::Script => {
            let parsed: ScriptInput = serde_json::from_str(raw)?;
            let payloads: Vec<Vec<u8>> = parsed
                .scripts
                .iter()
                .map(|s| s.commit_to_subtree())
                .collect();
            let n = parsed.scripts.len();
            Ok((SUB_TREE_ID_SCRIPT, payloads, n))
        }
        SubTreeId::Stake => {
            let parsed: StakeInput = serde_json::from_str(raw)?;
            let payloads: Vec<Vec<u8>> = parsed
                .stake_entries
                .iter()
                .map(|s| s.commit_to_subtree())
                .collect();
            let n = parsed.stake_entries.len();
            Ok((SUB_TREE_ID_STAKE, payloads, n))
        }
        SubTreeId::Governance => {
            let parsed: GovernanceInput = serde_json::from_str(raw)?;
            let payloads: Vec<Vec<u8>> =
                parsed.facts.iter().map(|f| f.commit_to_subtree()).collect();
            let n = parsed.facts.len();
            Ok((SUB_TREE_ID_GOVERNANCE, payloads, n))
        }
    }
}

/// SHA3 Merkle aggregation over the v1 Blake3 leaf hashes.
///
/// Mirrors `MerkleTree::build_v1` exactly — sort payloads, hash each
/// with `leaf_hash_v2`, pad to next power of two with the
/// `EMPTY_INDEX_SENTINEL` empty leaf — but substitutes raw SHA3-256 for
/// the internal-node hash so a divergence between the two roots
/// indicates aggregation drift.
fn sha3_root_of_v1(sub_tree_id: u8, mut payloads: Vec<Vec<u8>>) -> Hash {
    payloads.sort();
    let mut leaves: Vec<Hash> = payloads
        .iter()
        .enumerate()
        .map(|(i, p)| leaf_hash_v2(sub_tree_id, i as u64, p))
        .collect();
    let target = leaves.len().max(1).next_power_of_two();
    while leaves.len() < target {
        leaves.push(leaf_hash_v2(sub_tree_id, EMPTY_INDEX_SENTINEL, &[]));
    }
    let mut current = leaves;
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
    use omega_commitment_core::tree::ZERO_HASH;

    const UTXO_FIXTURE: &str = r#"{
        "utxos": [
            {
                "tx_id": "0101010101010101010101010101010101010101010101010101010101010101",
                "output_index": 0,
                "address": "6102020202020202020202020202020202020202020202020202020202020202",
                "value_lovelace": 1000000,
                "assets": [],
                "datum_option": { "kind": "none" },
                "script_ref": null
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
        assert_ne!(r.blake3_root, [0u8; 32]);
        assert_ne!(r.sha3_root, [0u8; 32]);
        // Note: at depth 0, both roots equal the single leaf hash, so we
        // do not assert they differ here. The divergence invariant is
        // checked in `sha3_and_blake3_roots_diverge_on_same_input`.
        assert_ne!(r.input_digest, [0u8; 32]);
    }

    #[test]
    fn recompute_is_deterministic() {
        let a = recompute(SubTreeId::Utxo, UTXO_FIXTURE).unwrap();
        let b = recompute(SubTreeId::Utxo, UTXO_FIXTURE).unwrap();
        assert_eq!(a.blake3_root, b.blake3_root);
        assert_eq!(a.sha3_root, b.sha3_root);
        assert_eq!(a.input_digest, b.input_digest);
    }

    #[test]
    fn recompute_blake3_root_matches_v1_builder_directly() {
        // The v1 Blake3 path inside recompute must agree with calling
        // MerkleTree::build_v1 directly — this is the cross-check that
        // protects against any divergence between the bundle tool
        // and the per-sub-tree CLI.
        let parsed: UtxoInput = serde_json::from_str(UTXO_FIXTURE).unwrap();
        let payloads: Vec<Vec<u8>> = parsed
            .utxos
            .iter()
            .map(|u| u.commit_to_subtree().unwrap())
            .collect();
        let tree = MerkleTree::build_v1(SUB_TREE_ID_UTXO, payloads).unwrap();
        let r = recompute(SubTreeId::Utxo, UTXO_FIXTURE).unwrap();
        assert_eq!(r.blake3_root, tree.root());
    }

    #[test]
    fn sha3_root_of_v1_empty_pads_to_sentinel_leaf() {
        // sha3_root_of_v1 with empty input pads to one EMPTY_INDEX_SENTINEL
        // leaf hash, which is non-zero (carries the v1 domain tag and the
        // sentinel index in its preimage).
        let r = sha3_root_of_v1(SUB_TREE_ID_UTXO, vec![]);
        assert_ne!(r, ZERO_HASH);
        // Equals the v1 padding leaf hash directly (depth-0 tree).
        assert_eq!(r, leaf_hash_v2(SUB_TREE_ID_UTXO, EMPTY_INDEX_SENTINEL, &[]));
    }

    #[test]
    fn sha3_root_of_v1_single_payload_is_the_leaf() {
        let payload = b"only".to_vec();
        let r = sha3_root_of_v1(SUB_TREE_ID_UTXO, vec![payload.clone()]);
        // Single leaf, no padding needed (next_power_of_two(1) == 1).
        // Depth-0: root == leaf == leaf_hash_v2(sub_tree, 0, payload).
        assert_eq!(r, leaf_hash_v2(SUB_TREE_ID_UTXO, 0, &payload));
    }

    #[test]
    fn sha3_root_of_v1_two_leaves_is_sha3_of_concatenation() {
        let a = b"alpha".to_vec();
        let b = b"beta".to_vec();
        // Sort to mirror the builder.
        let mut sorted = [a.clone(), b.clone()];
        sorted.sort();
        let l0 = leaf_hash_v2(SUB_TREE_ID_UTXO, 0, &sorted[0]);
        let l1 = leaf_hash_v2(SUB_TREE_ID_UTXO, 1, &sorted[1]);
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(&l0);
        buf[32..].copy_from_slice(&l1);
        let expected = sha3_256(&buf);
        assert_eq!(sha3_root_of_v1(SUB_TREE_ID_UTXO, vec![a, b]), expected);
    }

    #[test]
    fn sha3_and_blake3_roots_diverge_on_same_input() {
        // Sanity: SHA3 and Blake3 roots of the same v1 leaf set must
        // differ once the tree has at least one internal-node layer.
        let payloads: Vec<Vec<u8>> = (0..8u8).map(|i| vec![i, 0xAA]).collect();
        let blake_root = MerkleTree::build_v1(SUB_TREE_ID_UTXO, payloads.clone())
            .unwrap()
            .root();
        let sha3_root = sha3_root_of_v1(SUB_TREE_ID_UTXO, payloads);
        assert_ne!(blake_root, sha3_root);
    }
}
