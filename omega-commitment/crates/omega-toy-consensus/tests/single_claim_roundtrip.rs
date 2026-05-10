//! Turmoil test for a single accepted claim round trip.

mod common;

use std::time::Duration;

use jsonrpsee::core::client::ClientT;

#[test]
fn single_claim_roundtrip() -> turmoil::Result {
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(42);
    let mut sim = common::three_node_sim();

    sim.client("client", async move {
        let leader_url = common::leader_url().await;

        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(300))
            .build(&leader_url)
            .unwrap();
        let mut params = jsonrpsee::core::params::ObjectParams::new();
        params.insert("claim", claim).unwrap();
        let outcome: omega_toy_consensus::SubmitOutcome =
            client.request("omega_submitClaim", params).await.unwrap();
        assert!(outcome.accepted);
        let applied_index = outcome.applied_index.expect("applied_index when accepted");

        let mut observed = Vec::new();
        for _ in 0..120 {
            observed.clear();
            let mut all_applied = true;
            for node in ["node1", "node2", "node3"] {
                let url = format!("http://127.0.0.1:800{}", &node[4..]);
                let client = jsonrpsee::http_client::HttpClientBuilder::default()
                    .build(url)
                    .unwrap();
                let state: omega_toy_consensus::NodeState = client
                    .request(
                        "omega_getState",
                        jsonrpsee::core::params::ArrayParams::new(),
                    )
                    .await
                    .unwrap();
                all_applied &= state.applied_index >= applied_index
                    && state.nullifier_count >= 1
                    && state.starstream_utxo_count >= 1;
                observed.push((node, state));
            }
            if all_applied {
                return Ok(());
            }
            tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(250)))
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        panic!("not all nodes applied index {applied_index}: {observed:?}");
    });
    sim.run()
}
