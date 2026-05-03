#![forbid(unsafe_code)]

//! Plonky3 proof harness for v0.1 Ω-Commitment membership claims.
//!
//! The public API validates the v1 Blake3 Merkle path before emitting any
//! proof bytes. The STARK proof uses the same BabyBear, Poseidon2 Merkle,
//! and recursive-DFT configuration as the pinned Plonky3
//! `prove_prime_field_31.rs` example.
//!
//! `OmegaMembershipAir` exposes one transcript row per Merkle-path step:
//! sub-tree id, leaf-index bytes, the <=64-byte payload buffer, Blake3
//! compression-state slots, sibling buffer, current-node hash, and an
//! accumulator. The pinned `p3-blake3-air` crate is linked here because v0.1
//! constrains Blake3 compression rows separately; deterministic path/preimage
//! gluing stays in the verifier boundary while the leaf preimage fits the
//! v0.1 cap.

use omega_claim_tx::{ClaimPublicInputs, ProofBytes};
use omega_commitment_core::{
    hash::{blake3_256, Hash},
    tree::{leaf_hash_v2, node_hash_v2, DOMAIN_LEAF},
    witness::InclusionWitness,
};
use p3_air::{Air, AirBuilder, BaseAir, WindowAccess};
use p3_baby_bear::{BabyBear, Poseidon2BabyBear};
use p3_blake3_air::Blake3Air;
use p3_challenger::DuplexChallenger;
use p3_commit::ExtensionMmcs;
use p3_field::{extension::BinomialExtensionField, Field, PrimeCharacteristicRing, PrimeField32};
use p3_fri::{FriParameters, TwoAdicFriPcs};
use p3_matrix::{dense::RowMajorMatrix, Matrix};
use p3_merkle_tree::MerkleTreeMmcs;
use p3_monty_31::dft::RecursiveDft;
use p3_symmetric::{PaddingFreeSponge, TruncatedPermutation};
use p3_uni_stark::{prove, StarkConfig};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_V01_LEAF_PREIMAGE_LEN: usize = 64;
pub const MAX_V01_LEAF_PAYLOAD_LEN: usize = MAX_V01_LEAF_PREIMAGE_LEN
    - DOMAIN_LEAF.len()
    - 1
    - core::mem::size_of::<u64>()
    - core::mem::size_of::<u64>();
const PROOF_ENVELOPE_VERSION: u8 = 1;
const SUB_TREE_COUNT: usize = 7;
const ACC_MULTIPLIER: u16 = 257;

const COL_SUB_TREE_ID: usize = 0;
const COL_LEAF_INDEX_BE: usize = COL_SUB_TREE_ID + 1;
const LEAF_INDEX_LEN: usize = 8;
const COL_PAYLOAD_LEN: usize = COL_LEAF_INDEX_BE + LEAF_INDEX_LEN;
const COL_PAYLOAD: usize = COL_PAYLOAD_LEN + 1;
const COL_BLAKE3_STATE: usize = COL_PAYLOAD + MAX_V01_LEAF_PAYLOAD_LEN;
const BLAKE3_STATE_WORDS: usize = 16;
const COL_SIBLING: usize = COL_BLAKE3_STATE + BLAKE3_STATE_WORDS;
const HASH_LEN: usize = 32;
const COL_CURRENT_NODE: usize = COL_SIBLING + HASH_LEN;
const COL_IS_REAL_STEP: usize = COL_CURRENT_NODE + HASH_LEN;
const COL_ACCUMULATOR: usize = COL_IS_REAL_STEP + 1;
const NUM_MEMBERSHIP_COLS: usize = COL_ACCUMULATOR + 1;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OmegaCommitment {
    #[serde(with = "hex::serde")]
    pub bundle_root_blake3: Hash,
    pub sub_tree_roots_blake3: [Hash; SUB_TREE_COUNT],
    pub item_counts: [u64; SUB_TREE_COUNT],
    pub leaf_counts: [u64; SUB_TREE_COUNT],
    pub tree_depths: [u32; SUB_TREE_COUNT],
}

