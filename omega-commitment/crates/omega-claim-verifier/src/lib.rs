//! Plonky3 STARK verifier for Merkle-membership claims against a published
//! Ω-Commitment.
//!
//! # Overview
//!
//! `omega-claim-verifier` consumes proof envelopes produced by
//! [`omega_claim_prover::prove_collection`] and returns `Ok(())` exactly when
//! the proof attests to the inclusion of the claimed leaves under the bound
//! commitment. The verifier surface is pure: no I/O, no async, no global
//! state, no panic on adversarial input.
//!
//! # Design context
//!
//! - OpenSpec change: [`add-proof-experiment-harness`][1].
//! - Spec scenarios: [`Plonky3 verifier for Merkle membership`][2].
//! - PR-2 review record: [`PR-2-REVIEW-v2.md`][3] (post-soundness-fix; adds
//!   `WrongSubTreeRoot`, `DepthMismatch`, and the binding-digest envelope
//!   rewrite check).
//!
//! [1]: ../../../openspec/changes/add-proof-experiment-harness/
//! [2]: ../../../openspec/changes/add-proof-experiment-harness/specs/proof-harness/spec.md
//! [3]: ../../../openspec/changes/add-proof-experiment-harness/PR-2-REVIEW-v2.md
//!
//! # Tier of trust
//!
//! Soundness-bearing, top tier. Every public function carries a `# Soundness`
//! block; every error path is the last line of defence against an
//! accepted-but-malformed claim. See [`verify`] for the canonical example.
//!
//! # v0.1 limitations
//!
//! - Leaf preimages ≤ 64 bytes (one Blake3 compression block). Longer
//!   preimages require v0.2's `LeafPreimageAir`.
//! - Wire format v2: `ClaimPublicInputs` carries `tree_depth` and
//!   `per_sub_tree_root`. v1 envelopes are unsupported and rejected with
//!   [`VerifyError::UnsupportedVersion`].
//! - The verifier's STARK configuration is byte-identical to the prover's;
//!   any drift in the verifier's `make_stark_config` versus
//!   [`omega_claim_prover`]'s prover-side builder produces
//!   [`VerifyError::InvalidProof`] on every input.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::missing_crate_level_docs)]

use omega_claim_prover::{
    proof_airs, proof_binding_words, OmegaCommitment, ProofEnvelope, PROOF_BINDING_WORD_OFFSET,
    PROOF_ENVELOPE_VERSION, PROOF_PUBLIC_VALUE_COUNT, PUBLIC_LEAF_INDEX_OFFSET, PUBLIC_ROOT_OFFSET,
    PUBLIC_SUB_TREE_ID_OFFSET, PUBLIC_TREE_DEPTH_OFFSET,
};
use omega_claim_tx::{ClaimPublicInputs, ProofBytes};
use p3_baby_bear::{BabyBear, Poseidon2BabyBear};
use p3_batch_stark::{verify_batch, BatchProof, ProverData};
use p3_challenger::DuplexChallenger;
use p3_commit::ExtensionMmcs;
use p3_field::{extension::BinomialExtensionField, Field, PrimeCharacteristicRing};
use p3_fri::{FriParameters, TwoAdicFriPcs};
use p3_merkle_tree::MerkleTreeMmcs;
use p3_monty_31::dft::RecursiveDft;
use p3_symmetric::{PaddingFreeSponge, TruncatedPermutation};
use p3_uni_stark::StarkConfig;
use rand::rngs::SmallRng;
use rand::SeedableRng;
use thiserror::Error;

