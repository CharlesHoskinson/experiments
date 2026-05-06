//! Failpoint test for a dropped AppendEntries RPC.

mod common;

use std::time::Duration;

use jsonrpsee::core::client::ClientT;
use omega_claim_tx::ClaimTx;

const FAILPOINT: &str = "omega_network::send_appendentries";

fn node_url(node_id: u64) -> String {
    format!("http://127.0.0.1:800{node_id}")
}

async fn state(node_id: u64) -> omega_toy_consensus::NodeState {
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(node_url(node_id))
        .unwrap();
    client
        .request(
            "omega_getState",
            jsonrpsee::core::params::ArrayParams::new(),
        )
        .await
        .unwrap()
}

async fn leader() -> u64 {
    for node_id in [1, 2, 3] {
        if matches!(
            state(node_id).await.role,
            omega_toy_consensus::NodeRole::Leader
        ) {
            return node_id;
        }
    }
    panic!("leader exists after 3s");
}

async fn submit_to(node_id: u64, claim: ClaimTx) -> omega_toy_consensus::SubmitOutcome {
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(node_url(node_id))
        .unwrap();
    let mut params = jsonrpsee::core::params::ObjectParams::new();
    params.insert("claim", claim).unwrap();
    client.request("omega_submitClaim", params).await.unwrap()
}

async fn wait_all_applied(applied_index: u64) {
    let mut observed = Vec::new();
    for _ in 0..120 {
        observed.clear();
        let mut all_applied = true;
        for node_id in [1, 2, 3] {
            let state = state(node_id).await;
            all_applied &= state.applied_index >= applied_index
                && state.nullifier_count >= 1
                && state.starstream_utxo_count >= 1;
            observed.push((node_id, state));
        }
        if all_applied {
            return;
        }
        tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(250)))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    panic!("not all nodes applied index {applied_index}: {observed:?}");
}

#[test]
fn drop_first_appendentries_eventually_recovers() -> turmoil::Result {
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(15);
    let mut sim = common::three_node_sim();

    sim.client("client", async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        fail::cfg(FAILPOINT, "1*return->off").unwrap();

        let outcome = submit_to(leader().await, claim).await;
        assert!(outcome.accepted);
        let applied_index = outcome.applied_index.expect("accepted outcome has index");
        fail::remove(FAILPOINT);

        wait_all_applied(applied_index).await;
        Ok(())
    });

    let result = sim.run();
    fail::remove(FAILPOINT);
    result
}
