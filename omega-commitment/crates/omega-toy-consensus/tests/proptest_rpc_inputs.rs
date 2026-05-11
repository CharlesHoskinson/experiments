//! Proptest samples for malformed `omega_submitClaim` inputs.

mod common;

use jsonrpsee::core::{client::ClientT, ClientError};
use omega_claim_tx::{ClaimTx, ProofBytes};
use proptest::prelude::*;
use proptest::strategy::ValueTree;

prop_compose! {
    fn arb_payload_bytes(max: usize)(bytes in prop::collection::vec(any::<u8>(), 0..max)) -> Vec<u8> {
        bytes
    }
}

fn with_proof_bytes(mut claim: ClaimTx, payload: Vec<u8>) -> ClaimTx {
    match &mut claim {
        ClaimTx::Utxo(utxo) => {
            utxo.proof = ProofBytes(payload);
        }
        ClaimTx::Collection(collection) => {
            collection.proof = ProofBytes(payload);
        }
    }
    claim
}

fn documented_code(code: i32) -> bool {
    // -32603 is reachable via openraft `Fatal` and the membership-change
    // collapse path in `routing::translate_client_write_error`; include it
    // in the allowlist so a randomly-malformed input that reaches that
    // path does not flake the proptest. See spec § "Error code map".
    matches!(
        code,
        -32600 | -32602 | -32603 | -32001 | -32002 | -32003 | -32004 | -32005
    )
}

#[test]
fn rpc_input_fuzz_never_panics() -> turmoil::Result {
    let base_claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(1);
    let mut sim = common::three_node_sim();

    sim.client("client", async move {
        let leader_url = common::leader_url().await;
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .build(&leader_url)
            .unwrap();

        let mut runner = proptest::test_runner::TestRunner::default();
        for _ in 0..32 {
            let payload = arb_payload_bytes(2048)
                .new_tree(&mut runner)
                .unwrap()
                .current();
            let bad_claim = with_proof_bytes(base_claim.clone(), payload.clone());
            let mut params = jsonrpsee::core::params::ObjectParams::new();
            params.insert("claim", bad_claim).unwrap();

            let result: Result<omega_toy_consensus::SubmitOutcome, ClientError> =
                client.request("omega_submitClaim", params).await;
            match result {
                Ok(outcome) => {
                    if !outcome.accepted {
                        let reason = outcome.reject_reason.unwrap_or_default();
                        assert!(
                            ["verify", "invalid", "replay", "internal"].contains(&reason.as_str())
                        );
                    }
                }
                Err(ClientError::Call(obj)) => {
                    assert!(
                        documented_code(obj.code()),
                        "undocumented JSON-RPC code {} for input {:?}",
                        obj.code(),
                        payload
                    );
                }
                Err(other) => panic!("transport error: {other:?}"),
            }
        }
        Ok(())
    });

    sim.run()
}
