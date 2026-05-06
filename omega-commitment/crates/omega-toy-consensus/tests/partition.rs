//! Turmoil tests for partition behavior.

mod common;

use std::time::Duration;

use jsonrpsee::core::{client::ClientT, ClientError};
use omega_claim_tx::ClaimTx;

static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

async fn leader_and_followers() -> (u64, Vec<u64>) {
    let mut leader = None;
    let mut followers = Vec::new();
    for node_id in [1, 2, 3] {
        let state = state(node_id).await;
        if matches!(state.role, omega_toy_consensus::NodeRole::Leader) {
            leader = Some(node_id);
        } else {
            followers.push(node_id);
        }
    }
    (leader.expect("leader exists after 3s"), followers)
}

fn partition_pair(a: u64, b: u64) {
    let left = format!("node{a}");
    let right = format!("node{b}");
    turmoil::partition(left.as_str(), right.as_str());
    omega_toy_consensus::test_support::partition_raft_link(a, b);
}

fn turmoil_partition_pair(a: u64, b: u64) {
    let left = format!("node{a}");
    let right = format!("node{b}");
    turmoil::partition(left.as_str(), right.as_str());
}

async fn submit(
    node_id: u64,
    claim: ClaimTx,
) -> Result<omega_toy_consensus::SubmitOutcome, ClientError> {
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(node_url(node_id))
        .unwrap();
    let mut params = jsonrpsee::core::params::ObjectParams::new();
    params.insert("claim", claim).unwrap();
    client.request("omega_submitClaim", params).await
}

#[test]
fn partitioned_minority_does_not_commit() -> turmoil::Result {
    let _guard = TEST_LOCK.lock().unwrap();
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(13);
    let mut sim =
        common::three_node_sim_with_deadline(Duration::from_secs(5), Duration::from_secs(60));

    sim.client("client", async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let (leader, followers) = leader_and_followers().await;
        let minority = followers[0];
        partition_pair(minority, leader);
        partition_pair(minority, followers[1]);

        let result = tokio::time::timeout(Duration::from_secs(10), submit(minority, claim)).await;
        match result {
            Err(_elapsed) => {}
            Ok(Err(ClientError::Call(obj))) => {
                assert!(
                    obj.code() == -32000 || obj.code() == -32005,
                    "expected NotLeader or Timeout from minority node, got {}",
                    obj.code()
                );
            }
            Ok(Ok(outcome)) => panic!("minority must not accept: {outcome:?}"),
            Ok(Err(other)) => panic!("unexpected transport error: {other:?}"),
        }
        Ok(())
    });
    sim.run()
}

#[test]
fn partitioned_majority_continues_to_commit() -> turmoil::Result {
    let _guard = TEST_LOCK.lock().unwrap();
    let claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(99);
    let mut sim = common::three_node_sim();

    sim.client("client", async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let (leader, followers) = leader_and_followers().await;
        let minority = followers[0];
        turmoil_partition_pair(minority, leader);
        turmoil_partition_pair(minority, followers[1]);

        let result = submit(leader, claim).await;
        let outcome = result.expect("majority leader accepts");
        assert!(outcome.accepted);
        assert!(outcome.applied_index.is_some());
        Ok(())
    });
    sim.run()
}