/// Native field used by the membership AIR: BabyBear (31-bit Mersenne-like
/// prime). Pinned to match the prover's commitment scheme.
type Val = BabyBear;
/// Quartic extension of [`Val`], used as the FRI challenge field for
/// 100+ bits of soundness.
type Challenge = BinomialExtensionField<Val, 4>;
/// Recursive DFT over [`Val`], used by the Plonky3 polynomial commitment
/// scheme to commit to trace columns.
type Dft = RecursiveDft<Val>;
/// Width-16 Poseidon2 permutation over BabyBear, used as the inner
/// compression function of the Merkle-tree MMCS.
type Perm16 = Poseidon2BabyBear<16>;
/// Width-24 Poseidon2 permutation over BabyBear, used both as the duplex
/// challenger absorber and as the sponge underlying [`Poseidon2Sponge`].
type Perm24 = Poseidon2BabyBear<24>;
/// Padding-free sponge built from [`Perm24`] (rate 16, capacity 8, output 8),
/// used as the leaf hash of the Merkle-tree MMCS.
type Poseidon2Sponge = PaddingFreeSponge<Perm24, 24, 16, 8>;
/// Truncated [`Perm16`]-based two-to-one compression (chunk 8, output 8),
/// used as the internal node hash of the Merkle-tree MMCS.
type Poseidon2Compression = TruncatedPermutation<Perm16, 2, 8, 16>;
/// Vector-commitment Merkle MMCS over [`Val`] using the Poseidon2 sponge
/// and compression above; commits to the prover's trace columns.
type ValMmcs = MerkleTreeMmcs<
    <Val as Field>::Packing,
    <Val as Field>::Packing,
    Poseidon2Sponge,
    Poseidon2Compression,
    2,
    8,
>;
/// Lifts [`ValMmcs`] to commitments over [`Challenge`] for the FRI
/// extension-field rounds.
type ChallengeMmcs = ExtensionMmcs<Val, Challenge, ValMmcs>;
/// Two-adic FRI polynomial commitment scheme parameterised over
/// [`Val`], [`Dft`], [`ValMmcs`], and [`ChallengeMmcs`].
type Pcs = TwoAdicFriPcs<Val, Dft, ValMmcs, ChallengeMmcs>;
/// Duplex challenger that derives Fiat-Shamir randomness from absorbed
/// commitments via [`Perm24`].
type Challenger = DuplexChallenger<Val, Perm24, 24, 16>;
/// Concrete Plonky3 STARK configuration consumed by [`verify_batch`]. This
/// alias is the verifier's commitment to a specific PCS / challenger /
/// challenge-field tuple; any change must be mirrored on the prover side.
type OmegaStarkConfig = StarkConfig<Pcs, Challenge, Challenger>;

