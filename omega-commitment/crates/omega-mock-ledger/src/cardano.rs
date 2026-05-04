//! Optional Cardano phase-1 validation hook.
//!
//! Pinned `pallas-validate = =1.0.0-alpha.6` exposes the phase-1 entry point
//! with this signature in `var/upstream/pallas/pallas-validate/src/phase1/mod.rs`:
//!
//! ```ignore
//! pub fn validate_tx(
//!     metx: &MultiEraTx,
//!     txix: TransactionIndex,
//!     env: &Environment,
//!     utxos: &UTxOs,
//!     cert_state: &mut CertState,
//! ) -> ValidationResult
//! ```
//!
//! The feature is default-off until the harness has fixture coverage for the
//! `Environment`, `UTxOs`, and `CertState` setup expected by that API.
