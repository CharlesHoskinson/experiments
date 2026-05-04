use omega_claim_prover::{
    prove_collection, prove_collection_with_trace_tamper, MembershipWitness, OmegaCommitment,
    ProverConfig, TraceTamper,
};
use omega_claim_tx::ClaimPublicInputs;
use omega_claim_verifier::verify;
use omega_commitment_core::{
    hash::{blake3_256, Hash},
    tree::{leaf_hash_v2, MerkleTree},
    witness::InclusionWitness,
    SUB_TREE_ID_UTXO,
};

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

fn fixture_at(index: usize) -> (OmegaCommitment, MembershipWitness) {
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
fn verifier_rejects_air_column_mutations_after_proving() {
    let (commitment, witness) = fixture_at(42);
    let public_inputs = vec![witness.public.clone()];
    let config = ProverConfig::default();

    let honest = prove_collection(&commitment, std::slice::from_ref(&witness), &config)
        .expect("honest proof");
    verify(&commitment, &public_inputs, &honest).expect("honest proof verifies");

    for tamper in [
        TraceTamper::PayloadByte,
        TraceTamper::SiblingByte,
        TraceTamper::CurrentNodeByte,
        TraceTamper::LeafIndexByte,
    ] {
        let proof = prove_collection_with_trace_tamper(
            &commitment,
            std::slice::from_ref(&witness),
            &config,
            tamper,
        )
        .expect("tampered trace still produces a proof object");

        assert!(
            verify(&commitment, &public_inputs, &proof).is_err(),
            "verifier accepted proof with {tamper:?} mutation"
        );
    }
}
