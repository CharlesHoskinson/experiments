#![forbid(unsafe_code)]

//! Plonky3 proof harness for v0.1 Ω-Commitment membership claims.
//!
//! v0.1 proves Merkle inclusion for leaf preimages whose full
//! `omega:v2:leaf || sub_tree_id || leaf_index_be || payload_len_be || payload`
//! preimage is at most one Blake3 compression block (64 bytes). That keeps the
//! leaf preimage gluing explicit and small while `p3-blake3-air` constrains the
//! Blake3 compression rows. Variable-length, multi-block leaf preimages are a
//! v0.2 `LeafPreimageAir` task.

mod blake3_trace;

use blake3_trace::{
    build_blake3_trace, hash_from_words, leaf_compression, node_compressions, B3_BLOCK_LEN_OFFSET,
    B3_CHAINING_VALUES_OFFSET, B3_COUNTER_HI_OFFSET, B3_COUNTER_LOW_OFFSET, B3_FLAGS_OFFSET,
    B3_INPUTS_OFFSET, B3_OUTPUTS_OFFSET, BLAKE3_LOOKUP_NAME, COMPRESSION_LOOKUP_WIDTH,
    COUNTER_BYTES, IV, LEAF_FLAGS, NODE_FIRST_FLAGS, NODE_SECOND_FLAGS, U32_BYTES,
};
use omega_claim_tx::{ClaimPublicInputs, ProofBytes};
use omega_commitment_core::{
    hash::{blake3_256, Hash},
    tree::{leaf_hash_v2, node_hash_v2, DOMAIN_LEAF, DOMAIN_NODE},
    witness::InclusionWitness,
};
use p3_air::{
    Air, AirBuilder, AirLayout, BaseAir, BaseLeaf, SymbolicAirBuilder, SymbolicExpression,
    WindowAccess,
};
use p3_baby_bear::{BabyBear, Poseidon2BabyBear};
use p3_batch_stark::{prove_batch, ProverData, StarkInstance};
use p3_blake3_air::{Blake3Air, NUM_BLAKE3_COLS};
use p3_challenger::DuplexChallenger;
use p3_commit::ExtensionMmcs;
use p3_field::{extension::BinomialExtensionField, Field, PrimeCharacteristicRing};
use p3_fri::{FriParameters, TwoAdicFriPcs};
use p3_lookup::{Direction, Kind, Lookup, LookupAir};
use p3_matrix::{dense::RowMajorMatrix, Matrix};
use p3_merkle_tree::MerkleTreeMmcs;
use p3_monty_31::dft::RecursiveDft;
use p3_symmetric::{PaddingFreeSponge, TruncatedPermutation};
use p3_uni_stark::StarkConfig;
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
pub const PROOF_ENVELOPE_VERSION: u8 = 2;
pub const PUBLIC_SUB_TREE_ID_OFFSET: usize = 0;
pub const PUBLIC_LEAF_INDEX_OFFSET: usize = PUBLIC_SUB_TREE_ID_OFFSET + 1;
pub const PUBLIC_TREE_DEPTH_OFFSET: usize = PUBLIC_LEAF_INDEX_OFFSET + LEAF_INDEX_LEN;
pub const PUBLIC_ROOT_OFFSET: usize = PUBLIC_TREE_DEPTH_OFFSET + 1;
pub const PROOF_BINDING_WORD_OFFSET: usize = PUBLIC_ROOT_OFFSET + HASH_LEN;
pub const PROOF_BINDING_WORDS: usize = 32;
pub const PROOF_PUBLIC_VALUE_COUNT: usize = PROOF_BINDING_WORD_OFFSET + PROOF_BINDING_WORDS;

const SUB_TREE_COUNT: usize = 7;
const HASH_LEN: usize = 32;
const LEAF_INDEX_LEN: usize = 8;
const LEAF_PAYLOAD_LEN_CHOICES: usize = MAX_V01_LEAF_PAYLOAD_LEN + 1;
const DOMAIN_PROOF_BINDING: &[u8] = b"omega:proof:v2:binding";

