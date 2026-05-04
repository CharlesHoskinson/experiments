#![allow(missing_docs)]

use std::path::PathBuf;

use omega_claim_prover::{prove_collection, MembershipWitness, OmegaCommitment, ProverConfig};
use omega_claim_tx::{ClaimPublicInputs, ClaimTx, ClaimUtxo, ClaimWitness};
use omega_commitment_core::{
    hash::{blake3_256, Hash},
    tree::{leaf_hash_v2, MerkleTree},
    witness::InclusionWitness,
    SUB_TREE_ID_UTXO,
};
use omega_mock_ledger::MockLedger;

fn temp_db_path(name: &str, node: u8) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-mock-ledger-{name}-node{node}-{}-{}.sqlite",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}

fn hash(byte: u8) -> Hash {
    [byte; 32]
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

fn accepted_claim() -> (OmegaCommitment, ClaimTx, ClaimPublicInputs) {
    let payloads = (0..16)
        .map(|i| {
            let mut payload = Vec::with_capacity(16);
            payload.extend_from_slice(&(i as u64).to_be_bytes());
            payload.extend_from_slice(&(i as u64 + 10_000).to_be_bytes());
            payload
        })
        .collect::<Vec<_>>();
    let tree = MerkleTree::build_v1(SUB_TREE_ID_UTXO, payloads.clone()).unwrap();
    let commitment = commitment_for(tree.root(), payloads.len(), tree.leaf_count(), tree.depth());
    let index = 3usize;
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
    let membership_witness =
        MembershipWitness::from_inclusion(public.clone(), payload.clone(), inclusion.clone());
    let proof =
        prove_collection(&commitment, &[membership_witness], &ProverConfig::default()).unwrap();
    let claim = ClaimTx::Utxo(ClaimUtxo {
        public: public.clone(),
        witness: ClaimWitness {
            leaf_payload: payload,
            merkle_path: inclusion.siblings,
            signing_key_proof: vec![0xED],
        },
        proof,
    });

    (commitment, claim, public)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restart_preserves_nullifiers_and_starstream_utxos() {
    let paths = [
        temp_db_path("restart", 1),
        temp_db_path("restart", 2),
        temp_db_path("restart", 3),
    ];
    for path in &paths {
        let _ = std::fs::remove_file(path);
    }
    let (commitment, claim, public) = accepted_claim();

    for path in &paths {
        let ledger = MockLedger::open(path).expect("open ledger");
        ledger
            .apply_claim(7, &commitment, claim.clone())
            .await
            .expect("apply claim");
    }

    for path in &paths {
        let ledger = MockLedger::open(path).expect("reopen ledger");
        assert!(ledger
            .nullifier_exists(public.sub_tree_id, public.leaf_index)
            .await
            .expect("nullifier query"));
        assert_eq!(
            ledger
                .starstream_utxo_count()
                .await
                .expect("starstream count"),
            1
        );
    }
}
