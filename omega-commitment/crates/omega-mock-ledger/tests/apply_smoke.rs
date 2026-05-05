#![allow(missing_docs)]

use std::path::PathBuf;
use std::time::Duration;

use omega_claim_prover::{prove_collection, MembershipWitness, OmegaCommitment, ProverConfig};
use omega_claim_tx::{ClaimPublicInputs, ClaimTx, ClaimUtxo, ClaimWitness};
use omega_commitment_core::{
    hash::{blake3_256, Hash},
    tree::{leaf_hash_v2, MerkleTree},
    witness::InclusionWitness,
    SUB_TREE_ID_UTXO,
};
use omega_mock_ledger::{LedgerError, MockLedger};

fn temp_db_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-mock-ledger-{name}-{}-{}.sqlite",
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_claim_inserts_nullifier_and_utxo_then_rejects_replay() {
    let path = temp_db_path("apply");
    let _ = std::fs::remove_file(&path);
    let ledger = MockLedger::open(&path).expect("open ledger");
    let (commitment, claim, public) = accepted_claim();

    ledger
        .apply_claim(7, &commitment, claim.clone())
        .await
        .expect("first apply");

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

    ledger
        .checkpoint_wal_truncate()
        .await
        .expect("manual checkpoint");
    let snapshot_path = ledger
        .snapshot("apply-smoke".to_string())
        .await
        .expect("snapshot");
    assert!(snapshot_path.exists());
    let snapshot = rusqlite::Connection::open(&snapshot_path).expect("open snapshot");
    let snapshot_nullifiers = snapshot
        .query_row("SELECT COUNT(*) FROM nullifiers", [], |row| {
            row.get::<_, i64>(0)
        })
        .expect("snapshot nullifier count");
    assert_eq!(snapshot_nullifiers, 1);
    drop(snapshot);

    ledger
        .insert_synthetic_claim_for_test(public.sub_tree_id, 99, 7)
        .await
        .expect("synthetic post-snapshot write");
    assert_eq!(ledger.nullifier_count().await.expect("live count"), 2);
    ledger
        .restore_snapshot(snapshot_path)
        .await
        .expect("restore snapshot");
    assert_eq!(ledger.nullifier_count().await.expect("restored count"), 1);

    let checkpoint_task = ledger.spawn_wal_truncate_task_with_interval(Duration::from_millis(10));
    tokio::time::sleep(Duration::from_millis(25)).await;
    checkpoint_task.abort();

    let err = ledger.apply_claim(8, &commitment, claim).await.unwrap_err();
    assert!(matches!(err, LedgerError::Replay { .. }));
}