const COL_SUB_TREE_ID: usize = 0;
const COL_LEAF_INDEX_BE: usize = COL_SUB_TREE_ID + 1;
const COL_REMAINING_INDEX_BITS: usize = COL_LEAF_INDEX_BE + LEAF_INDEX_LEN;
const REMAINING_INDEX_BITS: usize = 64;
const COL_PATH_STEP_INDEX: usize = COL_REMAINING_INDEX_BITS + REMAINING_INDEX_BITS;
const COL_TREE_DEPTH: usize = COL_PATH_STEP_INDEX + 1;
const COL_PAYLOAD_LEN: usize = COL_TREE_DEPTH + 1;
const COL_PAYLOAD_LEN_SELECTOR: usize = COL_PAYLOAD_LEN + 1;
const COL_PAYLOAD: usize = COL_PAYLOAD_LEN_SELECTOR + LEAF_PAYLOAD_LEN_CHOICES;
const COL_PREV_NODE: usize = COL_PAYLOAD + MAX_V01_LEAF_PAYLOAD_LEN;
const COL_SIBLING: usize = COL_PREV_NODE + HASH_LEN;
const COL_LEFT_NODE: usize = COL_SIBLING + HASH_LEN;
const COL_RIGHT_NODE: usize = COL_LEFT_NODE + HASH_LEN;
const COL_NODE_MID: usize = COL_RIGHT_NODE + HASH_LEN;
const COL_CURRENT_NODE: usize = COL_NODE_MID + HASH_LEN;
const COL_DIRECTION_BIT: usize = COL_CURRENT_NODE + HASH_LEN;
const COL_HAS_NODE_HASH: usize = COL_DIRECTION_BIT + 1;
const COL_IS_FIRST_STEP: usize = COL_HAS_NODE_HASH + 1;
const COL_IS_REAL_STEP: usize = COL_IS_FIRST_STEP + 1;
const COL_IS_LAST_STEP: usize = COL_IS_REAL_STEP + 1;
const NUM_MEMBERSHIP_COLS: usize = COL_IS_LAST_STEP + 1;

pub type Val = BabyBear;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceTamper {
    PayloadByte,
    SiblingByte,
    CurrentNodeByte,
    LeafIndexByte,
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
    #[error("claim public sub-tree root does not match commitment for witness {witness_index}")]
    PublicSubTreeRootMismatch { witness_index: usize },
    #[error("claim public tree depth does not match commitment for witness {witness_index}")]
    PublicTreeDepthMismatch { witness_index: usize },
    #[error(
        "commitment tree depth {depth} for witness {witness_index} exceeds v0.1 u8 public input"
    )]
    TreeDepthTooLarge { witness_index: usize, depth: u32 },
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
    pub public_values: Vec<Vec<u32>>,
    pub stark_proof: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct OmegaMembershipAir {
    num_lookups: usize,
}

#[derive(Debug, Clone, Default)]
pub struct OmegaBlake3Air {
    num_lookups: usize,
}

#[derive(Debug, Clone)]
pub enum OmegaProofAir {
    Membership(OmegaMembershipAir),
    Blake3(OmegaBlake3Air),
}

/// Returns one membership AIR per public input followed by the shared Blake3
/// compression AIR used by the global LogUp interaction.
pub fn proof_airs(membership_count: usize) -> Vec<OmegaProofAir> {
    let mut airs = Vec::with_capacity(membership_count + 1);
    airs.extend(
        core::iter::repeat_with(|| OmegaProofAir::Membership(OmegaMembershipAir::default()))
            .take(membership_count),
    );
    airs.push(OmegaProofAir::Blake3(OmegaBlake3Air::default()));
    airs
}

impl<F> BaseAir<F> for OmegaMembershipAir {
    fn width(&self) -> usize {
        NUM_MEMBERSHIP_COLS
    }

    fn num_public_values(&self) -> usize {
        PROOF_PUBLIC_VALUE_COUNT
    }
}

impl<F> BaseAir<F> for OmegaBlake3Air {
    fn width(&self) -> usize {
        NUM_BLAKE3_COLS
    }
}

impl<F> BaseAir<F> for OmegaProofAir {
    fn width(&self) -> usize {
        match self {
            Self::Membership(air) => <OmegaMembershipAir as BaseAir<F>>::width(air),
            Self::Blake3(air) => <OmegaBlake3Air as BaseAir<F>>::width(air),
        }
    }

    fn num_public_values(&self) -> usize {
        match self {
            Self::Membership(air) => <OmegaMembershipAir as BaseAir<F>>::num_public_values(air),
            Self::Blake3(air) => <OmegaBlake3Air as BaseAir<F>>::num_public_values(air),
        }
    }

    fn max_constraint_degree(&self) -> Option<usize> {
        match self {
            Self::Membership(air) => <OmegaMembershipAir as BaseAir<F>>::max_constraint_degree(air),
            Self::Blake3(air) => <OmegaBlake3Air as BaseAir<F>>::max_constraint_degree(air),
        }
    }
}

impl<AB: AirBuilder> Air<AB> for OmegaMembershipAir {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let local = main.current_slice();
        let next = main.next_slice();
        let public = core::array::from_fn::<_, PROOF_PUBLIC_VALUE_COUNT, _>(|index| {
            builder.public_values()[index]
        });

        builder.assert_bool(local[COL_IS_REAL_STEP]);
        builder.assert_bool(local[COL_IS_LAST_STEP]);
        builder.assert_bool(local[COL_IS_FIRST_STEP]);
        builder.assert_bool(local[COL_HAS_NODE_HASH]);
        builder.assert_bool(local[COL_DIRECTION_BIT]);

        let real = local[COL_IS_REAL_STEP];
        let not_real = AB::Expr::ONE - real.into();
        let first_step = local[COL_IS_FIRST_STEP];
        let last_step = local[COL_IS_LAST_STEP];
        let has_node = local[COL_HAS_NODE_HASH];
        let node_active = has_node;
        let no_node_active = real - has_node;
        let continue_active = real * (AB::Expr::ONE - last_step.into());
        let stop_active = real * last_step;

