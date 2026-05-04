#![forbid(unsafe_code)]

//! Pure verifier for v0.1 Ω-Commitment membership proof envelopes.

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

type Val = BabyBear;
type Challenge = BinomialExtensionField<Val, 4>;
type Dft = RecursiveDft<Val>;
type Perm16 = Poseidon2BabyBear<16>;
type Perm24 = Poseidon2BabyBear<24>;
type Poseidon2Sponge = PaddingFreeSponge<Perm24, 24, 16, 8>;
type Poseidon2Compression = TruncatedPermutation<Perm16, 2, 8, 16>;
type ValMmcs = MerkleTreeMmcs<
    <Val as Field>::Packing,
    <Val as Field>::Packing,
    Poseidon2Sponge,
    Poseidon2Compression,
    2,
    8,
>;
type ChallengeMmcs = ExtensionMmcs<Val, Challenge, ValMmcs>;
type Pcs = TwoAdicFriPcs<Val, Dft, ValMmcs, ChallengeMmcs>;
type Challenger = DuplexChallenger<Val, Perm24, 24, 16>;
type OmegaStarkConfig = StarkConfig<Pcs, Challenge, Challenger>;

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum VerifyError {
    #[error("unsupported proof envelope version {version}")]
    UnsupportedVersion { version: u8 },
    #[error("proof commitment does not match verifier commitment")]
    CommitmentMismatch,
    #[error("proof public inputs do not match verifier public inputs")]
    PublicInputMismatch,
    #[error("public input {index} references unknown sub-tree id {sub_tree_id}")]
    UnknownSubTree { index: usize, sub_tree_id: u8 },
    #[error("public input {index} bundle root does not match the verifier commitment")]
    PublicBundleRootMismatch { index: usize },
    #[error("public input {index} per-sub-tree root does not match the verifier commitment")]
    WrongSubTreeRoot { index: usize },
    #[error("public input {index} tree depth mismatch: expected {expected}, actual {actual}")]
    DepthMismatch {
        index: usize,
        expected: u8,
        actual: u8,
    },
    #[error("invalid Plonky3 proof")]
    InvalidProof,
}

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
