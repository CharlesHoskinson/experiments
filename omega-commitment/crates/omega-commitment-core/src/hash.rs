//! Dual-track hashing: Blake3-256 (primary) + SHA3-256 (shadow).

use sha3::{Digest, Sha3_256};

pub type Hash = [u8; 32];

/// Primary hash: Blake3-256.
pub fn blake3_256(data: &[u8]) -> Hash {
    *blake3::hash(data).as_bytes()
}

/// Shadow hash: SHA3-256.
pub fn sha3_256(data: &[u8]) -> Hash {
    let mut h = Sha3_256::new();
    h.update(data);
    h.finalize().into()
}

/// Dual-track hash. Returns (blake3_256, sha3_256). Both must be checked
/// by verifiers; divergence means a bug or tampering.
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