        let mut first = builder.when_first_row();
        first.assert_one(real);
        first.assert_one(first_step);
        first.assert_zero(local[COL_PATH_STEP_INDEX]);

        let mut real_builder = builder.when(real);
        real_builder.assert_eq(local[COL_SUB_TREE_ID], public[PUBLIC_SUB_TREE_ID_OFFSET]);
        real_builder.assert_eq(local[COL_TREE_DEPTH], public[PUBLIC_TREE_DEPTH_OFFSET]);
        for offset in 0..LEAF_INDEX_LEN {
            real_builder.assert_eq(
                local[COL_LEAF_INDEX_BE + offset],
                public[PUBLIC_LEAF_INDEX_OFFSET + offset],
            );
        }
        assert_payload_len_selector(&mut real_builder, local);
        for bit in 0..REMAINING_INDEX_BITS {
            real_builder.assert_bool(local[COL_REMAINING_INDEX_BITS + bit]);
        }

        let mut first_builder = builder.when(first_step);
        first_builder.assert_one(real);
        assert_remaining_bits_match_leaf_index(&mut first_builder, local, &public);

        let mut node_builder = builder.when(node_active);
        node_builder.assert_eq(local[COL_DIRECTION_BIT], local[COL_REMAINING_INDEX_BITS]);
        let direction_zero_active = has_node * (AB::Expr::ONE - local[COL_DIRECTION_BIT].into());
        let direction_one_active = has_node * local[COL_DIRECTION_BIT];
        {
            let mut direction_zero = builder.when(direction_zero_active);
            for byte in 0..HASH_LEN {
                direction_zero.assert_eq(local[COL_LEFT_NODE + byte], local[COL_PREV_NODE + byte]);
                direction_zero.assert_eq(local[COL_RIGHT_NODE + byte], local[COL_SIBLING + byte]);
            }
        }
        {
            let mut direction_one = builder.when(direction_one_active);
            for byte in 0..HASH_LEN {
                direction_one.assert_eq(local[COL_LEFT_NODE + byte], local[COL_SIBLING + byte]);
                direction_one.assert_eq(local[COL_RIGHT_NODE + byte], local[COL_PREV_NODE + byte]);
            }
        }
        let mut no_node_builder = builder.when(no_node_active);
        no_node_builder.assert_zero(local[COL_TREE_DEPTH]);
        for byte in 0..HASH_LEN {
            no_node_builder.assert_eq(local[COL_CURRENT_NODE + byte], local[COL_PREV_NODE + byte]);
            no_node_builder.assert_zero(local[COL_SIBLING + byte]);
            no_node_builder.assert_zero(local[COL_LEFT_NODE + byte]);
            no_node_builder.assert_zero(local[COL_RIGHT_NODE + byte]);
            no_node_builder.assert_zero(local[COL_NODE_MID + byte]);
        }

        let mut last_builder = builder.when(stop_active.clone());
        last_builder.assert_eq(local[COL_PATH_STEP_INDEX] + has_node, local[COL_TREE_DEPTH]);
        for byte in 0..HASH_LEN {
            last_builder.assert_eq(
                local[COL_CURRENT_NODE + byte],
                public[PUBLIC_ROOT_OFFSET + byte],
            );
        }

        let mut transition = builder.when_transition();
        transition.assert_zero(next[COL_IS_FIRST_STEP]);
        transition
            .when(continue_active.clone())
            .assert_one(next[COL_IS_REAL_STEP]);
        transition
            .when(stop_active.clone())
            .assert_zero(next[COL_IS_REAL_STEP]);
        transition
            .when(not_real.clone())
            .assert_zero(next[COL_IS_REAL_STEP]);
        transition
            .when(not_real)
            .assert_zero(next[COL_IS_FIRST_STEP]);

        let mut transition_window = builder.when_transition();
        let mut continue_builder = transition_window.when(continue_active);
        continue_builder.assert_eq(
            next[COL_PATH_STEP_INDEX],
            local[COL_PATH_STEP_INDEX] + AB::F::ONE,
        );
        for byte in 0..HASH_LEN {
            continue_builder.assert_eq(next[COL_PREV_NODE + byte], local[COL_CURRENT_NODE + byte]);
        }
        for bit in 0..(REMAINING_INDEX_BITS - 1) {
            continue_builder.assert_eq(
                next[COL_REMAINING_INDEX_BITS + bit],
                local[COL_REMAINING_INDEX_BITS + bit + 1],
            );
        }
        continue_builder.assert_zero(next[COL_REMAINING_INDEX_BITS + REMAINING_INDEX_BITS - 1]);

        let mut last_row = builder.when_last_row();
        last_row.assert_zero(real * (AB::Expr::ONE - last_step.into()));

        let mut padding = builder.when(AB::Expr::ONE - local[COL_IS_REAL_STEP].into());
        for value in local {
            padding.assert_zero(*value);
        }
    }
}

