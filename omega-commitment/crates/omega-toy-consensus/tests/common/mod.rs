//! Shared helpers for turmoil-based 3-node tests.

pub mod synthetic_claim;

use std::path::PathBuf;
use std::time::Duration;

use jsonrpsee::core::client::ClientT;
use omega_toy_consensus::{NodeConfig, PeerConfig, RpcConfig};

/// Builds a 3-node LoganNet config triple.
#[allow(dead_code)]
pub fn three_node_configs() -> [NodeConfig; 3] {
    three_node_configs_with_deadline(Duration::from_secs(3_600))
}

/// Builds a 3-node LoganNet config triple with a custom apply deadline.
pub fn three_node_configs_with_deadline(apply_deadline: Duration) -> [NodeConfig; 3] {
    let peer = |id: u64| PeerConfig {
        node_id: id,
        libp2p_addr: format!("/ip4/0.0.0.0/tcp/{}", 4000 + id),
        rpc_url: format!("http://127.0.0.1:800{id}"),
    };
    let mk = |id: u64, peers: Vec<PeerConfig>| NodeConfig {
        node_id: id,
        data_dir: ledger_path(id),
        libp2p_listen: format!("/ip4/0.0.0.0/tcp/{}", 4000 + id),
        peers,
        rpc: RpcConfig {
            bind: format!("0.0.0.0:{}", 8000 + id).parse().unwrap(),
            max_batch: 25,
            max_request_bytes: 16 * 1024 * 1024,
        },
        cluster_id: "loganet-test".into(),
        apply_deadline,
    };
    [
        mk(1, vec![peer(2), peer(3)]),
        mk(2, vec![peer(1), peer(3)]),
        mk(3, vec![peer(1), peer(2)]),
    ]
}

/// Boots a 3-node turmoil sim.
pub fn three_node_sim() -> turmoil::Sim<'static> {
    three_node_sim_with_deadline(Duration::from_secs(3_600), Duration::from_secs(3_600))
}

/// Boots a 3-node turmoil sim with custom apply and simulation deadlines.
pub fn three_node_sim_with_deadline(
    apply_deadline: Duration,
    simulation_duration: Duration,
) -> turmoil::Sim<'static> {
    omega_toy_consensus::test_support::clear_raft_link_blocks();
    let configs = three_node_configs_with_deadline(apply_deadline);
    let mut builder = turmoil::Builder::new();
    let mut sim = builder
        .enable_tokio_io()
        .simulation_duration(simulation_duration)
        .build();
    for cfg in configs {
        let cfg_clone = cfg.clone();
        let host = format!("node{}", cfg_clone.node_id);
        sim.host(host.as_str(), move || {
            let cfg = cfg_clone.clone();
            async move {
                let _handle = omega_toy_consensus::start(cfg).await?;
                std::future::pending::<()>().await;
                Ok(())
            }
        });
    }
    sim
}

/// Returns the current leader's localhost RPC URL.
#[allow(dead_code)]
pub async fn leader_url() -> String {
    for node_id in [1, 2, 3] {
        let url = format!("http://127.0.0.1:800{node_id}");
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
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
            return url;
        }
    }
    panic!("no leader found");
}

fn ledger_path(node_id: u64) -> PathBuf {
    tempfile::tempdir()
        .expect("tempdir")
        .keep()
        .join(format!("node-{node_id}.sqlite"))
}
