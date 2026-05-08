//! Turmoil test for initial leader election.

mod common;

use std::time::Duration;

use jsonrpsee::core::client::ClientT;

#[test]
fn single_leader_emerges() -> turmoil::Result {
    let mut sim = common::three_node_sim();
    sim.client("client", async move {
        tokio::time::sleep(Duration::from_secs(3)).await;

        let mut leaders = 0;
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
            if matches!(state.role, omega_toy_consensus::NodeRole::Leader) {
                leaders += 1;
            }
        }
        assert_eq!(leaders, 1, "exactly one leader after 3s");
        Ok(())
    });
    sim.run()
}