impl<AB: AirBuilder> Air<AB> for OmegaBlake3Air {
    fn eval(&self, builder: &mut AB) {
        Blake3Air {}.eval(builder);
    }
}

impl<AB: AirBuilder> Air<AB> for OmegaProofAir {
    fn eval(&self, builder: &mut AB) {
        match self {
            Self::Membership(air) => air.eval(builder),
            Self::Blake3(air) => air.eval(builder),
        }
    }
}

impl<F: Field> LookupAir<F> for OmegaMembershipAir {
    fn add_lookup_columns(&mut self) -> Vec<usize> {
        let column = self.num_lookups;
        self.num_lookups += 1;
        vec![column]
    }

    fn get_lookups(&mut self) -> Vec<Lookup<F>> {
        self.num_lookups = 0;
        let symbolic = SymbolicAirBuilder::<F>::new(AirLayout {
            main_width: NUM_MEMBERSHIP_COLS,
            num_public_values: PROOF_PUBLIC_VALUE_COUNT,
            ..Default::default()
        });
        let main = symbolic.main();
        let local = main.current_slice();
        let real = expr(local[COL_IS_REAL_STEP]);
        let has_node = expr(local[COL_HAS_NODE_HASH]);
        let node_mult = real * has_node;

        let leaf_lookup = LookupAir::register_lookup(
            self,
            Kind::Global(BLAKE3_LOOKUP_NAME.to_string()),
            &[(
                leaf_lookup_exprs(local),
                expr(local[COL_IS_FIRST_STEP]),
                Direction::Receive,
            )],
        );
        let node_first_lookup = LookupAir::register_lookup(
            self,
            Kind::Global(BLAKE3_LOOKUP_NAME.to_string()),
            &[(
                node_first_lookup_exprs(local),
                node_mult.clone(),
                Direction::Receive,
            )],
        );
        let node_second_lookup = LookupAir::register_lookup(
            self,
            Kind::Global(BLAKE3_LOOKUP_NAME.to_string()),
            &[(
                node_second_lookup_exprs(local),
                node_mult,
                Direction::Receive,
            )],
        );
        vec![leaf_lookup, node_first_lookup, node_second_lookup]
    }
}

impl<F: Field> LookupAir<F> for OmegaBlake3Air {
    fn add_lookup_columns(&mut self) -> Vec<usize> {
        let column = self.num_lookups;
        self.num_lookups += 1;
        vec![column]
    }

    fn get_lookups(&mut self) -> Vec<Lookup<F>> {
        self.num_lookups = 0;
        let symbolic = SymbolicAirBuilder::<F>::new(AirLayout {
            main_width: NUM_BLAKE3_COLS,
            ..Default::default()
        });
        let main = symbolic.main();
        let local = main.current_slice();
        let flag0 = expr(local[B3_FLAGS_OFFSET]);
        let flag1 = expr(local[B3_FLAGS_OFFSET + 1]);
        // Arithmetic OR over Blake3 flag bits: dummy padding rows have no
        // CHUNK_START or CHUNK_END bit set, so they do not send lookup tuples.
        let multiplicity = flag0.clone() + flag1.clone() - flag0 * flag1;
        let lookup = LookupAir::register_lookup(
            self,
            Kind::Global(BLAKE3_LOOKUP_NAME.to_string()),
            &[(
                blake3_table_lookup_exprs(local),
                multiplicity,
                Direction::Send,
            )],
        );
        vec![lookup]
    }
}

impl<F: Field> LookupAir<F> for OmegaProofAir {
    fn add_lookup_columns(&mut self) -> Vec<usize> {
        match self {
            Self::Membership(air) => LookupAir::<F>::add_lookup_columns(air),
            Self::Blake3(air) => LookupAir::<F>::add_lookup_columns(air),
        }
    }

    fn get_lookups(&mut self) -> Vec<Lookup<F>> {
        match self {
            Self::Membership(air) => LookupAir::<F>::get_lookups(air),
            Self::Blake3(air) => LookupAir::<F>::get_lookups(air),
        }
    }
}

pub fn prove_collection(
    commitment: &OmegaCommitment,
    witnesses: &[MembershipWitness],
    config: &ProverConfig,
) -> Result<ProofBytes, ProverError> {
    prove_collection_inner(commitment, witnesses, config, None)
}

#[doc(hidden)]
pub fn prove_collection_with_trace_tamper(
    commitment: &OmegaCommitment,
    witnesses: &[MembershipWitness],
    config: &ProverConfig,
    tamper: TraceTamper,
) -> Result<ProofBytes, ProverError> {
    prove_collection_inner(commitment, witnesses, config, Some(tamper))
}

