#![forbid(unsafe_code)]

//! Pure verifier for v0.1 Ω-Commitment membership proof envelopes.

use omega_claim_prover::{
    proof_binding_words, OmegaCommitment, OmegaMembershipAir, ProofEnvelope,
    PROOF_BINDING_WORD_OFFSET, PROOF_PUBLIC_VALUE_COUNT,
};
use omega_claim_tx::{ClaimPublicInputs, ProofBytes};
use p3_baby_bear::{BabyBear, Poseidon2BabyBear};
use p3_blake3_air::Blake3Air;
use p3_challenger::DuplexChallenger;
use p3_commit::ExtensionMmcs;
use p3_field::{extension::BinomialExtensionField, Field, PrimeCharacteristicRing};
use p3_fri::{FriParameters, TwoAdicFriPcs};
use p3_merkle_tree::MerkleTreeMmcs;
use p3_monty_31::dft::RecursiveDft;
use p3_symmetric::{PaddingFreeSponge, TruncatedPermutation};
use p3_uni_stark::{verify as verify_stark, Proof, StarkConfig};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use thiserror::Error;

const PROOF_ENVELOPE_VERSION: u8 = 1;

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
    #[error("public input {index} bundle root does not match the verifier commitment")]
    PublicBundleRootMismatch { index: usize },
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
    for (index, public) in public_inputs.iter().enumerate() {
        if public.bundle_root_blake3 != commitment.bundle_root_blake3 {
            return Err(VerifyError::PublicBundleRootMismatch { index });
        }
    }
    if envelope.public_values[PROOF_BINDING_WORD_OFFSET..]
        != proof_binding_words(commitment, public_inputs)
    {
        return Err(VerifyError::InvalidProof);
    }

    verify_membership_proof(&envelope)?;
    verify_blake3_compression_proof(&envelope)?;

    Ok(())
}

fn verify_membership_proof(envelope: &ProofEnvelope) -> Result<(), VerifyError> {
    let proof: Proof<OmegaStarkConfig> =
        postcard::from_bytes(&envelope.stark_proof).map_err(|_| VerifyError::InvalidProof)?;
    let trace_height = trace_height(&proof)?;
    let config = make_stark_config(trace_height, envelope.config.rng_seed);
    verify_stark(
        &config,
        &OmegaMembershipAir,
        &proof,
        &public_values_as_fields(envelope.public_values),
    )
    .map_err(|_| VerifyError::InvalidProof)
}

fn verify_blake3_compression_proof(envelope: &ProofEnvelope) -> Result<(), VerifyError> {
    let proof: Proof<OmegaStarkConfig> = postcard::from_bytes(&envelope.blake3_compression_proof)
        .map_err(|_| VerifyError::InvalidProof)?;
    let trace_height = trace_height(&proof)?;
    let config = make_stark_config(trace_height, envelope.config.rng_seed);
    verify_stark(&config, &Blake3Air {}, &proof, &[]).map_err(|_| VerifyError::InvalidProof)
}

fn trace_height(proof: &Proof<OmegaStarkConfig>) -> Result<usize, VerifyError> {
    if proof.degree_bits >= usize::BITS as usize {
        return Err(VerifyError::InvalidProof);
    }
    Ok(1usize << proof.degree_bits)
}

fn make_stark_config(trace_height: usize, rng_seed: u64) -> OmegaStarkConfig {
    let mut rng = SmallRng::seed_from_u64(rng_seed);
    let perm16 = Perm16::new_from_rng_128(&mut rng);
    let perm24 = Perm24::new_from_rng_128(&mut rng);
    let hash = Poseidon2Sponge::new(perm24.clone());
    let compress = Poseidon2Compression::new(perm16);
    let val_mmcs = ValMmcs::new(hash, compress, 3);
    let challenge_mmcs = ChallengeMmcs::new(val_mmcs.clone());
    let fri_params = FriParameters::new_benchmark_high_arity(challenge_mmcs);
    let dft = Dft::new(trace_height << 1);
    let pcs = Pcs::new(dft, val_mmcs, fri_params);
    let challenger = Challenger::new(perm24);
    OmegaStarkConfig::new(pcs, challenger)
}

fn public_values_as_fields(
    values: [u32; PROOF_PUBLIC_VALUE_COUNT],
) -> [Val; PROOF_PUBLIC_VALUE_COUNT] {
    values.map(Val::from_u32)
}
