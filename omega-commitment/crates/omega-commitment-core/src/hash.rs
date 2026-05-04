//! Dual-track hashing: Blake3-256 (primary) plus SHA3-256 (drift-detection).
//!
//! Blake3 computes every leaf hash and every per-sub-tree root in this
//! crate. SHA3 is used only at the bundle layer, paired with Blake3 to
//! detect aggregation-step drift; see [`dual_hash`] for the contract.

use sha3::{Digest, Sha3_256};

/// 32-byte hash digest. Output of every hash function in this crate
/// and the leaf-hash type carried through Merkle trees and witnesses.
pub type Hash = [u8; 32];

/// Blake3-256 of `data`.
///
/// # Examples
///
/// ```
/// use omega_commitment_core::hash::blake3_256;
/// let h = blake3_256(b"hello");
/// assert_eq!(h.len(), 32);
/// ```
///
/// # Soundness
///
/// Output is collision-resistant against any adversary bounded by
/// Blake3's security level (128 bits). This is the workhorse hash for
/// every leaf, every internal Merkle node, and every per-sub-tree
/// root produced by this crate; a Blake3 collision would defeat the
/// entire commitment surface, which is why the v1 leaf and node
/// constructions add domain separation on top (see
/// [`crate::tree::leaf_hash_v2`] and [`crate::tree::node_hash_v2`]).
pub fn blake3_256(data: &[u8]) -> Hash {
    *blake3::hash(data).as_bytes()
}

/// SHA3-256 of `data`.
///
/// # Examples
///
/// ```
/// use omega_commitment_core::hash::sha3_256;
/// let h = sha3_256(b"hello");
/// assert_eq!(h.len(), 32);
/// ```
///
/// # Soundness
///
/// Output is collision-resistant under the SHA3-256 security model
/// (NIST FIPS 202). SHA3 is NOT used for any leaf, node, or per-sub-tree
/// root in this crate; it appears only at the bundle layer paired with
/// Blake3 via [`dual_hash`]. See that function's `# Soundness` block
/// for the framing: SHA3 here is drift-detection, not a break-hedge.
pub fn sha3_256(data: &[u8]) -> Hash {
    let mut h = Sha3_256::new();
    h.update(data);
    h.finalize().into()
}

/// Compute both [`blake3_256`] and [`sha3_256`] of `data` and return
/// `(blake3, sha3)`.
///
/// # Examples
///
/// ```
/// use omega_commitment_core::hash::{blake3_256, dual_hash, sha3_256};
/// let (b, s) = dual_hash(b"omega");
/// assert_eq!(b, blake3_256(b"omega"));
/// assert_eq!(s, sha3_256(b"omega"));
/// assert_ne!(b, s);
/// ```
///
/// # Soundness
///
/// `dual_hash` is the dual-track hashing helper used at the bundle
/// layer (a single hash above the seven Blake3 sub-tree roots).
///
/// **The SHA3 leg is drift-detection, NOT a break-hedge.** Both bundle
/// roots aggregate identical Blake3 leaf hashes, so a leaf-level Blake3
/// break would defeat both legs simultaneously. A divergence between
/// the two bundle roots therefore signals an aggregation-step bug, a
/// transcoding error, or in-flight tampering — not a Blake3 weakness.
/// A truly-independent SHA3 sub-tree (separate per-leaf SHA3 hashing)
/// is a v2.0 follow-up; see `ARCHITECTURE.md:9` for the audit reframing
/// (finding A1/F004).
///
/// Verifiers that consume bundle roots MUST check both legs and reject
/// on divergence; trusting the SHA3 leg as a stand-alone integrity
/// certificate is incorrect under this v0.1 framing.
pub fn dual_hash(data: &[u8]) -> (Hash, Hash) {
    (blake3_256(data), sha3_256(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blake3_256_known_vector() {
        // Test vector: blake3-256 of "" is the canonical Blake3 IV-derived empty hash.
        let h = blake3_256(b"");
        assert_eq!(
            hex::encode(h),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }

    #[test]
    fn sha3_256_known_vector() {
        // Test vector: sha3-256 of "" is known (NIST FIPS 202).
        let h = sha3_256(b"");
        assert_eq!(
            hex::encode(h),
            "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a"
        );
    }

    #[test]
    fn dual_hash_returns_both() {
        let (b, s) = dual_hash(b"omega");
        assert_ne!(b, s, "Blake3 and SHA3 must produce different outputs");
        assert_eq!(b, blake3_256(b"omega"));
        assert_eq!(s, sha3_256(b"omega"));
    }
}