fn prove_collection_inner(
    commitment: &OmegaCommitment,
    witnesses: &[MembershipWitness],
    config: &ProverConfig,
    tamper: Option<TraceTamper>,
) -> Result<ProofBytes, ProverError> {
    commitment.validate_bundle_root()?;
    let checked = validate_witnesses(commitment, witnesses)?;
    let public_inputs = witnesses
        .iter()
        .map(|w| w.public.clone())
        .collect::<Vec<_>>();
    let built = build_traces(commitment, &checked, tamper);
    let public_values = proof_public_values(commitment, &public_inputs);
    let public_values_fields = public_values
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

    let stark_config = make_stark_config(config);
    let mut airs = proof_airs(built.membership_traces.len());
    let mut traces = built.membership_traces.iter().collect::<Vec<_>>();
    traces.push(&built.blake3_trace);
    let degree_bits = traces
        .iter()
        .map(|trace| log2_power_of_two(trace.height()))
        .collect::<Vec<_>>();
    let prover_data = ProverData::from_airs_and_degrees(&stark_config, &mut airs, &degree_bits);
    let instances =
        StarkInstance::new_multiple(&airs, &traces, &public_values_fields, &prover_data.common);
    let proof = prove_batch(&stark_config, &instances, &prover_data);
    let stark_proof =
        postcard::to_allocvec(&proof).map_err(|e| ProverError::Serialize(e.to_string()))?;
    let envelope = ProofEnvelope {
        version: PROOF_ENVELOPE_VERSION,
        config: *config,
        commitment: commitment.clone(),
        public_inputs,
        public_values,
        stark_proof,
    };
    postcard::to_allocvec(&envelope)
        .map(ProofBytes)
        .map_err(|e| ProverError::Serialize(e.to_string()))
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
    let commitment_depth = commitment.tree_depths[sub_tree_index];
    let expected_depth = commitment_depth as usize;
    if commitment_depth > u8::MAX as u32 {
        return Err(ProverError::TreeDepthTooLarge {
            witness_index,
            depth: commitment_depth,
        });
    }
    if witness.public.tree_depth != commitment_depth as u8 {
        return Err(ProverError::PublicTreeDepthMismatch { witness_index });
    }
    if witness.public.per_sub_tree_root != commitment.sub_tree_roots_blake3[sub_tree_index] {
        return Err(ProverError::PublicSubTreeRootMismatch { witness_index });
    }
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
    })
}

fn build_traces(
    commitment: &OmegaCommitment,
    witnesses: &[CheckedWitness],
    tamper: Option<TraceTamper>,
) -> BuiltTraces {
    let mut compressions = Vec::new();
    let mut membership_traces = Vec::with_capacity(witnesses.len());

    for (index, witness) in witnesses.iter().enumerate() {
        let trace =
            build_membership_trace(witness, tamper.filter(|_| index == 0), &mut compressions);
        membership_traces.push(trace);
    }

    let _ = commitment;
    let blake3_trace = build_blake3_trace(&compressions);
    BuiltTraces {
        membership_traces,
        blake3_trace,
    }
}

fn build_membership_trace(
    witness: &CheckedWitness,
    tamper: Option<TraceTamper>,
    compressions: &mut Vec<blake3_trace::Compression>,
) -> RowMajorMatrix<Val> {
    let path_len = witness.merkle_path.len();
    let real_rows = path_len.max(1);
    let height = real_rows.next_power_of_two().max(8);
    let mut values = Val::zero_vec(height * NUM_MEMBERSHIP_COLS);
    let leaf_compression = leaf_compression(
        witness.public.sub_tree_id,
        witness.public.leaf_index,
        &witness.leaf_payload,
    );
    debug_assert_eq!(
        hash_from_words(leaf_compression.output_words),
        witness.leaf_hash
    );
    compressions.push(leaf_compression);

    let mut prev = witness.leaf_hash;
    let mut remaining_index = witness.public.leaf_index;
    for step in 0..real_rows {
        let has_node = path_len > 0;
        let sibling = witness
            .merkle_path
            .get(step)
            .copied()
            .unwrap_or([0u8; HASH_LEN]);
        let direction = if has_node {
            (remaining_index & 1) as u8
        } else {
            0
        };
        let (left, right) = if direction == 0 {
            (prev, sibling)
        } else {
            (sibling, prev)
        };
        let (mid, current) = if has_node {
            let [first, second] = node_compressions(&left, &right);
            let mid = hash_from_words(first.output_words);
            let current = hash_from_words(second.output_words);
            debug_assert_eq!(current, node_hash_v2(&left, &right));
            compressions.extend([first, second]);
            (mid, current)
        } else {
            ([0u8; HASH_LEN], prev)
        };

        let row = &mut values[step * NUM_MEMBERSHIP_COLS..(step + 1) * NUM_MEMBERSHIP_COLS];
        let real_row = RealRowValues {
            step,
            remaining_index,
            direction,
            has_node,
            prev,
            sibling,
            left,
            right,
            mid,
            current,
        };
        fill_real_row(row, witness, &real_row);
        prev = current;
        remaining_index >>= 1;
    }

    if let Some(tamper) = tamper {
        tamper_trace(&mut values[..NUM_MEMBERSHIP_COLS], tamper);
    }

    RowMajorMatrix::new(values, NUM_MEMBERSHIP_COLS)
}

