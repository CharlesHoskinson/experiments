//! Canonical sub-tree identifier + ordering + filename mapping.
//!
//! The Ω-Commitment bundle aggregates the seven sub-tree roots in a
//! fixed canonical order. `ALL` is the authoritative order used by
//! both `assemble` and `verify`.

use serde::Serialize;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubTreeId {
    Utxo,
    Header,
    TxIndex,
    TokenPolicy,
    Script,
    Stake,
    Governance,
}

/// All sub-trees in canonical Ω-Commitment order. The bundle root
/// hashes the seven sub-tree roots in exactly this order.
pub const ALL: [SubTreeId; 7] = [
    SubTreeId::Utxo,
    SubTreeId::Header,
    SubTreeId::TxIndex,
    SubTreeId::TokenPolicy,
    SubTreeId::Script,
    SubTreeId::Stake,
    SubTreeId::Governance,
];

impl SubTreeId {
    /// Filename expected inside `--input-dir`.
    pub fn filename(&self) -> &'static str {
        match self {
            SubTreeId::Utxo => "utxo.json",
            SubTreeId::Header => "header.json",
            SubTreeId::TxIndex => "tx_index.json",
            SubTreeId::TokenPolicy => "token_policy.json",
            SubTreeId::Script => "script.json",
            SubTreeId::Stake => "stake.json",
            SubTreeId::Governance => "governance.json",
        }
    }

    /// Stable kebab-case label used in JSON output.
    pub fn label(&self) -> &'static str {
        match self {
            SubTreeId::Utxo => "utxo",
            SubTreeId::Header => "header",
            SubTreeId::TxIndex => "tx-index",
            SubTreeId::TokenPolicy => "token-policy",
            SubTreeId::Script => "script",
            SubTreeId::Stake => "stake",
            SubTreeId::Governance => "governance",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_has_seven_in_canonical_order() {
        assert_eq!(ALL.len(), 7);
        assert_eq!(ALL[0], SubTreeId::Utxo);
        assert_eq!(ALL[6], SubTreeId::Governance);
    }

    #[test]
    fn filenames_are_unique() {
        let names: std::collections::HashSet<&str> = ALL.iter().map(|s| s.filename()).collect();
        assert_eq!(names.len(), 7);
    }

    #[test]
    fn labels_match_per_sub_tree_cli_kebab_case() {
        // Sanity-check labels match the kebab-case rendering used by
        // omega-commitment-cli's SubTree enum.
        assert_eq!(SubTreeId::Utxo.label(), "utxo");
        assert_eq!(SubTreeId::TxIndex.label(), "tx-index");
        assert_eq!(SubTreeId::TokenPolicy.label(), "token-policy");
        assert_eq!(SubTreeId::Governance.label(), "governance");
    }

    #[test]
    fn filename_for_each_variant() {
        assert_eq!(SubTreeId::Utxo.filename(), "utxo.json");
        assert_eq!(SubTreeId::Header.filename(), "header.json");
        assert_eq!(SubTreeId::TxIndex.filename(), "tx_index.json");
        assert_eq!(SubTreeId::TokenPolicy.filename(), "token_policy.json");
        assert_eq!(SubTreeId::Script.filename(), "script.json");
        assert_eq!(SubTreeId::Stake.filename(), "stake.json");
        assert_eq!(SubTreeId::Governance.filename(), "governance.json");
    }
}
