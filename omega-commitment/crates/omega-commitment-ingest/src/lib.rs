//! omega-commitment-ingest: transforms real Cardano data (CBOR
//! LedgerState snapshots, chain history) into the per-sub-tree JSON
//! formats consumed by `omega-commitment commit`.
//!
//! v0.8.0 implements UTXO ingestion end-to-end and scaffolds the
//! other four LedgerState-derivable sub-trees (token-policy, script,
//! stake, governance) — those four return `unimplemented!()` with a
//! pointer to the follow-up `omega-commitment-ingest-mainnet` plan.
//! Header and tx-index ingestion is documented as future work
//! requiring a chain-follower.

pub mod cbor;
pub mod governance;
pub mod script;
pub mod stake;
pub mod token_policy;
pub mod utxo;