struct RealRowValues {
    step: usize,
    remaining_index: u64,
    direction: u8,
    has_node: bool,
    prev: Hash,
    sibling: Hash,
    left: Hash,
    right: Hash,
    mid: Hash,
    current: Hash,
}

fn fill_real_row(row: &mut [Val], witness: &CheckedWitness, real_row: &RealRowValues) {
    row[COL_SUB_TREE_ID] = Val::from_u8(witness.public.sub_tree_id);
    for (offset, byte) in witness.public.leaf_index.to_be_bytes().iter().enumerate() {
        row[COL_LEAF_INDEX_BE + offset] = Val::from_u8(*byte);
    }
    for bit in 0..REMAINING_INDEX_BITS {
        row[COL_REMAINING_INDEX_BITS + bit] =
            Val::from_bool((real_row.remaining_index >> bit) & 1 == 1);
    }
    row[COL_PATH_STEP_INDEX] = Val::from_u32(real_row.step as u32);
    row[COL_TREE_DEPTH] = Val::from_u8(witness.public.tree_depth);
    row[COL_PAYLOAD_LEN] = Val::from_u8(witness.leaf_payload.len() as u8);
    row[COL_PAYLOAD_LEN_SELECTOR + witness.leaf_payload.len()] = Val::ONE;
    for (offset, byte) in witness.leaf_payload.iter().enumerate() {
        row[COL_PAYLOAD + offset] = Val::from_u8(*byte);
    }
    copy_hash(row, COL_PREV_NODE, real_row.prev);
    copy_hash(row, COL_SIBLING, real_row.sibling);
    copy_hash(row, COL_LEFT_NODE, real_row.left);
    copy_hash(row, COL_RIGHT_NODE, real_row.right);
    copy_hash(row, COL_NODE_MID, real_row.mid);
    copy_hash(row, COL_CURRENT_NODE, real_row.current);
    row[COL_DIRECTION_BIT] = Val::from_u8(real_row.direction);
    row[COL_HAS_NODE_HASH] = Val::from_bool(real_row.has_node);
    row[COL_IS_FIRST_STEP] = Val::from_bool(real_row.step == 0);
    row[COL_IS_REAL_STEP] = Val::ONE;
    row[COL_IS_LAST_STEP] = Val::from_bool(real_row.step + 1 == witness.merkle_path.len().max(1));
}

fn copy_hash(row: &mut [Val], offset: usize, hash: Hash) {
    for (index, byte) in hash.iter().enumerate() {
        row[offset + index] = Val::from_u8(*byte);
    }
}

