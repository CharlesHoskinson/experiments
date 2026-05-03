//! omega-commitment-bundle: assembles the canonical Ω-Commitment tuple
//! `(blake2b_bundle_root, sha3_bundle_root)` from the seven sub-tree
//! commitments per the dual-hash decision (2026-05-01).

pub mod bundle;
pub mod recompute;
pub mod sub_tree_id;
