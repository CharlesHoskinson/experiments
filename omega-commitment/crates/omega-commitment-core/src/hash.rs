//! Dual-track hashing: Blake2b-256 (primary) + SHA3-256 (shadow).

use blake2::digest::consts::U32;
use blake2::{Blake2b, Digest as Blake2Digest};
use sha3::Sha3_256;

pub type Hash = [u8; 32];

/// Primary hash: Blake2b truncated to 256 bits.
pub fn blake2b_256(data: &[u8]) -> Hash {
    let mut h = Blake2b::<U32>::new();
    h.update(data);
    h.finalize().into()
}

/// Shadow hash: SHA3-256.
pub fn sha3_256(data: &[u8]) -> Hash {
    let mut h = Sha3_256::new();
    h.update(data);
    h.finalize().into()
}

/// Dual-track hash. Returns (blake2b_256, sha3_256). Both must be checked
/// by verifiers; divergence means a bug or tampering.
pub fn dual_hash(data: &[u8]) -> (Hash, Hash) {
    (blake2b_256(data), sha3_256(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blake2b_256_known_vector() {
        // Test vector: blake2b-256 of "" is known.
        let h = blake2b_256(b"");
        assert_eq!(
            hex::encode(h),
            "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8"
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
        assert_ne!(b, s, "Blake2b and SHA3 must produce different outputs");
        assert_eq!(b, blake2b_256(b"omega"));
        assert_eq!(s, sha3_256(b"omega"));
    }
}
