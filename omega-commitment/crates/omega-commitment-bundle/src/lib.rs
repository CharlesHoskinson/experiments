//! omega-commitment-bundle: assembles the canonical Ω-Commitment tuple
//! `(blake2b_bundle_root, sha3_bundle_root)` from the seven sub-tree
//! commitments per the dual-hash decision (2026-05-01).
//!
//! As of the 2026-05-03 audit reframing, the SHA3 bundle root is a
//! drift-detection signal computed over the same v1 Blake2b leaf
//! hashes — NOT a binding-agility hedge against a Blake2b break.
//! See `bundle::verify` and the module docs in `bundle.rs` for the
//! per-sub-tree `item_count` check that closes A1/F003. The truly-
//! independent SHA3 tree (separate per-leaf SHA3 hashing) is tracked
//! as a v2.0 follow-up.

pub mod bundle;
pub mod recompute;
pub mod sub_tree_id;
