//! Turmoil test for follower NotLeader hints.

mod common;

use std::time::Duration;

use jsonrpsee::core::{client::ClientT, ClientError};

#[test]
fn submit_to_follower_returns_neg_32000_with_url() -> turmoil::Result {
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(7);
    let mut sim = common::three_node_sim();

    sim.client("client", async move {
        tokio::time::sleep(Duration::from_secs(3)).await;

        let mut leader_url = None;
        let mut follower_url = None;
        for node in ["node1", "node2", "node3"] {
            let url = format!("http://127.0.0.1:800{}", &node[4..]);
            let client = jsonrpsee::http_client::HttpClientBuilder::default()
                .request_timeout(Duration::from_secs(300))
                .build(&url)
                .unwrap();
            let state: omega_toy_consensus::NodeState = client
                .request(
                    "omega_getState",
                    jsonrpsee::core::params::ArrayParams::new(),
                )
                .await
                .unwrap();
            if matches!(state.role, omega_toy_consensus::NodeRole::Leader) {
                leader_url = Some(url);
            } else if follower_url.is_none() {
                follower_url = Some(url);
            }
        }
        let leader_url = leader_url.expect("a leader exists after 3s");
        let follower_url = follower_url.expect("a follower exists after 3s");

        let follower = jsonrpsee::http_client::HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(300))
            .build(&follower_url)
            .unwrap();
        let mut params = jsonrpsee::core::params::ObjectParams::new();
        params.insert("claim", claim.clone()).unwrap();
        let result: Result<omega_toy_consensus::SubmitOutcome, ClientError> =
            follower.request("omega_submitClaim", params).await;

        let hint_url = match result {
            Err(ClientError::Call(obj)) if obj.code() == -32000 => {
                let data = obj.data().expect("hint present");
                let value: serde_json::Value = serde_json::from_str(data.get()).unwrap();
                value["leader_rpc_url"]
                    .as_str()
                    .expect("leader_rpc_url string")
                    .to_string()
            }
            other => panic!("unexpected follower response: {other:?}"),
        };
        assert_eq!(hint_url, leader_url);

        let leader = jsonrpsee::http_client::HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(300))
            .build(&hint_url)
            .unwrap();
        let mut params = jsonrpsee::core::params::ObjectParams::new();
        params.insert("claim", claim).unwrap();
        let outcome: omega_toy_consensus::SubmitOutcome =
            leader.request("omega_submitClaim", params).await.unwrap();
        assert!(outcome.accepted);
        assert!(outcome.applied_index.is_some());

        Ok(())
    });
    sim.run()
}
