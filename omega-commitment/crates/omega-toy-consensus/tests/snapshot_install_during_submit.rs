//! Turmoil test for submit consistency across an elapsed-time window.
//!
//! NOTE: This test does NOT trigger an `install_snapshot` RPC; v0.1 has
//! no public snapshot trigger and the elapsed `sleep` window does not
//! force openraft to take a snapshot. The test asserts only that three
//! sequential submits commit and replicate. Group 2 / Group 3 will
//! replace this with a real snapshot-install integration test once an
//! RPC-level snapshot trigger lands. See
//! `cardano-wiki/wiki/pages/loganet-roadmap.md` § "Group 1" — last
//! "out of scope" bullet.

mod common;

use std::time::Duration;

use jsonrpsee::core::client::ClientT;
use omega_claim_tx::ClaimTx;

async fn submit(
    client: &jsonrpsee::http_client::HttpClient,
    claim: ClaimTx,
) -> omega_toy_consensus::SubmitOutcome {
    let mut params = jsonrpsee::core::params::ObjectParams::new();
    params.insert("claim", claim).unwrap();
    client.request("omega_submitClaim", params).await.unwrap()
}

async fn state(node_id: u64) -> omega_toy_consensus::NodeState {
    let url = format!("http://127.0.0.1:800{node_id}");
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .build(url)
        .unwrap();
    client
        .request(
            "omega_getState",
            jsonrpsee::core::params::ArrayParams::new(),
        )
        .await
        .unwrap()
}

async fn wait_counts_match(expected_nullifiers: u64, expected_utxos: u64) {
    let mut observed = Vec::new();
    for _ in 0..120 {
        observed.clear();
        let mut all_match = true;
        for node_id in [1, 2, 3] {
            let state = state(node_id).await;
            all_match &= state.nullifier_count == expected_nullifiers
                && state.starstream_utxo_count == expected_utxos;
            observed.push((node_id, state));
        }
        if all_match {
            return;
        }
        tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(250)))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    panic!("node counts did not converge: {observed:?}");
}

/// Three sequential submits commit and replicate across a multi-second
/// elapsed window. Renamed from `snapshot_install_mid_submit_keeps_state_consistent`
/// to acknowledge that no snapshot install is exercised in v0.1 — the
/// elapsed window does not force openraft to take or install a snapshot.
#[test]
fn three_submits_across_elapsed_window_replicate_to_all_nodes() -> turmoil::Result {
    let warmups = [
        common::synthetic_claim::synthetic_accepted_claim_for_leaf(0),
        common::synthetic_claim::synthetic_accepted_claim_for_leaf(1),
    ];
    let final_claim = common::synthetic_claim::synthetic_accepted_claim_for_leaf(50);
    let mut sim = common::three_node_sim();

    sim.client("client", async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let leader_url = common::leader_url().await;
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .request_timeout(Duration::from_secs(300))
            .build(&leader_url)
            .unwrap();

        for claim in warmups {
            let outcome = submit(&client, claim).await;
            assert!(outcome.accepted);
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
        // Group 1 has no public snapshot trigger; this elapsed window follows
        // the plan without adding an extra JSON-RPC method.
        tokio::time::sleep(Duration::from_secs(10)).await;

        let outcome = submit(&client, final_claim).await;
        assert!(outcome.accepted);

        tokio::time::sleep(Duration::from_secs(5)).await;
        let leader_state: omega_toy_consensus::NodeState = client
            .request(
                "omega_getState",
                jsonrpsee::core::params::ArrayParams::new(),
            )
            .await
            .unwrap();
        assert_eq!(leader_state.nullifier_count, 3);
        assert_eq!(leader_state.starstream_utxo_count, 3);
        wait_counts_match(
            leader_state.nullifier_count,
            leader_state.starstream_utxo_count,
        )
        .await;
        Ok(())
    });

    sim.run()
}