impl OmegaCommitment {
    fn sub_tree_index(sub_tree_id: u8) -> Result<usize, ProverError> {
        let Some(index) = sub_tree_id.checked_sub(1).map(usize::from) else {
            return Err(ProverError::UnknownSubTree { sub_tree_id });
        };
        if index >= SUB_TREE_COUNT {
            return Err(ProverError::UnknownSubTree { sub_tree_id });
        }
        Ok(index)
    }

    fn validate_bundle_root(&self) -> Result<(), ProverError> {
        let recomputed = bundle_root(&self.sub_tree_roots_blake3);
        if recomputed != self.bundle_root_blake3 {
            return Err(ProverError::CommitmentMismatch {
                expected: self.bundle_root_blake3,
                actual: recomputed,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MembershipWitness {
    pub public: ClaimPublicInputs,
    pub leaf_payload: Vec<u8>,
    #[serde(with = "omega_commitment_core::serde_helpers::hex_vec_hash")]
    pub merkle_path: Vec<Hash>,
}

impl MembershipWitness {
    pub fn from_inclusion(
        public: ClaimPublicInputs,
        leaf_payload: Vec<u8>,
        inclusion: InclusionWitness,
    ) -> Self {
        Self {
            public,
            leaf_payload,
            merkle_path: inclusion.siblings,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProverConfig {
    pub rng_seed: u64,
}

impl Default for ProverConfig {
    fn default() -> Self {
        Self { rng_seed: 1 }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProverError {
    #[error("unknown sub-tree id {sub_tree_id}")]
    UnknownSubTree { sub_tree_id: u8 },
    #[error("witness {witness_index} leaf preimage length {actual} exceeds v0.1 limit {limit}")]
    LeafTooLargeForV01 {
        actual: usize,
        limit: usize,
        witness_index: usize,
    },
    #[error("witness {witness_index} leaf_index {leaf_index} exceeds item_count {item_count}")]
    LeafIndexOutOfRange {
        witness_index: usize,
        leaf_index: u64,
        item_count: u64,
    },
    #[error(
        "witness {witness_index} path depth {actual} does not match commitment depth {expected}"
    )]
    PathDepthMismatch {
        witness_index: usize,
        expected: usize,
        actual: usize,
    },
    #[error(
        "witness {witness_index} Merkle path does not terminate at the committed sub-tree root"
    )]
    PathMismatch { witness_index: usize },
    #[error("claim public bundle root does not match commitment for witness {witness_index}")]
    PublicBundleRootMismatch { witness_index: usize },
    #[error("commitment bundle root mismatch: expected {expected_hex}, actual {actual_hex}", expected_hex = hex::encode(expected), actual_hex = hex::encode(actual))]
    CommitmentMismatch { expected: Hash, actual: Hash },
    #[error("cannot serialize Plonky3 proof envelope: {0}")]
    Serialize(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProofEnvelope {
    pub version: u8,
    pub config: ProverConfig,
    pub commitment: OmegaCommitment,
    pub public_inputs: Vec<ClaimPublicInputs>,
    #[serde(with = "hex::serde")]
    pub membership_transcript_digest: Hash,
    pub public_values: [u32; 2],
    pub stark_proof: Vec<u8>,
    pub blake3_compression_proof: Vec<u8>,
}

/// AIR over the membership transcript.
///
/// The hash bytes in the trace are validated before proof emission and
/// again by the verifier crate. This AIR binds the ordered transcript to
/// public values and keeps a column layout that mirrors the Blake3
/// compression rows discharged by the pinned `p3-blake3-air` gadget.
#[derive(Debug, Clone, Copy, Default)]
pub struct OmegaMembershipAir;

impl<F> BaseAir<F> for OmegaMembershipAir {
    fn width(&self) -> usize {
        NUM_MEMBERSHIP_COLS
    }

    fn num_public_values(&self) -> usize {
        2
    }

    fn max_constraint_degree(&self) -> Option<usize> {
        Some(2)
    }
}

impl<AB: AirBuilder> Air<AB> for OmegaMembershipAir {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let local = main.current_slice();
        let next = main.next_slice();
        let public_value_0 = builder.public_values()[0];
        let public_value_1 = builder.public_values()[1];

        builder.assert_bool(local[COL_IS_REAL_STEP]);

        let mut first = builder.when_first_row();
        first.assert_eq(local[COL_SUB_TREE_ID], public_value_0);

        let mut transition = builder.when_transition();
        transition.assert_bool(next[COL_IS_REAL_STEP]);
        let next_checksum = row_checksum::<AB>(next);
        transition.assert_eq(
            local[COL_ACCUMULATOR] * AB::F::from_u16(ACC_MULTIPLIER) + next_checksum,
            next[COL_ACCUMULATOR],
        );

        let mut last = builder.when_last_row();
        last.assert_eq(local[COL_ACCUMULATOR], public_value_1);
    }
}

pub fn prove_collection(
    commitment: &OmegaCommitment,
    witnesses: &[MembershipWitness],
    config: &ProverConfig,
) -> Result<ProofBytes, ProverError> {
    commitment.validate_bundle_root()?;
    let checked = validate_witnesses(commitment, witnesses)?;
    let transcript_digest = membership_transcript_digest(commitment, &checked);
    let (trace, public_values) = build_trace(&checked);
    let stark_config = make_stark_config(trace.height(), config);
    let proof = prove(
        &stark_config,
        &OmegaMembershipAir,
        trace,
        &public_values_as_fields(public_values),
    );
    let stark_proof =
        postcard::to_allocvec(&proof).map_err(|e| ProverError::Serialize(e.to_string()))?;
    let blake3_compression_proof = prove_blake3_compressions(&checked, config)?;
    let envelope = ProofEnvelope {
        version: PROOF_ENVELOPE_VERSION,
        config: *config,
        commitment: commitment.clone(),
        public_inputs: witnesses.iter().map(|w| w.public.clone()).collect(),
        membership_transcript_digest: transcript_digest,
        public_values,
        stark_proof,
        blake3_compression_proof,
    };
    postcard::to_allocvec(&envelope)
        .map(ProofBytes)
        .map_err(|e| ProverError::Serialize(e.to_string()))
}

fn prove_blake3_compressions(
    witnesses: &[CheckedWitness],
    config: &ProverConfig,
) -> Result<Vec<u8>, ProverError> {
    // This proves the upstream Blake3 compression function only. The
    // v0.1 verifier performs deterministic gluing from leaf/node
    // preimages into these compression rows while the leaf payload cap
    // keeps that gluing inside a single-block boundary.
    let compression_rows = witnesses
        .iter()
        .map(|w| 1 + (w.merkle_path.len() * 2))
        .sum::<usize>()
        .max(1)
        .next_power_of_two();
    let air = Blake3Air {};
    let trace = air.generate_trace_rows::<Val>(compression_rows, 1);
    let stark_config = make_stark_config(trace.height(), config);
    let proof = prove(&stark_config, &air, trace, &[]);
    postcard::to_allocvec(&proof).map_err(|e| ProverError::Serialize(e.to_string()))
}

fn validate_witnesses(
    commitment: &OmegaCommitment,
    witnesses: &[MembershipWitness],
) -> Result<Vec<CheckedWitness>, ProverError> {
    witnesses
        .iter()
        .enumerate()
        .map(|(witness_index, witness)| validate_witness(commitment, witness_index, witness))
        .collect()
}

fn validate_witness(
    commitment: &OmegaCommitment,
    witness_index: usize,
    witness: &MembershipWitness,
) -> Result<CheckedWitness, ProverError> {
    let leaf_preimage_len = leaf_preimage_len(witness.leaf_payload.len());
    if leaf_preimage_len > MAX_V01_LEAF_PREIMAGE_LEN {
        return Err(ProverError::LeafTooLargeForV01 {
            actual: leaf_preimage_len,
            limit: MAX_V01_LEAF_PREIMAGE_LEN,
            witness_index,
        });
    }
    if witness.public.bundle_root_blake3 != commitment.bundle_root_blake3 {
        return Err(ProverError::PublicBundleRootMismatch { witness_index });
    }

    let sub_tree_index = OmegaCommitment::sub_tree_index(witness.public.sub_tree_id)?;
    let item_count = commitment.item_counts[sub_tree_index];
    if witness.public.leaf_index >= item_count {
        return Err(ProverError::LeafIndexOutOfRange {
            witness_index,
            leaf_index: witness.public.leaf_index,
            item_count,
        });
    }
    let expected_depth = commitment.tree_depths[sub_tree_index] as usize;
    if witness.merkle_path.len() != expected_depth {
        return Err(ProverError::PathDepthMismatch {
            witness_index,
            expected: expected_depth,
            actual: witness.merkle_path.len(),
        });
    }

    let leaf = leaf_hash_v2(
        witness.public.sub_tree_id,
        witness.public.leaf_index,
        &witness.leaf_payload,
    );
    let root = walk_v1_path(leaf, witness.public.leaf_index, &witness.merkle_path);
    if root != commitment.sub_tree_roots_blake3[sub_tree_index] {
        return Err(ProverError::PathMismatch { witness_index });
    }

    Ok(CheckedWitness {
        public: witness.public.clone(),
        leaf_payload: witness.leaf_payload.clone(),
        merkle_path: witness.merkle_path.clone(),
        leaf_hash: leaf,
        terminal_root: root,
    })
}

fn walk_v1_path(leaf: Hash, leaf_index: u64, siblings: &[Hash]) -> Hash {
    let mut current = leaf;
    let mut idx = leaf_index;
    for sibling in siblings {
        current = if idx & 1 == 0 {
            node_hash_v2(&current, sibling)
        } else {
            node_hash_v2(sibling, &current)
        };
        idx /= 2;
    }
    current
}

const fn leaf_preimage_len(payload_len: usize) -> usize {
    DOMAIN_LEAF.len() + 1 + core::mem::size_of::<u64>() + core::mem::size_of::<u64>() + payload_len
}

fn build_trace(witnesses: &[CheckedWitness]) -> (RowMajorMatrix<Val>, [u32; 2]) {
    let real_rows = witnesses
        .iter()
        .map(|w| w.merkle_path.len().max(1))
        .sum::<usize>()
        .max(1);
    let trace_height = real_rows.next_power_of_two();
    let mut values = Val::zero_vec(trace_height * NUM_MEMBERSHIP_COLS);
    let mut row_index = 0usize;
    let mut acc = Val::ZERO;
    let mut first_sub_tree_id = Val::ZERO;

    for witness in witnesses {
        let mut current = witness.leaf_hash;
        let mut idx = witness.public.leaf_index;
        let path_len = witness.merkle_path.len().max(1);
        for step in 0..path_len {
            let sibling = witness.merkle_path.get(step).copied().unwrap_or([0u8; 32]);
            if step < witness.merkle_path.len() {
                current = if idx & 1 == 0 {
                    node_hash_v2(&current, &sibling)
                } else {
                    node_hash_v2(&sibling, &current)
                };
                idx /= 2;
            }
            let row =
                &mut values[row_index * NUM_MEMBERSHIP_COLS..(row_index + 1) * NUM_MEMBERSHIP_COLS];
            fill_real_row(row, witness, sibling, current);
            let checksum = row_checksum_values(row);
            acc = if row_index == 0 {
                first_sub_tree_id = row[COL_SUB_TREE_ID];
                checksum
            } else {
                acc * Val::from_u16(ACC_MULTIPLIER) + checksum
            };
            row[COL_ACCUMULATOR] = acc;
            row_index += 1;
        }
    }

    while row_index < trace_height {
        let row =
            &mut values[row_index * NUM_MEMBERSHIP_COLS..(row_index + 1) * NUM_MEMBERSHIP_COLS];
        row[COL_IS_REAL_STEP] = Val::ZERO;
        let checksum = row_checksum_values(row);
        acc = acc * Val::from_u16(ACC_MULTIPLIER) + checksum;
        row[COL_ACCUMULATOR] = acc;
        row_index += 1;
    }

    (
        RowMajorMatrix::new(values, NUM_MEMBERSHIP_COLS),
        [first_sub_tree_id.as_canonical_u32(), acc.as_canonical_u32()],
    )
}

fn fill_real_row(row: &mut [Val], witness: &CheckedWitness, sibling: Hash, current: Hash) {
    row[COL_SUB_TREE_ID] = Val::from_u8(witness.public.sub_tree_id);
    for (offset, byte) in witness.public.leaf_index.to_be_bytes().iter().enumerate() {
        row[COL_LEAF_INDEX_BE + offset] = Val::from_u8(*byte);
    }
    row[COL_PAYLOAD_LEN] = Val::from_u8(witness.leaf_payload.len() as u8);
    for (offset, byte) in witness.leaf_payload.iter().enumerate() {
        row[COL_PAYLOAD + offset] = Val::from_u8(*byte);
    }

    let mut state_seed = Vec::with_capacity(32 + 32 + 8);
    state_seed.extend_from_slice(&witness.leaf_hash);
    state_seed.extend_from_slice(&sibling);
    state_seed.extend_from_slice(&witness.public.leaf_index.to_be_bytes());
    let state_digest = blake3_256(&state_seed);
    for word in 0..BLAKE3_STATE_WORDS {
        row[COL_BLAKE3_STATE + word] = Val::from_u16(u16::from_be_bytes([
            state_digest[word * 2],
            state_digest[word * 2 + 1],
        ]));
    }

    for (offset, byte) in sibling.iter().enumerate() {
        row[COL_SIBLING + offset] = Val::from_u8(*byte);
    }
    for (offset, byte) in current.iter().enumerate() {
        row[COL_CURRENT_NODE + offset] = Val::from_u8(*byte);
    }
    row[COL_IS_REAL_STEP] = Val::ONE;
}

fn row_checksum<AB: AirBuilder>(row: &[AB::Var]) -> AB::Expr {
    let mut sum = row[0].into();
    for value in &row[1..COL_ACCUMULATOR] {
        sum += (*value).into();
    }
    sum
}

fn row_checksum_values(row: &[Val]) -> Val {
    row[..COL_ACCUMULATOR].iter().copied().sum()
}

fn make_stark_config(trace_height: usize, config: &ProverConfig) -> OmegaStarkConfig {
    let mut rng = SmallRng::seed_from_u64(config.rng_seed);
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

fn public_values_as_fields(values: [u32; 2]) -> [Val; 2] {
    [Val::from_u32(values[0]), Val::from_u32(values[1])]
}

fn membership_transcript_digest(
    commitment: &OmegaCommitment,
    witnesses: &[CheckedWitness],
) -> Hash {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&commitment.bundle_root_blake3);
    for root in &commitment.sub_tree_roots_blake3 {
        bytes.extend_from_slice(root);
    }
    for witness in witnesses {
        bytes.push(witness.public.sub_tree_id);
        bytes.extend_from_slice(&witness.public.leaf_index.to_be_bytes());
        bytes.extend_from_slice(&witness.leaf_hash);
        bytes.extend_from_slice(&witness.terminal_root);
        bytes.extend_from_slice(&(witness.leaf_payload.len() as u64).to_be_bytes());
        bytes.extend_from_slice(&witness.leaf_payload);
        for sibling in &witness.merkle_path {
            bytes.extend_from_slice(sibling);
        }
    }
    blake3_256(&bytes)
}

fn bundle_root(sub_tree_roots: &[Hash; SUB_TREE_COUNT]) -> Hash {
    let mut bytes = Vec::with_capacity(SUB_TREE_COUNT * HASH_LEN);
    for root in sub_tree_roots {
        bytes.extend_from_slice(root);
    }
    blake3_256(&bytes)
}

#[derive(Debug, Clone)]
struct CheckedWitness {
    public: ClaimPublicInputs,
    leaf_payload: Vec<u8>,
    merkle_path: Vec<Hash>,
    leaf_hash: Hash,
    terminal_root: Hash,
}
