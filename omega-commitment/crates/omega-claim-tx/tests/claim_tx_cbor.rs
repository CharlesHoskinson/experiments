use omega_claim_tx::{
    ClaimCollection, ClaimPublicInputs, ClaimTx, ClaimUtxo, ClaimWitness, ProofBytes,
};
use proptest::prelude::*;

fn public_inputs(leaf_index: u64) -> ClaimPublicInputs {
    ClaimPublicInputs {
        sub_tree_id: omega_commitment_core::SUB_TREE_ID_UTXO,
        leaf_index,
        bundle_root_blake3: [0x11; 32],
        nullifier: [leaf_index as u8; 32],
        recipient_starstream_addr: [0x22; 32],
    }
}

fn witness(seed: u8) -> ClaimWitness {
    ClaimWitness {
        leaf_payload: vec![seed; 32],
        merkle_path: vec![[0xA5; 32], [0x5A; 32]],
        signing_key_proof: vec![seed ^ 0xFF; 64],
    }
}

fn proof(seed: u8) -> ProofBytes {
    ProofBytes(vec![seed; 128])
}

fn sample_utxo_claim() -> ClaimTx {
    ClaimTx::Utxo(ClaimUtxo {
        public: public_inputs(42),
        witness: witness(7),
        proof: proof(9),
    })
}

fn sample_collection_claim(count: u64) -> ClaimTx {
    let public = (0..count).map(public_inputs).collect();
    let witness = (0..count).map(|i| witness(i as u8)).collect();

    ClaimTx::Collection(ClaimCollection {
        public,
        witness,
        proof: proof(3),
    })
}

#[test]
fn single_leaf_claim_round_trips_through_cbor() {
    let claim = sample_utxo_claim();

    let encoded = claim.to_cbor().expect("sample claim encodes");
    let decoded = ClaimTx::from_cbor(&encoded).expect("sample claim decodes");

    assert_eq!(decoded, claim);
}

#[test]
fn batched_leaf_claim_round_trips_through_cbor() {
    let claim = sample_collection_claim(8);

    let encoded = claim.to_cbor().expect("sample collection encodes");
    let decoded = ClaimTx::from_cbor(&encoded).expect("sample collection decodes");

    assert_eq!(decoded, claim);
}

#[test]
fn tampered_cbor_is_rejected() {
    let claim = sample_utxo_claim();
    let mut encoded = claim.to_cbor().expect("sample claim encodes");

    let payload_byte = encoded
        .iter()
        .position(|byte| *byte == 0x2A)
        .expect("leaf index marker is present in sample payload");
    encoded[payload_byte] ^= 0x01;

    assert!(matches!(
        ClaimTx::from_cbor(&encoded),
        Err(omega_claim_tx::CborError::ChecksumMismatch)
    ));
}

#[test]
fn cbor_size_within_bounds_for_1024_leaf_collection() {
    let claim = sample_collection_claim(1024);

    let encoded = claim.to_cbor().expect("1024-leaf collection encodes");

    assert!(
        encoded.len() < 32 * 1024 * 1024,
        "1024-leaf collection CBOR must stay below 32 MiB, got {} bytes",
        encoded.len()
    );
}

#[test]
fn collection_arity_mismatch_is_rejected_before_encode() {
    let claim = ClaimTx::Collection(ClaimCollection {
        public: vec![public_inputs(0), public_inputs(1)],
        witness: vec![witness(0)],
        proof: proof(0),
    });

    assert!(matches!(
        claim.to_cbor(),
        Err(omega_claim_tx::CborError::CollectionArityMismatch {
            public: 2,
            witness: 1
        })
    ));
}

prop_compose! {
    fn arb_public_inputs()(
        sub_tree_id in 1u8..=7,
        leaf_index in any::<u64>(),
        bundle_root_blake3 in any::<[u8; 32]>(),
        nullifier in any::<[u8; 32]>(),
        recipient_starstream_addr in any::<[u8; 32]>(),
    ) -> ClaimPublicInputs {
        ClaimPublicInputs {
            sub_tree_id,
            leaf_index,
            bundle_root_blake3,
            nullifier,
            recipient_starstream_addr,
        }
    }
}

prop_compose! {
    fn arb_witness()(
        leaf_payload in prop::collection::vec(any::<u8>(), 0..=64),
        merkle_path in prop::collection::vec(any::<[u8; 32]>(), 0..=32),
        signing_key_proof in prop::collection::vec(any::<u8>(), 0..=128),
    ) -> ClaimWitness {
        ClaimWitness {
            leaf_payload,
            merkle_path,
            signing_key_proof,
        }
    }
}

prop_compose! {
    fn arb_proof()(
        bytes in prop::collection::vec(any::<u8>(), 0..=256),
    ) -> ProofBytes {
        ProofBytes(bytes)
    }
}

prop_compose! {
    fn arb_utxo_claim()(
        public in arb_public_inputs(),
        witness in arb_witness(),
        proof in arb_proof(),
    ) -> ClaimTx {
        ClaimTx::Utxo(ClaimUtxo { public, witness, proof })
    }
}

prop_compose! {
    fn arb_collection_claim()(
        entries in prop::collection::vec((arb_public_inputs(), arb_witness()), 2..=16),
        proof in arb_proof(),
    ) -> ClaimTx {
        let (public, witness): (Vec<_>, Vec<_>) = entries.into_iter().unzip();
        ClaimTx::Collection(ClaimCollection { public, witness, proof })
    }
}

proptest! {
    #[test]
    fn utxo_claim_cbor_round_trip_property(claim in arb_utxo_claim()) {
        let encoded = claim.to_cbor().expect("generated Utxo claim encodes");
        let decoded = ClaimTx::from_cbor(&encoded).expect("generated Utxo claim decodes");
        prop_assert_eq!(decoded, claim);
    }

    #[test]
    fn collection_claim_cbor_round_trip_property(claim in arb_collection_claim()) {
        let encoded = claim.to_cbor().expect("generated collection claim encodes");
        let decoded = ClaimTx::from_cbor(&encoded).expect("generated collection claim decodes");
        prop_assert_eq!(decoded, claim);
    }
}
