//! Run three in-process LoganNet nodes for local smoke testing.
//!
//! Start with `cargo run -p omega-toy-consensus --example three_node_local`.
//! Each node binds RPC on 127.0.0.1:8001, 127.0.0.1:8002, and
//! 127.0.0.1:8003. Press Ctrl-C to stop.

use std::time::Duration;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,omega_toy_consensus=debug,openraft=info")
        .init();

    let peer = |id: u64| omega_toy_consensus::PeerConfig {
        node_id: id,
        libp2p_addr: format!("/ip4/127.0.0.1/tcp/{}", 4000 + id),
        rpc_url: format!("http://127.0.0.1:{}", 8000 + id),
    };
    let mk =
        |id: u64, peers: Vec<omega_toy_consensus::PeerConfig>| omega_toy_consensus::NodeConfig {
            node_id: id,
            data_dir: tempfile::tempdir()
                .unwrap()
                .keep()
                .join(format!("node-{id}.sqlite")),
            libp2p_listen: format!("/ip4/127.0.0.1/tcp/{}", 4000 + id),
            peers,
            rpc: omega_toy_consensus::RpcConfig {
                bind: format!("127.0.0.1:{}", 8000 + id).parse().unwrap(),
                max_batch: 25,
                max_request_bytes: 16 * 1024 * 1024,
            },
            cluster_id: "loganet-dev".into(),
            apply_deadline: Duration::from_secs(3_600),
        };

    let h1 = omega_toy_consensus::start(mk(1, vec![peer(2), peer(3)])).await?;
    let h2 = omega_toy_consensus::start(mk(2, vec![peer(1), peer(3)])).await?;
    let h3 = omega_toy_consensus::start(mk(3, vec![peer(1), peer(2)])).await?;

    tracing::info!("three nodes up; RPC at 127.0.0.1:8001, 8002, and 8003");
    tokio::signal::ctrl_c().await?;

    h1.shutdown().await?;
    h2.shutdown().await?;
    h3.shutdown().await?;
    Ok(())
}
