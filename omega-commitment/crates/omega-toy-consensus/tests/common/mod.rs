//! Shared helpers for turmoil-based 3-node tests.

pub mod synthetic_claim;

use std::path::PathBuf;
use std::time::Duration;

use jsonrpsee::core::client::ClientT;
use omega_toy_consensus::{NodeConfig, PeerConfig, RpcConfig};

/// Builds a 3-node LoganNet config triple.
#[allow(dead_code)]
pub fn three_node_configs() -> [NodeConfig; 3] {
    three_node_configs_with_deadline(Duration::from_secs(60))
}

/// Builds a 3-node LoganNet config triple with a custom apply deadline.
///
/// `rpc.bind` uses `127.0.0.1` (loopback) so the v0.1 loopback-enforcement
/// in `validate_config` accepts the bring-up. `libp2p_listen` stays on
/// `/ip4/0.0.0.0/...` for the listen-on-all-interfaces convention; peers dial
/// localhost because libp2p uses Tokio TCP rather than turmoil's simulated
/// TCP stack.
pub fn three_node_configs_with_deadline(apply_deadline: Duration) -> [NodeConfig; 3] {
    let keypairs: [omega_network::identity::Keypair; 3] =
        std::array::from_fn(|_| omega_network::identity::Keypair::generate_ed25519());
    let peer_ids: [String; 3] =
        std::array::from_fn(|idx| keypairs[idx].public().to_peer_id().to_string());
    let paths: [(PathBuf, PathBuf); 3] = std::array::from_fn(|idx| node_paths((idx + 1) as u64));
    let libp2p_ports: [u16; 3] = std::array::from_fn(|_| free_tcp_port());
    for idx in 0..3 {
        write_identity(&paths[idx].1, &keypairs[idx]);
    }

    let peer = |id: u64| PeerConfig {
        node_id: id,
        libp2p_peer_id: peer_ids[(id - 1) as usize].clone(),
        libp2p_addr: format!("/ip4/127.0.0.1/tcp/{}", libp2p_ports[(id - 1) as usize]),
        rpc_url: format!("http://127.0.0.1:800{id}"),
    };
    let mk = |id: u64, peers: Vec<PeerConfig>| {
        let idx = (id - 1) as usize;
        NodeConfig {
            node_id: id,
            data_dir: paths[idx].0.clone(),
            identity_file: Some(paths[idx].1.clone()),
            libp2p_listen: format!("/ip4/0.0.0.0/tcp/{}", libp2p_ports[idx]),
            peers,
            rpc: RpcConfig {
                bind: format!("127.0.0.1:{}", 8000 + id).parse().unwrap(),
                max_batch: 25,
                max_request_bytes: 16 * 1024 * 1024,
            },
            cluster_id: "loganet-test".into(),
            apply_deadline,
            raft_rpc_timeout: None,
        }
    };
    [
        mk(1, vec![peer(2), peer(3)]),
        mk(2, vec![peer(1), peer(3)]),
        mk(3, vec![peer(1), peer(2)]),
    ]
}

/// Boots a 3-node turmoil sim.
#[allow(dead_code)]
pub fn three_node_sim() -> turmoil::Sim<'static> {
    three_node_sim_with_deadline(Duration::from_secs(60), Duration::from_secs(300))
}

/// Boots a 3-node turmoil sim with custom apply and simulation deadlines.
pub fn three_node_sim_with_deadline(
    apply_deadline: Duration,
    simulation_duration: Duration,
) -> turmoil::Sim<'static> {
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
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut last_observed = Vec::new();
    loop {
        let mut leaders = Vec::new();
        last_observed.clear();
        for node_id in [1, 2, 3] {
            let url = format!("http://127.0.0.1:800{node_id}");
            let client = jsonrpsee::http_client::HttpClientBuilder::default()
                .build(&url)
                .unwrap();
            match client
                .request::<omega_toy_consensus::NodeState, _>(
                    "omega_getState",
                    jsonrpsee::core::params::ArrayParams::new(),
                )
                .await
            {
                Ok(state) => {
                    last_observed.push(format!("{node_id}:{:?}", state.role));
                    if matches!(state.role, omega_toy_consensus::NodeRole::Leader) {
                        leaders.push(url);
                    }
                }
                Err(error) => {
                    last_observed.push(format!("{node_id}:error:{error}"));
                }
            }
        }
        if leaders.len() == 1 {
            return leaders.remove(0);
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("no single leader found within 30s: {last_observed:?}");
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn node_paths(node_id: u64) -> (PathBuf, PathBuf) {
    let root = tempfile::tempdir().expect("tempdir").keep();
    (
        root.join(format!("node-{node_id}.sqlite")),
        root.join(format!("node-{node_id}.identity.bin")),
    )
}

fn write_identity(path: &std::path::Path, keypair: &omega_network::identity::Keypair) {
    let bytes = keypair.to_protobuf_encoding().expect("identity encode");
    std::fs::write(path, bytes).expect("identity write");
}

fn free_tcp_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .expect("free TCP port")
        .local_addr()
        .expect("local addr")
        .port()
}