/// Reasons [`verify`] rejects a proof envelope.
///
/// Variants are ordered roughly by the verifier's check sequence: parse
/// errors first, envelope-binding errors next, public-input
/// well-formedness errors, then the final Plonky3 proof check. The
/// `#[non_exhaustive]` marker reserves room for future v0.2 variants
/// without breaking downstream `match` exhaustiveness.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum VerifyError {
    /// Fires when [`ProofEnvelope::version`] is not
    /// [`PROOF_ENVELOPE_VERSION`]. v1 envelopes are unsupported.
    #[error("unsupported proof envelope version {version}")]
    UnsupportedVersion {
        /// The version byte read from the envelope.
        version: u8,
    },
    /// Fires when the envelope's bound [`OmegaCommitment`] does not equal
    /// the verifier's `commitment` argument. An adversary cannot rebind a
    /// proof to a commitment it was not produced for.
    #[error("proof commitment does not match verifier commitment")]
    CommitmentMismatch,
    /// Fires when the envelope's bound public-input slice does not
    /// byte-equal the verifier's `public_inputs` argument.
    #[error("proof public inputs do not match verifier public inputs")]
    PublicInputMismatch,
    /// Fires when a public input names a `sub_tree_id` outside the
    /// commitment's seven sub-trees.
    #[error("public input {index} references unknown sub-tree id {sub_tree_id}")]
    UnknownSubTree {
        /// Index of the offending public input within the batch.
        index: usize,
        /// The unknown sub-tree id.
        sub_tree_id: u8,
    },
    /// Fires when a public input's `bundle_root_blake3` does not equal the
    /// commitment's bundle root.
    #[error("public input {index} bundle root does not match the verifier commitment")]
    PublicBundleRootMismatch {
        /// Index of the offending public input within the batch.
        index: usize,
    },
    /// Fires when a public input's `per_sub_tree_root` does not equal the
    /// commitment's recorded root for the claimed `sub_tree_id`.
    ///
    /// Added in PR #2 as part of the soundness fix: this check, together
    /// with [`Self::DepthMismatch`], runs *before* Plonky3 verification so
    /// the verifier rejects stale or substituted sub-tree roots without
    /// touching the FRI machinery.
    #[error("public input {index} per-sub-tree root does not match the verifier commitment")]
    WrongSubTreeRoot {
        /// Index of the offending public input within the batch.
        index: usize,
    },
    /// Fires when a public input's `tree_depth` does not equal the
    /// commitment's recorded depth for the claimed `sub_tree_id`.
    ///
    /// Added in PR #2 alongside [`Self::WrongSubTreeRoot`]; closes a
    /// padding-leaf forgery vector where an adversary could inflate or
    /// deflate the claimed depth without otherwise altering the witness.
    #[error("public input {index} tree depth mismatch: expected {expected}, actual {actual}")]
    DepthMismatch {
        /// Index of the offending public input within the batch.
        index: usize,
        /// The depth recorded by the published commitment.
        expected: u8,
        /// The depth carried by the public input.
        actual: u8,
    },
    /// Fires when Plonky3 itself rejects the envelope. This collapses
    /// every cryptographic failure into one variant: malformed envelope
    /// bytes, public-value layout mismatches, unsatisfied trace
    /// constraints, FRI failure, LogUp lookup imbalance, and
    /// binding-digest mismatch all surface here.
    #[error("invalid Plonky3 proof")]
    InvalidProof,
}

