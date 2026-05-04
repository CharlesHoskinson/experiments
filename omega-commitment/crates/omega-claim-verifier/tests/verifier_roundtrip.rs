use std::sync::OnceLock;

use omega_claim_prover::{
    prove_collection, MembershipWitness, OmegaCommitment, ProofEnvelope, ProverConfig,
};
use omega_claim_tx::{ClaimPublicInputs, ProofBytes};
use omega_claim_verifier::{verify, VerifyError};
use omega_commitment_core::{
    hash::{blake3_256, Hash},
    tree::{leaf_hash_v2, MerkleTree},
    witness::InclusionWitness,
    SUB_TREE_ID_UTXO,
};

#[derive(Clone)]
struct Fixture {
    commitment: OmegaCommitment,
    public_inputs: Vec<ClaimPublicInputs>,
    proof: ProofBytes,
}

static FIXTURE: OnceLock<Fixture> = OnceLock::new();

fn hash(byte: u8) -> Hash {
    [byte; 32]
}

fn payloads(count: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|i| {
            let mut payload = Vec::with_capacity(16);
            payload.extend_from_slice(&(i as u64).to_be_bytes());
            payload.extend_from_slice(&(i as u64 + 10_000).to_be_bytes());
            payload
        })
        .collect()
}

fn commitment_for(
    root: Hash,
    item_count: usize,
    leaf_count: usize,
    tree_depth: usize,
) -> OmegaCommitment {
    let mut sub_tree_roots = [[0u8; 32]; 7];
    sub_tree_roots[(SUB_TREE_ID_UTXO - 1) as usize] = root;
    let mut bundle_preimage = Vec::with_capacity(7 * 32);
    for root in sub_tree_roots {
        bundle_preimage.extend_from_slice(&root);
    }
    OmegaCommitment {
        bundle_root_blake3: blake3_256(&bundle_preimage),
        sub_tree_roots_blake3: sub_tree_roots,
        item_counts: [item_count as u64, 0, 0, 0, 0, 0, 0],
        leaf_counts: [leaf_count as u64, 0, 0, 0, 0, 0, 0],
        tree_depths: [tree_depth as u32, 0, 0, 0, 0, 0, 0],
    }
}

fn build_fixture() -> Fixture {
    let payloads = payloads(16);
    let tree = MerkleTree::build_v1(SUB_TREE_ID_UTXO, payloads.clone()).unwrap();
    let commitment = commitment_for(tree.root(), payloads.len(), tree.leaf_count(), tree.depth());
    let index = 5usize;
    let payload = payloads[index].clone();
    let leaf = leaf_hash_v2(SUB_TREE_ID_UTXO, index as u64, &payload);
    let inclusion = InclusionWitness::build_at_index(&tree, index as u32).unwrap();
    assert_eq!(inclusion.leaf, leaf);

    let public = ClaimPublicInputs {
        sub_tree_id: SUB_TREE_ID_UTXO,
        leaf_index: index as u64,
        tree_depth: tree.depth() as u8,
        per_sub_tree_root: tree.root(),
        bundle_root_blake3: commitment.bundle_root_blake3,
        nullifier: hash(0xA1),
        recipient_starstream_addr: hash(0xB2),
    };
    let witness = MembershipWitness::from_inclusion(public.clone(), payload, inclusion);
    let proof = prove_collection(&commitment, &[witness], &ProverConfig::default()).unwrap();

    Fixture {
        commitment,
        public_inputs: vec![public],
        proof,
    }
}

fn fixture() -> Fixture {
    FIXTURE.get_or_init(build_fixture).clone()
}

#[test]
fn verifier_accepts_prover_output_for_same_commitment_and_inputs() {
    let fixture = fixture();

    verify(&fixture.commitment, &fixture.public_inputs, &fixture.proof).expect("valid proof");
}

#[test]
fn verifier_rejects_tampered_proof_bytes() {
    let mut fixture = fixture();
    let midpoint = fixture.proof.0.len() / 2;
    fixture.proof.0[midpoint] ^= 0x01;

    let err = verify(&fixture.commitment, &fixture.public_inputs, &fixture.proof).unwrap_err();

    assert_eq!(err, VerifyError::InvalidProof);
}

#[test]
fn verifier_rejects_wrong_commitment_before_accepting_proof() {
    let mut fixture = fixture();
    fixture.commitment.bundle_root_blake3[0] ^= 0x01;

    let err = verify(&fixture.commitment, &fixture.public_inputs, &fixture.proof).unwrap_err();

    assert_eq!(err, VerifyError::CommitmentMismatch);
}

#[test]
fn verifier_rejects_public_inputs_that_do_not_match_the_envelope() {
    let mut fixture = fixture();
    fixture.public_inputs[0].nullifier[0] ^= 0x01;

    let err = verify(&fixture.commitment, &fixture.public_inputs, &fixture.proof).unwrap_err();

    assert_eq!(err, VerifyError::PublicInputMismatch);
}

#[test]
fn verifier_rejects_envelope_public_inputs_rewritten_with_matching_call_args() {
    let fixture = fixture();
    let mut envelope: ProofEnvelope = postcard::from_bytes(&fixture.proof.0).unwrap();
    envelope.public_inputs[0].recipient_starstream_addr[0] ^= 0x01;
    let proof = ProofBytes(postcard::to_allocvec(&envelope).unwrap());

    let err = verify(&fixture.commitment, &envelope.public_inputs, &proof).unwrap_err();

    assert_eq!(err, VerifyError::InvalidProof);
}

#[test]
fn verifier_reports_wrong_public_sub_tree_root() {
    let fixture = fixture();
    let mut envelope: ProofEnvelope = postcard::from_bytes(&fixture.proof.0).unwrap();
    envelope.public_inputs[0].per_sub_tree_root[0] ^= 0x01;
    let proof = ProofBytes(postcard::to_allocvec(&envelope).unwrap());

    let err = verify(&fixture.commitment, &envelope.public_inputs, &proof).unwrap_err();

    assert_eq!(err, VerifyError::WrongSubTreeRoot { index: 0 });
}

#[test]
fn verifier_reports_public_tree_depth_mismatch() {
    let fixture = fixture();
    let mut envelope: ProofEnvelope = postcard::from_bytes(&fixture.proof.0).unwrap();
    envelope.public_inputs[0].tree_depth += 1;
    let proof = ProofBytes(postcard::to_allocvec(&envelope).unwrap());

    let err = verify(&fixture.commitment, &envelope.public_inputs, &proof).unwrap_err();

    assert_eq!(
        err,
        VerifyError::DepthMismatch {
            index: 0,
            expected: fixture.public_inputs[0].tree_depth,
            actual: envelope.public_inputs[0].tree_depth
        }
    );
}
