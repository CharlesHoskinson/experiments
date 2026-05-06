//! Shared helpers for turmoil-based 3-node tests.

pub mod synthetic_claim;

use std::path::PathBuf;
use std::time::Duration;

use omega_toy_consensus::{NodeConfig, PeerConfig, RpcConfig};

/// Builds a 3-node LoganNet config triple.
pub fn three_node_configs() -> [NodeConfig; 3] {
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
        apply_deadline: Duration::from_secs(3_600),
    };
    [
        mk(1, vec![peer(2), peer(3)]),
        mk(2, vec![peer(1), peer(3)]),
        mk(3, vec![peer(1), peer(2)]),
    ]
}

/// Boots a 3-node turmoil sim.
pub fn three_node_sim() -> turmoil::Sim<'static> {
    let configs = three_node_configs();
    let mut builder = turmoil::Builder::new();
    let mut sim = builder
        .enable_tokio_io()
        .simulation_duration(Duration::from_secs(3_600))
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

fn ledger_path(node_id: u64) -> PathBuf {
    tempfile::tempdir()
        .expect("tempdir")
        .keep()
        .join(format!("node-{node_id}.sqlite"))
}
