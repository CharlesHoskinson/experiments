use omega_claim_prover::{
    prove_collection, MembershipWitness, OmegaCommitment, ProverConfig, ProverError,
};
use omega_claim_tx::ClaimPublicInputs;
use omega_commitment_core::{
    hash::{blake3_256, Hash},
    tree::{leaf_hash_v2, MerkleTree},
    witness::InclusionWitness,
    SUB_TREE_ID_UTXO,
};

// AIR-layer trace mutation coverage lives in soundness_negative.rs. These
// smoke tests cover honest proof generation and prover-side witness rejection.

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

fn witness_at(index: usize) -> (OmegaCommitment, MembershipWitness) {
    let payloads = payloads(256);
    let tree = MerkleTree::build_v1(SUB_TREE_ID_UTXO, payloads.clone()).unwrap();
    let payload = payloads[index].clone();
    let leaf = leaf_hash_v2(SUB_TREE_ID_UTXO, index as u64, &payload);
    let inclusion = InclusionWitness::build_at_index(&tree, index as u32).unwrap();
    assert_eq!(inclusion.leaf, leaf);

    let commitment = commitment_for(tree.root(), payloads.len(), tree.leaf_count(), tree.depth());
    let public = ClaimPublicInputs {
        sub_tree_id: SUB_TREE_ID_UTXO,
        leaf_index: index as u64,
        tree_depth: tree.depth() as u8,
        per_sub_tree_root: tree.root(),
        bundle_root_blake3: commitment.bundle_root_blake3,
        nullifier: hash(0xA1),
        recipient_starstream_addr: hash(0xB2),
    };
    let witness = MembershipWitness::from_inclusion(public, payload, inclusion);
    (commitment, witness)
}

#[test]
fn prove_collection_produces_non_empty_plonky3_proof_for_256_leaf_tree() {
    let (commitment, witness) = witness_at(42);

    let proof = prove_collection(&commitment, &[witness], &ProverConfig::default()).expect("proof");

    assert!(proof.0.len() > 128);
}

#[test]
fn prove_collection_rejects_wrong_path_before_emitting_proof() {
    let (commitment, mut witness) = witness_at(42);
    witness.merkle_path[0][0] ^= 0x01;

    let err = prove_collection(&commitment, &[witness], &ProverConfig::default()).unwrap_err();

    assert!(matches!(
        err,
        ProverError::PathMismatch { witness_index: 0 }
    ));
}

#[test]
fn prove_collection_rejects_payloads_past_v01_leaf_bound() {
    let (commitment, mut witness) = witness_at(42);
    witness.leaf_payload = vec![0xAA; 35];

    let err = prove_collection(&commitment, &[witness], &ProverConfig::default()).unwrap_err();

    assert!(matches!(
        err,
        ProverError::LeafTooLargeForV01 {
            actual: 65,
            limit: 64,
            witness_index: 0,
        }
    ));
}
