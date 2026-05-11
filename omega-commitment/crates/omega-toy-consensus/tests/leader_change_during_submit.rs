//! Turmoil test for a leader change racing a submit.

mod common;

use std::time::Duration;

use jsonrpsee::core::{client::ClientT, ClientError};

fn node_id_from_url(url: &str) -> u64 {
    let port = url.rsplit(':').next().unwrap().parse::<u64>().unwrap();
    port - 8000
}

fn partition_pair(a: u64, b: u64) {
    let left = format!("node{a}");
    let right = format!("node{b}");
    turmoil::partition(left.as_str(), right.as_str());
}

#[test]
fn leader_change_during_submit_yields_disjunction() -> turmoil::Result {
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(75);
    let mut sim =
        common::three_node_sim_with_deadline(Duration::from_secs(5), Duration::from_secs(60));

    sim.client("client", async move {
        let leader_url = common::leader_url().await;
        let leader_id = node_id_from_url(&leader_url);
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(8))
            .build(&leader_url)
            .unwrap();

        let mut params = jsonrpsee::core::params::ObjectParams::new();
        params.insert("claim", claim).unwrap();
        let outcome = tokio::time::timeout(
            Duration::from_secs(12),
            client.request::<omega_toy_consensus::SubmitOutcome, _>("omega_submitClaim", params),
        )
        .await
        .ok();
        if let Some(peer) = [1, 2, 3].into_iter().find(|peer| *peer != leader_id) {
            partition_pair(leader_id, peer);
        }
        tokio::time::sleep(Duration::from_secs(2)).await;

        match outcome {
            None => {}
            Some(Err(ClientError::Call(obj))) => {
                assert!(
                    obj.code() == -32000 || obj.code() == -32005,
                    "expected NotLeader or Timeout, got {}",
                    obj.code()
                );
            }
            Some(Err(ClientError::RequestTimeout)) => {}
            Some(Ok(outcome)) => {
                assert!(outcome.accepted || outcome.reject_reason.is_some());
            }
            Some(Err(other)) => panic!("unexpected transport error: {other:?}"),
        }
        Ok(())
    });

    sim.run()
}
