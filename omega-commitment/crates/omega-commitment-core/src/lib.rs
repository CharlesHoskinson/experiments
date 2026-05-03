//! omega-commitment-core: Ω-Commitment sub-tree library.
//!
//! Provides canonical leaf encodings, a Plonky3-friendly Merkle tree, and
//! inclusion witnesses. v0.6.0 supports all seven Ω-Commitment sub-trees:
//! UTXO set, block header chain, transaction index, native token policies,
//! script registry, stake state, and governance state.

pub mod governance_state_leaf;
pub mod hash;
pub mod header_leaf;
pub mod script_registry_leaf;
pub mod serde_helpers;
pub mod snapshot_manifest;
pub mod stake_state_leaf;
pub mod token_policy_leaf;
pub mod tree;
pub mod tx_index_leaf;
pub mod utxo_leaf;
pub mod witness;

// ---------------------------------------------------------------------------
// Sub-tree identifiers.
//
// One-byte tags bound into every v1 leaf hash via `tree::leaf_hash_v1`. The
// values are stable: a verifier sees the tag in the leaf preimage and
// rejects any leaf claimed against the wrong sub-tree. See `tree.rs` for
// the domain-separated leaf/node hash construction.
// ---------------------------------------------------------------------------

/// UTXO sub-tree.
pub const SUB_TREE_ID_UTXO: u8 = 1;
/// Block-header chain sub-tree.
pub const SUB_TREE_ID_HEADER: u8 = 2;
/// Transaction-index sub-tree.
pub const SUB_TREE_ID_TX_INDEX: u8 = 3;
/// Native-token-policy sub-tree.
pub const SUB_TREE_ID_TOKEN_POLICY: u8 = 4;
/// Script-registry sub-tree.
pub const SUB_TREE_ID_SCRIPT: u8 = 5;
/// Stake-state sub-tree.
pub const SUB_TREE_ID_STAKE: u8 = 6;
/// Governance-state sub-tree.
pub const SUB_TREE_ID_GOVERNANCE: u8 = 7;
