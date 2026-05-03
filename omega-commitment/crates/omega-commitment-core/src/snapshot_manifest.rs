//! Snapshot manifest pinning the `(block_hash, slot, epoch)` tuple of
//! the chain point at which a UTxO snapshot was taken.
//!
//! The manifest closes audit finding A2/F002: production runs of
//! `omega-utxo-snapshot` must commit to a specific chain point rather
//! than `acquire(None)`'ing the wandering tip. The manifest records:
//!
//!   - `block_hash` and `slot` that the LSQ session pins via
//!     `Point::Specific`,
//!   - `epoch` the slot belongs to,
//!   - `stability_depth` (≥ k = 2160 enforced by [`SnapshotManifest::validate`]),
//!   - `stake_snapshot_select` indicating which of the three Cardano
//!     stake distributions (Mark / Set / Go) the snapshot belongs to.
//!
//! Manifests round-trip via `serde` (camelCase JSON keys); the
//! `omega-utxo-snapshot` CLI consumes them via `--manifest <path>`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Cardano consensus stability parameter `k`. A snapshot is "stable"
/// only after `k` blocks have built atop the pinned point; the manifest
/// validator rejects depths shallower than this.
pub const K_STABILITY: u32 = 2160;

/// Which of the three Cardano stake-distribution snapshots
/// (mark / set / go) the manifest pins.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StakeSnapshot {
    Mark,
    Set,
    Go,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotManifest {
    /// 32-byte Cardano block hash at the snapshot point.
    #[serde(with = "hex::serde")]
    pub block_hash: [u8; 32],
    /// Absolute slot number.
    pub slot: u64,
    /// Epoch the slot belongs to.
    pub epoch: u64,
    /// Number of blocks confirmed atop the pinned point. Must be at
    /// least `K_STABILITY = 2160`; enforced by [`Self::validate`].
    pub stability_depth: u32,
    /// Mark / Set / Go selection.
    pub stake_snapshot_select: StakeSnapshot,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum ManifestError {
    #[error("stability_depth {actual} below k = {required} blocks; snapshot is not yet stable")]
    StabilityTooShallow { actual: u32, required: u32 },
}

impl SnapshotManifest {
    /// Reject manifests whose stability depth is below `k = 2160`.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.stability_depth < K_STABILITY {
            return Err(ManifestError::StabilityTooShallow {
                actual: self.stability_depth,
                required: K_STABILITY,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SnapshotManifest {
        SnapshotManifest {
            block_hash: [0xAB; 32],
            slot: 123_456_789,
            epoch: 500,
            stability_depth: 2160,
            stake_snapshot_select: StakeSnapshot::Set,
        }
    }

    #[test]
    fn validate_accepts_exactly_k() {
        let m = sample();
        m.validate().unwrap();
    }

    #[test]
    fn validate_rejects_below_k() {
        let mut m = sample();
        m.stability_depth = K_STABILITY - 1;
        let err = m.validate().unwrap_err();
        assert_eq!(
            err,
            ManifestError::StabilityTooShallow {
                actual: K_STABILITY - 1,
                required: K_STABILITY,
            }
        );
    }

    #[test]
    fn validate_accepts_above_k() {
        let mut m = sample();
        m.stability_depth = K_STABILITY * 10;
        m.validate().unwrap();
    }

    #[test]
    fn json_round_trip_preserves_fields() {
        let m = sample();
        let s = serde_json::to_string(&m).unwrap();
        // Verify camelCase keys.
        assert!(s.contains("\"blockHash\""), "expected camelCase key in {s}");
        assert!(
            s.contains("\"stabilityDepth\""),
            "expected camelCase key in {s}"
        );
        assert!(
            s.contains("\"stakeSnapshotSelect\""),
            "expected camelCase key in {s}"
        );
        let parsed: SnapshotManifest = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, m);
    }

    #[test]
    fn stake_snapshot_serializes_camel_case() {
        let m = SnapshotManifest {
            stake_snapshot_select: StakeSnapshot::Mark,
            ..sample()
        };
        let s = serde_json::to_string(&m).unwrap();
        assert!(
            s.contains("\"mark\""),
            "Mark should serialize lowercase: {s}"
        );

        let m_set = SnapshotManifest {
            stake_snapshot_select: StakeSnapshot::Set,
            ..sample()
        };
        let s_set = serde_json::to_string(&m_set).unwrap();
        assert!(s_set.contains("\"set\""));

        let m_go = SnapshotManifest {
            stake_snapshot_select: StakeSnapshot::Go,
            ..sample()
        };
        let s_go = serde_json::to_string(&m_go).unwrap();
        assert!(s_go.contains("\"go\""));
    }

    #[test]
    fn block_hash_is_hex_in_json() {
        let m = sample();
        let s = serde_json::to_string(&m).unwrap();
        // 32 × 0xAB encodes to 64 lowercase 'a','b' chars.
        assert!(s.contains(&"ab".repeat(32)));
    }
}
