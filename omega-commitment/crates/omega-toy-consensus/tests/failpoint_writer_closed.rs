//! Failpoint test for writer closure during submit.

mod common;

use std::time::Duration;

use jsonrpsee::core::{client::ClientT, ClientError};

const FAILPOINT: &str = "omega_mock_ledger::writer_close";

async fn state_from(client: &jsonrpsee::http_client::HttpClient) -> omega_toy_consensus::NodeState {
    client
        .request(
            "omega_getState",
            jsonrpsee::core::params::ArrayParams::new(),
        )
        .await
        .unwrap()
}

async fn state(node_id: u64) -> omega_toy_consensus::NodeState {
    let url = format!("http://127.0.0.1:800{node_id}");
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(url)
        .unwrap();
    state_from(&client).await
}

async fn wait_all_applied_at_least(target: u64) -> Vec<omega_toy_consensus::NodeState> {
    let mut observed = Vec::new();
    for _ in 0..120 {
        observed.clear();
        let mut all_applied = true;
        for node_id in [1, 2, 3] {
            let state = state(node_id).await;
            all_applied &= state.applied_index >= target;
            observed.push(state);
        }
        if all_applied {
            return observed;
        }
        tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(250)))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    panic!("not all nodes applied at least index {target}: {observed:?}");
}

#[test]
fn writer_closed_returns_neg_32004_no_state_advance() -> turmoil::Result {
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(50);
    let mut sim = common::three_node_sim();

    sim.client("client", async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let leader_url = common::leader_url().await;
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .build(&leader_url)
            .unwrap();
        let before = state_from(&client).await;
        let failed_index = before.applied_index + 1;

        fail::cfg(FAILPOINT, "return").unwrap();

        let mut params = jsonrpsee::core::params::ObjectParams::new();
        params.insert("claim", claim).unwrap();
        let result: Result<omega_toy_consensus::SubmitOutcome, ClientError> =
            client.request("omega_submitClaim", params).await;
        match result {
            Err(ClientError::Call(obj)) => {
                assert_eq!(obj.code(), -32004, "expected WriterClosed");
            }
            other => panic!("expected WriterClosed, got {other:?}"),
        }

        let observed = wait_all_applied_at_least(failed_index).await;
        fail::remove(FAILPOINT);
        tokio::time::sleep(Duration::from_secs(1)).await;

        let after = state_from(&client).await;
        assert!(after.applied_index <= failed_index);
        assert_eq!(after.nullifier_count, before.nullifier_count);
        assert_eq!(after.starstream_utxo_count, before.starstream_utxo_count);
        for state in observed {
            assert_eq!(state.nullifier_count, before.nullifier_count);
            assert_eq!(state.starstream_utxo_count, before.starstream_utxo_count);
        }
        Ok(())
    });

    let result = sim.run();
    fail::remove(FAILPOINT);
    result
}
