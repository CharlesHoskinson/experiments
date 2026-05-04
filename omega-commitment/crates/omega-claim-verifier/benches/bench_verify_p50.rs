#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use omega_claim_prover::{prove_collection, MembershipWitness, OmegaCommitment, ProverConfig};
use omega_claim_tx::{ClaimPublicInputs, ProofBytes};
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

fn proof_fixture(count: usize) -> (OmegaCommitment, Vec<ClaimPublicInputs>, ProofBytes) {
    let payloads = payloads(256);
    let tree = MerkleTree::build_v1(SUB_TREE_ID_UTXO, payloads.clone()).unwrap();
    let commitment = commitment_for(tree.root(), payloads.len(), tree.leaf_count(), tree.depth());
    let mut public_inputs = Vec::with_capacity(count);
    let mut witnesses = Vec::with_capacity(count);

    for (index, payload) in payloads.iter().enumerate().take(count) {
        let payload = payload.clone();
        let inclusion = InclusionWitness::build_at_index(&tree, index as u32).unwrap();
        assert_eq!(
            inclusion.leaf,
            leaf_hash_v2(SUB_TREE_ID_UTXO, index as u64, &payload)
        );
        let public = ClaimPublicInputs {
            sub_tree_id: SUB_TREE_ID_UTXO,
            leaf_index: index as u64,
            tree_depth: tree.depth() as u8,
            per_sub_tree_root: tree.root(),
            bundle_root_blake3: commitment.bundle_root_blake3,
            nullifier: hash(0xA1),
            recipient_starstream_addr: hash(0xB2),
        };
        public_inputs.push(public.clone());
        witnesses.push(MembershipWitness::from_inclusion(
            public, payload, inclusion,
        ));
    }

    let proof = prove_collection(&commitment, &witnesses, &ProverConfig::default()).unwrap();
    (commitment, public_inputs, proof)
}

fn bench_verify(c: &mut Criterion) {
    let mut group = c.benchmark_group("omega_claim_verifier");
    for count in [1usize, 16, 256] {
        let (commitment, public_inputs, proof) = proof_fixture(count);
        group.bench_function(format!("verify_{count}_leaves"), |b| {
            b.iter(|| {
                verify(
                    black_box(&commitment),
                    black_box(&public_inputs),
                    black_box(&proof),
                )
                .expect("verify")
            });
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(100);
    targets = bench_verify
}
criterion_main!(benches);