/// Verifies a Plonky3 STARK proof of Merkle inclusion against a published
/// Ω-Commitment.
///
/// `commitment` is the published Ω-Commitment, `public_inputs` is the batch
/// of leaves being claimed (one entry per membership instance), and `proof`
/// is the postcard-encoded [`ProofEnvelope`] produced by
/// [`omega_claim_prover::prove_collection`].
///
/// # Errors
///
/// Returns:
/// - [`VerifyError::InvalidProof`] when the envelope bytes fail to decode,
///   when Plonky3 rejects the proof (trace constraints unsatisfied, FRI
///   failure, lookup imbalance), or when the envelope's `public_values`
///   layout does not match the prover's binding-digest words.
/// - [`VerifyError::UnsupportedVersion`] when the envelope's version byte
///   is not [`PROOF_ENVELOPE_VERSION`].
/// - [`VerifyError::CommitmentMismatch`] when the envelope's bound
///   commitment does not equal `commitment`.
/// - [`VerifyError::PublicInputMismatch`] when the envelope's bound
///   public-input slice does not equal `public_inputs`.
/// - [`VerifyError::UnknownSubTree`] when a public input names a sub-tree
///   id outside `1..=7`.
/// - [`VerifyError::PublicBundleRootMismatch`] when a public input's
///   bundle root does not equal `commitment.bundle_root_blake3`.
/// - [`VerifyError::WrongSubTreeRoot`] when a public input's
///   `per_sub_tree_root` does not equal the commitment's recorded root for
///   the claimed sub-tree.
/// - [`VerifyError::DepthMismatch`] when a public input's `tree_depth`
///   does not equal the commitment's recorded depth for the claimed
///   sub-tree.
///
/// # Soundness
///
/// `verify(commitment, public_inputs, proof)` returns `Ok(())` iff, for
/// every entry `p` of `public_inputs`, there exists a witness `(payload,
/// merkle_path, sub_tree_id, leaf_index, depth)` such that:
///
/// 1. `leaf_hash_v2(sub_tree_id, leaf_index, payload) = current_node_0`.
/// 2. For each `i` in `0..depth-1`, `current_node_{i+1} =
///    node_hash_v2(left_i, right_i)` where `(left_i, right_i)` is the swap
///    of `(current_node_i, sibling_i)` keyed by the `i`-th bit of
///    `leaf_index`.
/// 3. `current_node_{depth-1} = commitment.sub_tree_roots_blake3[
///    sub_tree_index(sub_tree_id)]`.
/// 4. `p.tree_depth == depth` and `p.per_sub_tree_root ==
///    commitment.sub_tree_roots_blake3[sub_tree_index(p.sub_tree_id)]`.
///
/// The Blake3 compressions inside conditions 1 and 2 are bound to the
/// embedded `OmegaBlake3Air`'s permutation rows via LogUp. An adversary
/// who supplies inconsistent compression inputs causes the LogUp imbalance
/// check to fail inside [`verify_batch`], surfacing as
/// [`VerifyError::InvalidProof`].
///
/// The envelope's binding-digest words commit `(commitment, public_inputs)`
/// into the Plonky3 public values: an adversary who rewrites the
/// envelope's `public_inputs` field while leaving the proof bytes intact
/// triggers [`VerifyError::InvalidProof`] because the recomputed digest no
/// longer matches what the proof attests to. Conditions (4),
/// [`VerifyError::WrongSubTreeRoot`] and [`VerifyError::DepthMismatch`]
/// were added in PR #2 and run before Plonky3 verification, closing a
/// stale-root substitution path.
///
/// # Limitations
///
/// v0.1 restricts `payload` to ≤ 64 bytes (one Blake3 compression block);
/// a proof for a longer payload cannot be produced by the v0.1 prover, and
/// the verifier rejects with [`VerifyError::InvalidProof`] if the trace's
/// payload exceeds that length. Variable-length leaf preimages require
/// the v0.2 `LeafPreimageAir`, which is out of scope for this crate.
///
/// This function is pure: no I/O, no async, no global state, and no panic
/// on adversarial input.
pub fn verify(
    commitment: &OmegaCommitment,
    public_inputs: &[ClaimPublicInputs],
    proof: &ProofBytes,
) -> Result<(), VerifyError> {
    let envelope: ProofEnvelope =
        postcard::from_bytes(&proof.0).map_err(|_| VerifyError::InvalidProof)?;

    if envelope.version != PROOF_ENVELOPE_VERSION {
        return Err(VerifyError::UnsupportedVersion {
            version: envelope.version,
        });
    }
    if envelope.commitment != *commitment {
        return Err(VerifyError::CommitmentMismatch);
    }
    if envelope.public_inputs != public_inputs {
        return Err(VerifyError::PublicInputMismatch);
    }
    validate_public_inputs(commitment, public_inputs)?;
    validate_envelope_public_values(commitment, public_inputs, &envelope.public_values)?;
    verify_membership_batch(&envelope)
}

fn validate_public_inputs(
    commitment: &OmegaCommitment,
    public_inputs: &[ClaimPublicInputs],
) -> Result<(), VerifyError> {
    for (index, public) in public_inputs.iter().enumerate() {
        if public.bundle_root_blake3 != commitment.bundle_root_blake3 {
            return Err(VerifyError::PublicBundleRootMismatch { index });
        }
        let sub_tree_index =
            sub_tree_index(public.sub_tree_id).ok_or(VerifyError::UnknownSubTree {
                index,
                sub_tree_id: public.sub_tree_id,
            })?;
        if public.per_sub_tree_root != commitment.sub_tree_roots_blake3[sub_tree_index] {
            return Err(VerifyError::WrongSubTreeRoot { index });
        }
        let expected = u8::try_from(commitment.tree_depths[sub_tree_index])
            .map_err(|_| VerifyError::InvalidProof)?;
        if public.tree_depth != expected {
            return Err(VerifyError::DepthMismatch {
                index,
                expected,
                actual: public.tree_depth,
            });
        }
    }
    Ok(())
}

