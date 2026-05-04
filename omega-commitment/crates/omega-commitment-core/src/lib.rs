//! Canonical leaf encodings, a Plonky3-friendly binary Merkle tree, and
//! inclusion witnesses for the seven Ω-Commitment sub-trees: UTXO set,
//! block-header chain, transaction index, native-token policies, script
//! registry, stake state, and governance state.
//!
//! # Design context
//!
//! - OpenSpec change: [`add-proof-experiment-harness`][change].
//! - Blake3 migration design: [2026-05-03 Blake3 migration design][blake3].
//! - Architectural reframing of the dual-hash story: [`ARCHITECTURE.md`][arch].
//!
//! [change]: ../../../openspec/changes/add-proof-experiment-harness/
//! [blake3]: ../../../cardano-wiki/docs/superpowers/specs/2026-05-03-blake3-migration-design.md
//! [arch]: ../../../ARCHITECTURE.md
//!
//! # Tier of trust
//!
//! Soundness-bearing. Every public function and type that participates
//! in the leaf-hash, node-hash, or Merkle-build pipeline carries a
//! `# Soundness` block stating the invariant it preserves and the
//! attack class it closes. See [`tree::leaf_hash_v2`] for the canonical
//! example and [`witness::InclusionWitness::verify`] for the verifier
//! contract.
//!
//! # v0.1 limitations
//!
//! - Leaf preimages MUST be ≤ 64 bytes (one Blake3 compression block)
//!   to be provable by the v0.1 `OmegaMembershipAir` in
//!   `omega-claim-prover`. The encoders in this crate compute correct
//!   hashes for longer payloads, but a longer payload cannot be proven
//!   in the v0.1 STARK; v0.2's `LeafPreimageAir` lifts the bound.
//! - The SHA3-paired bundle root (consumed by `omega-commitment-bundle`)
//!   is **drift-detection, not break-hedge**: both bundle roots
//!   aggregate identical Blake3 leaf hashes, so a leaf-level Blake3
//!   break would defeat both. A divergence between the two bundle
//!   roots therefore signals an aggregation-step bug, not a Blake3
//!   weakness. A truly-independent SHA3 sub-tree is tracked as a v2.0
//!   follow-up. See `ARCHITECTURE.md:9` for the audit reframing.
//! - The legacy [`tree::MerkleTree::build`] path is retained for tests
//!   and CLIs that pre-hash; it does NOT apply v1 domain separation
//!   and pads with raw [`tree::ZERO_HASH`]. New production paths must
//!   use [`tree::MerkleTree::build_v1`].
//!
//! # Conventions specific to this crate
//!
//! - **Hash function**: Blake3-256 throughout. The `Hash` alias is
//!   `[u8; 32]`.
//! - **Domain separation**: every v1 leaf preimage is prefixed with
//!   `"omega:v2:leaf"` (see [`tree::DOMAIN_LEAF`]) and every v1
//!   internal-node preimage is prefixed with `"omega:v2:node"`
//!   ([`tree::DOMAIN_NODE`]). Leaf and node preimages cannot collide.
//! - **Sub-tree binding**: every v1 leaf hash binds the one-byte
//!   `sub_tree_id` (see the constants [`SUB_TREE_ID_UTXO`] through
//!   [`SUB_TREE_ID_GOVERNANCE`]) and the canonical sorted index, so a
//!   verifier reading the published `item_count` rejects any inclusion
//!   proof whose `canonical_index >= item_count`.
//! - **Sort-then-pad-to-pow2**: [`tree::MerkleTree::build_v1`] sorts the
//!   raw payload bytes lexicographically, hashes each leaf with its
//!   sorted index bound in, then pads with the domain-separated empty
//!   leaf (using the reserved [`tree::EMPTY_INDEX_SENTINEL`]) up to the
//!   next power of two.
//! - **Duplicate-payload rejection**: [`tree::MerkleTree::build_v1`]
//!   returns [`tree::BuildError::DuplicateLeafPayload`] when two
//!   distinct entries share canonical bytes; in every sub-tree the
//!   semantic key is injected into the payload, so byte-identical
//!   payloads always indicate ingest-side data corruption.
//! - **Hash-only**: there are no curve operations anywhere in this
//!   crate. The construction is post-quantum.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::missing_crate_level_docs)]

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
// One-byte tags bound into every v1 leaf hash via `tree::leaf_hash_v2`. The
// values are stable: a verifier sees the tag in the leaf preimage and
// rejects any leaf claimed against the wrong sub-tree. See `tree.rs` for
// the domain-separated leaf/node hash construction.
// ---------------------------------------------------------------------------

/// UTXO sub-tree identifier (`0x01`). Bound into every UTXO leaf
/// preimage via [`tree::leaf_hash_v2`].
pub const SUB_TREE_ID_UTXO: u8 = 1;
/// Block-header chain sub-tree identifier (`0x02`).
pub const SUB_TREE_ID_HEADER: u8 = 2;
/// Transaction-index sub-tree identifier (`0x03`).
pub const SUB_TREE_ID_TX_INDEX: u8 = 3;
/// Native-token-policy sub-tree identifier (`0x04`).
pub const SUB_TREE_ID_TOKEN_POLICY: u8 = 4;
/// Script-registry sub-tree identifier (`0x05`).
pub const SUB_TREE_ID_SCRIPT: u8 = 5;
/// Stake-state sub-tree identifier (`0x06`).
pub const SUB_TREE_ID_STAKE: u8 = 6;
/// Governance-state sub-tree identifier (`0x07`).
pub const SUB_TREE_ID_GOVERNANCE: u8 = 7;
