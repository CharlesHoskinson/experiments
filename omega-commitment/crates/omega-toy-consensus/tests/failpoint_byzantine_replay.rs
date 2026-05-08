//! Failpoint test for term monotonicity under replayed stale vote responses.
//!
//! NOTE: This test asserts only that the observed term on node 1 does
//! not regress while the `omega_network::receive_vote_replay` failpoint
//! is active. It does NOT verify that any RPC returns `-32003 Replay`
//! (the orchestrator excerpt's "Nullifier replay translation" row is
//! covered by `tests/routing.rs::ledger_replay_emits_neg_32003_with_hint`,
//! a unit test on the translator). Strengthening to a full byzantine
//! replay rejection test is Group 3 work — see
//! `cardano-wiki/wiki/pages/loganet-roadmap.md` § "Group 3".

#![cfg(feature = "failpoints")]

mod common;

use std::time::Duration;

use jsonrpsee::core::client::ClientT;

const FAILPOINT: &str = "omega_network::receive_vote_replay";

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

async fn observed_term(node_id: u64) -> u64 {
    state(node_id)
        .await
        .last_log_id
        .map(|log| log.term)
        .unwrap_or(0)
}

/// Term on node 1 is monotonic (`>=`) across a 2-second window with the
/// `omega_network::receive_vote_replay` failpoint active. This is a
/// safety smoke, not a binding rejection test — see the module doc.
#[test]
fn term_monotonic_under_vote_replay_failpoint() -> turmoil::Result {
    let mut sim = common::three_node_sim();

    sim.client("client", async move {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let term_before = observed_term(1).await;

        fail::cfg(FAILPOINT, "return").unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;
        fail::remove(FAILPOINT);

        let term_after = observed_term(1).await;
        assert!(
            term_after >= term_before,
            "term regressed: {term_before} -> {term_after}"
        );
        Ok(())
    });

    let result = sim.run();
    fail::remove(FAILPOINT);
    result
}