fn tamper_trace(row: &mut [Val], tamper: TraceTamper) {
    let column = match tamper {
        TraceTamper::PayloadByte => COL_PAYLOAD,
        TraceTamper::SiblingByte => COL_SIBLING,
        TraceTamper::CurrentNodeByte => COL_CURRENT_NODE,
        TraceTamper::LeafIndexByte => COL_LEAF_INDEX_BE + LEAF_INDEX_LEN - 1,
    };
    row[column] += Val::ONE;
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

fn make_stark_config(config: &ProverConfig) -> OmegaStarkConfig {
    let mut rng = SmallRng::seed_from_u64(config.rng_seed);
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

fn log2_power_of_two(value: usize) -> usize {
    debug_assert!(value.is_power_of_two());
    value.trailing_zeros() as usize
}

fn proof_public_values(
    commitment: &OmegaCommitment,
    public_inputs: &[ClaimPublicInputs],
) -> Vec<Vec<u32>> {
    let binding_words = proof_binding_words(commitment, public_inputs);
    public_inputs
        .iter()
        .map(|public| {
            let mut values = [0u32; PROOF_PUBLIC_VALUE_COUNT];
            values[PUBLIC_SUB_TREE_ID_OFFSET] = u32::from(public.sub_tree_id);
            for (offset, byte) in public.leaf_index.to_be_bytes().iter().enumerate() {
                values[PUBLIC_LEAF_INDEX_OFFSET + offset] = u32::from(*byte);
            }
            values[PUBLIC_TREE_DEPTH_OFFSET] = u32::from(public.tree_depth);
            for (offset, byte) in public.per_sub_tree_root.iter().enumerate() {
                values[PUBLIC_ROOT_OFFSET + offset] = u32::from(*byte);
            }
            values[PROOF_BINDING_WORD_OFFSET..].copy_from_slice(&binding_words);
            values.to_vec()
        })
        .collect()
}

pub fn proof_binding_words(
    commitment: &OmegaCommitment,
    public_inputs: &[ClaimPublicInputs],
) -> [u32; PROOF_BINDING_WORDS] {
    let digest = proof_binding_digest(commitment, public_inputs);
    let mut words = [0u32; PROOF_BINDING_WORDS];
    for (word, byte) in words.iter_mut().zip(digest) {
        *word = u32::from(byte);
    }
    words
}

fn proof_binding_digest(commitment: &OmegaCommitment, public_inputs: &[ClaimPublicInputs]) -> Hash {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(DOMAIN_PROOF_BINDING);
    bytes.extend_from_slice(&commitment.bundle_root_blake3);
    for root in &commitment.sub_tree_roots_blake3 {
        bytes.extend_from_slice(root);
    }
    for count in commitment.item_counts {
        bytes.extend_from_slice(&count.to_be_bytes());
    }
    for count in commitment.leaf_counts {
        bytes.extend_from_slice(&count.to_be_bytes());
    }
    for depth in commitment.tree_depths {
        bytes.extend_from_slice(&depth.to_be_bytes());
    }
    bytes.extend_from_slice(&(public_inputs.len() as u64).to_be_bytes());
    for public in public_inputs {
        bytes.push(public.sub_tree_id);
        bytes.extend_from_slice(&public.leaf_index.to_be_bytes());
        bytes.push(public.tree_depth);
        bytes.extend_from_slice(&public.per_sub_tree_root);
        bytes.extend_from_slice(&public.bundle_root_blake3);
        bytes.extend_from_slice(&public.nullifier);
        bytes.extend_from_slice(&public.recipient_starstream_addr);
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

fn assert_payload_len_selector<AB: AirBuilder>(builder: &mut AB, local: &[AB::Var]) {
    let mut selector_sum = AB::Expr::ZERO;
    let mut len = AB::Expr::ZERO;
    for choice in 0..LEAF_PAYLOAD_LEN_CHOICES {
        let selector = local[COL_PAYLOAD_LEN_SELECTOR + choice];
        builder.assert_bool(selector);
        selector_sum += selector.into();
        len += selector * AB::F::from_u8(choice as u8);
    }
    builder.assert_one(selector_sum);
    builder.assert_eq(local[COL_PAYLOAD_LEN], len);

    for payload_index in 0..MAX_V01_LEAF_PAYLOAD_LEN {
        let mut len_lte_index = AB::Expr::ZERO;
        for choice in 0..=payload_index {
            len_lte_index += local[COL_PAYLOAD_LEN_SELECTOR + choice].into();
        }
        builder.assert_zero(local[COL_PAYLOAD + payload_index] * len_lte_index);
    }
}

fn assert_remaining_bits_match_leaf_index<AB: AirBuilder>(
    builder: &mut AB,
    local: &[AB::Var],
    public: &[AB::PublicVar],
) {
    for byte_index in 0..LEAF_INDEX_LEN {
        let mut packed = AB::Expr::ZERO;
        for bit in 0..8 {
            packed +=
                local[COL_REMAINING_INDEX_BITS + byte_index * 8 + bit] * AB::F::from_u8(1 << bit);
        }
        builder.assert_eq(
            packed,
            public[PUBLIC_LEAF_INDEX_OFFSET + LEAF_INDEX_LEN - 1 - byte_index],
        );
    }
}

fn expr<F: Field>(value: p3_air::symbolic::SymbolicVariable<F>) -> SymbolicExpression<F> {
    value.into()
}

fn const_expr<F: Field>(value: u8) -> SymbolicExpression<F> {
    SymbolicExpression::Leaf(BaseLeaf::Constant(F::from_u8(value)))
}

fn const_u32_byte_expr<F: Field>(value: u32, byte: usize) -> SymbolicExpression<F> {
    const_expr(((value >> (byte * 8)) & 0xFF) as u8)
}

fn byte_expr_from_bits<F: Field>(
    local: &[p3_air::symbolic::SymbolicVariable<F>],
    offset: usize,
) -> SymbolicExpression<F> {
    let mut out = const_expr(0);
    for bit in 0..8 {
        out += expr(local[offset + bit]) * F::from_u8(1 << bit);
    }
    out
}

fn hash_exprs<F: Field>(
    local: &[p3_air::symbolic::SymbolicVariable<F>],
    offset: usize,
) -> Vec<SymbolicExpression<F>> {
    (0..HASH_LEN).map(|i| expr(local[offset + i])).collect()
}

fn iv_exprs<F: Field>() -> Vec<SymbolicExpression<F>> {
    IV.iter()
        .flat_map(|word| word.to_le_bytes().map(const_expr))
        .collect()
}

fn leaf_lookup_exprs<F: Field>(
    local: &[p3_air::symbolic::SymbolicVariable<F>],
) -> Vec<SymbolicExpression<F>> {
    let mut values = Vec::with_capacity(COMPRESSION_LOOKUP_WIDTH);
    values.extend(DOMAIN_LEAF.iter().copied().map(const_expr));
    values.push(expr(local[COL_SUB_TREE_ID]));
    for offset in 0..LEAF_INDEX_LEN {
        values.push(expr(local[COL_LEAF_INDEX_BE + offset]));
    }
    values.extend(core::iter::repeat_with(|| const_expr(0)).take(7));
    values.push(expr(local[COL_PAYLOAD_LEN]));
    for offset in 0..MAX_V01_LEAF_PAYLOAD_LEN {
        values.push(expr(local[COL_PAYLOAD + offset]));
    }
    values.extend(
        core::iter::repeat_with(|| const_expr(0))
            .take(64 - DOMAIN_LEAF.len() - 1 - LEAF_INDEX_LEN - 8 - MAX_V01_LEAF_PAYLOAD_LEN),
    );
    values.extend(iv_exprs());
    values.extend(core::iter::repeat_with(|| const_expr(0)).take(COUNTER_BYTES));
    values.push(
        const_expr((DOMAIN_LEAF.len() + 1 + LEAF_INDEX_LEN + 8) as u8)
            + expr(local[COL_PAYLOAD_LEN]),
    );
    values.extend(core::iter::repeat_with(|| const_expr(0)).take(U32_BYTES - 1));
    values.extend((0..U32_BYTES).map(|byte| const_u32_byte_expr(LEAF_FLAGS, byte)));
    values.extend(hash_exprs(local, COL_PREV_NODE));
    values
}

fn node_first_lookup_exprs<F: Field>(
    local: &[p3_air::symbolic::SymbolicVariable<F>],
) -> Vec<SymbolicExpression<F>> {
    let mut values = Vec::with_capacity(COMPRESSION_LOOKUP_WIDTH);
    values.extend(DOMAIN_NODE.iter().copied().map(const_expr));
    values.extend(hash_exprs(local, COL_LEFT_NODE));
    values
        .extend((0..(64 - DOMAIN_NODE.len() - HASH_LEN)).map(|i| expr(local[COL_RIGHT_NODE + i])));
    values.extend(iv_exprs());
    values.extend(core::iter::repeat_with(|| const_expr(0)).take(COUNTER_BYTES));
    values.extend((0..U32_BYTES).map(|byte| const_u32_byte_expr(64, byte)));
    values.extend((0..U32_BYTES).map(|byte| const_u32_byte_expr(NODE_FIRST_FLAGS, byte)));
    values.extend(hash_exprs(local, COL_NODE_MID));
    values
}

fn node_second_lookup_exprs<F: Field>(
    local: &[p3_air::symbolic::SymbolicVariable<F>],
) -> Vec<SymbolicExpression<F>> {
    let mut values = Vec::with_capacity(COMPRESSION_LOOKUP_WIDTH);
    values.extend(
        ((64 - DOMAIN_NODE.len() - HASH_LEN)..HASH_LEN).map(|i| expr(local[COL_RIGHT_NODE + i])),
    );
    values.extend(
        core::iter::repeat_with(|| const_expr(0))
            .take(64 - (DOMAIN_NODE.len() + HASH_LEN * 2 - 64)),
    );
    values.extend(hash_exprs(local, COL_NODE_MID));
    values.extend(core::iter::repeat_with(|| const_expr(0)).take(COUNTER_BYTES));
    values.extend((0..U32_BYTES).map(|byte| const_u32_byte_expr(13, byte)));
    values.extend((0..U32_BYTES).map(|byte| const_u32_byte_expr(NODE_SECOND_FLAGS, byte)));
    values.extend(hash_exprs(local, COL_CURRENT_NODE));
    values
}

fn blake3_table_lookup_exprs<F: Field>(
    local: &[p3_air::symbolic::SymbolicVariable<F>],
) -> Vec<SymbolicExpression<F>> {
    let mut values = Vec::with_capacity(COMPRESSION_LOOKUP_WIDTH);
    for word in 0..16 {
        for byte in 0..4 {
            values.push(byte_expr_from_bits(
                local,
                B3_INPUTS_OFFSET + word * 32 + byte * 8,
            ));
        }
    }
    for word in 0..8 {
        for byte in 0..4 {
            values.push(byte_expr_from_bits(
                local,
                B3_CHAINING_VALUES_OFFSET + word * 32 + byte * 8,
            ));
        }
    }
    for byte in 0..4 {
        values.push(byte_expr_from_bits(local, B3_COUNTER_LOW_OFFSET + byte * 8));
    }
    for byte in 0..4 {
        values.push(byte_expr_from_bits(local, B3_COUNTER_HI_OFFSET + byte * 8));
    }
    for byte in 0..4 {
        values.push(byte_expr_from_bits(local, B3_BLOCK_LEN_OFFSET + byte * 8));
    }
    for byte in 0..4 {
        values.push(byte_expr_from_bits(local, B3_FLAGS_OFFSET + byte * 8));
    }
    for word in 0..8 {
        for byte in 0..4 {
            values.push(byte_expr_from_bits(
                local,
                B3_OUTPUTS_OFFSET + word * 32 + byte * 8,
            ));
        }
    }
    values
}

struct BuiltTraces {
    membership_traces: Vec<RowMajorMatrix<Val>>,
    blake3_trace: RowMajorMatrix<Val>,
}

#[derive(Debug, Clone)]
struct CheckedWitness {
    public: ClaimPublicInputs,
    leaf_payload: Vec<u8>,
    merkle_path: Vec<Hash>,
    leaf_hash: Hash,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_tuples_have_the_same_width() {
        let leaf = leaf_compression(1, 0, b"");
        assert_eq!(
            blake3_trace::compression_lookup_values(&leaf).len(),
            COMPRESSION_LOOKUP_WIDTH
        );
    }
}