fn validate_envelope_public_values(
    commitment: &OmegaCommitment,
    public_inputs: &[ClaimPublicInputs],
    public_values: &[Vec<u32>],
) -> Result<(), VerifyError> {
    if public_values.len() != public_inputs.len() {
        return Err(VerifyError::InvalidProof);
    }
    let binding_words = proof_binding_words(commitment, public_inputs);
    for (values, public) in public_values.iter().zip(public_inputs) {
        if values.len() != PROOF_PUBLIC_VALUE_COUNT {
            return Err(VerifyError::InvalidProof);
        }
        if values[PUBLIC_SUB_TREE_ID_OFFSET] != u32::from(public.sub_tree_id) {
            return Err(VerifyError::InvalidProof);
        }
        for (offset, byte) in public.leaf_index.to_be_bytes().iter().enumerate() {
            if values[PUBLIC_LEAF_INDEX_OFFSET + offset] != u32::from(*byte) {
                return Err(VerifyError::InvalidProof);
            }
        }
        if values[PUBLIC_TREE_DEPTH_OFFSET] != u32::from(public.tree_depth) {
            return Err(VerifyError::InvalidProof);
        }
        for (offset, byte) in public.per_sub_tree_root.iter().enumerate() {
            if values[PUBLIC_ROOT_OFFSET + offset] != u32::from(*byte) {
                return Err(VerifyError::InvalidProof);
            }
        }
        if values[PROOF_BINDING_WORD_OFFSET..] != binding_words {
            return Err(VerifyError::InvalidProof);
        }
    }
    Ok(())
}

fn verify_membership_batch(envelope: &ProofEnvelope) -> Result<(), VerifyError> {
    let proof: BatchProof<OmegaStarkConfig> =
        postcard::from_bytes(&envelope.stark_proof).map_err(|_| VerifyError::InvalidProof)?;
    let mut airs = proof_airs(envelope.public_inputs.len());
    if proof.degree_bits.len() != airs.len() {
        return Err(VerifyError::InvalidProof);
    }
    let public_values = envelope
        .public_values
        .iter()
        .map(|values| {
            values
                .iter()
                .copied()
                .map(Val::from_u32)
                .collect::<Vec<_>>()
        })
        .chain(core::iter::once(Vec::new()))
        .collect::<Vec<_>>();
    let config = make_stark_config(envelope.config.rng_seed);
    let prover_data = ProverData::from_airs_and_degrees(&config, &mut airs, &proof.degree_bits);
    verify_batch(&config, &airs, &proof, &public_values, &prover_data.common)
        .map_err(|_| VerifyError::InvalidProof)
}

fn sub_tree_index(sub_tree_id: u8) -> Option<usize> {
    let index = sub_tree_id.checked_sub(1)? as usize;
    (index < 7).then_some(index)
}

/// Builds the verifier's [`OmegaStarkConfig`] from the envelope's RNG seed.
///
/// This MUST be byte-identical to the prover's configuration builder; any
/// drift in permutations, MMCS arity, FRI parameters, or challenger
/// construction will reject every otherwise-valid proof. The seed travels
/// inside [`ProofEnvelope`] precisely so the verifier can reconstruct the
/// same Poseidon2 round constants the prover used. Keep this function in
/// lockstep with the private `make_stark_config` builder used by
/// [`omega_claim_prover::prove_collection`].
fn make_stark_config(rng_seed: u64) -> OmegaStarkConfig {
    let mut rng = SmallRng::seed_from_u64(rng_seed);
    let perm16 = Perm16::new_from_rng_128(&mut rng);
    let perm24 = Perm24::new_from_rng_128(&mut rng);
    let hash = Poseidon2Sponge::new(perm24.clone());
    let compress = Poseidon2Compression::new(perm16);
    let val_mmcs = ValMmcs::new(hash, compress, 3);
    let challenge_mmcs = ChallengeMmcs::new(val_mmcs.clone());
    let fri_params = FriParameters::new_benchmark_high_arity(challenge_mmcs);
    let pcs = Pcs::new(Dft::default(), val_mmcs, fri_params);
    let challenger = Challenger::new(perm24);
    OmegaStarkConfig::new(pcs, challenger)
}
