//! Test fixtures for downstream integration tests.

use omega_claim_tx::{ClaimTx, ClaimUtxo, ClaimWitness};
use omega_commitment_core::{
    hash::{blake3_256, Hash},
    tree::{leaf_hash_v2, MerkleTree},
    witness::InclusionWitness,
    SUB_TREE_ID_UTXO,
};

use crate::{prove_collection, MembershipWitness, OmegaCommitment, ProverConfig};

/// Builds an accepted synthetic UTxO claim for a 256-leaf test tree.
///
/// # Panics
///
/// Panics when `leaf_index` is outside the synthetic 256-leaf tree or when
/// proof generation fails.
pub fn build_synthetic_accepted_claim(leaf_index: u64) -> omega_claim_tx::ClaimTx {
    let (commitment, witness) = witness_at(leaf_index);
    let proof = prove_collection(
        &commitment,
        std::slice::from_ref(&witness),
        &ProverConfig::default(),
    )
    .expect("synthetic fixture proof");
    ClaimTx::Utxo(ClaimUtxo {
        public: witness.public,
        witness: ClaimWitness {
            leaf_payload: witness.leaf_payload,
            merkle_path: witness.merkle_path,
            signing_key_proof: vec![0xED],
        },
        proof,
    })
}

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

fn witness_at(index: u64) -> (OmegaCommitment, MembershipWitness) {
    let index = usize::try_from(index).expect("leaf index fits usize");
    assert!(index < 256, "leaf index outside synthetic fixture");

    let payloads = payloads(256);
    let tree = MerkleTree::build_v1(SUB_TREE_ID_UTXO, payloads.clone()).unwrap();
    let payload = payloads[index].clone();
    let leaf = leaf_hash_v2(SUB_TREE_ID_UTXO, index as u64, &payload);
    let inclusion = InclusionWitness::build_at_index(&tree, index as u32).unwrap();
    assert_eq!(inclusion.leaf, leaf);

    let commitment = commitment_for(tree.root(), payloads.len(), tree.leaf_count(), tree.depth());
    let public = omega_claim_tx::ClaimPublicInputs {
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

#[cfg(test)]
mod tests {
    use omega_claim_tx::ClaimTx;

    use super::build_synthetic_accepted_claim;

    #[test]
    fn builds_accepted_claim_for_leaf_42() {
        let claim = build_synthetic_accepted_claim(42);

        let ClaimTx::Utxo(claim) = claim else {
            panic!("fixture must build a single UTxO claim");
        };
        assert_eq!(claim.public.leaf_index, 42);
        assert!(claim.proof.0.len() > 128);
    }
}
