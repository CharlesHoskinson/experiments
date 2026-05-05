use std::path::PathBuf;
use std::time::Duration;

use super::{NodeConfig, PeerConfig};

#[test]
fn single_node_localhost_basic() {
    let cfg = NodeConfig::single_node_localhost(1).unwrap();
    assert_eq!(cfg.node_id, 1);
    assert_eq!(cfg.libp2p_listen, "/ip4/127.0.0.1/tcp/4001");
    assert_eq!(cfg.rpc.bind.port(), 8001);
    assert_eq!(cfg.rpc.max_batch, 25);
    assert_eq!(cfg.rpc.max_request_bytes, 1024 * 1024);
    assert_eq!(cfg.apply_deadline, Duration::from_secs(5));
}

#[test]
fn single_node_localhost_rejects_zero() {
    let err = NodeConfig::single_node_localhost(0).unwrap_err();
    assert!(matches!(err, crate::ConsensusError::Config(_)));
}

#[test]
fn peer_config_parse_ok() {
    let p: PeerConfig = "2,/ip4/127.0.0.1/tcp/4002,http://127.0.0.1:8002"
        .parse()
        .unwrap();
    assert_eq!(p.node_id, 2);
    assert_eq!(p.libp2p_addr, "/ip4/127.0.0.1/tcp/4002");
    assert_eq!(p.rpc_url, "http://127.0.0.1:8002");
}

#[test]
fn peer_config_parse_too_few_fields() {
    let err: Result<PeerConfig, _> = "2,/ip4/127.0.0.1/tcp/4002".parse();
    assert!(err.is_err());
}

#[test]
fn peer_config_parse_bad_node_id() {
    let err: Result<PeerConfig, _> = "abc,/ip4/127.0.0.1/tcp/4002,http://x".parse();
    assert!(err.is_err());
}

#[test]
fn node_config_serde_round_trip_toml() {
    let cfg = NodeConfig {
        node_id: 7,
        data_dir: PathBuf::from("/tmp/x"),
        libp2p_listen: "/ip4/127.0.0.1/tcp/4007".into(),
        peers: vec![PeerConfig {
            node_id: 2,
            libp2p_addr: "/ip4/127.0.0.1/tcp/4002".into(),
            rpc_url: "http://127.0.0.1:8002".into(),
        }],
        rpc: super::RpcConfig {
            bind: "127.0.0.1:8007".parse().unwrap(),
            max_batch: 25,
            max_request_bytes: 1024 * 1024,
        },
        cluster_id: "loganet-dev".into(),
        apply_deadline: Duration::from_secs(5),
    };
    let toml = toml::to_string(&cfg).unwrap();
    let back: NodeConfig = toml::from_str(&toml).unwrap();
    assert_eq!(back.node_id, 7);
    assert_eq!(back.peers.len(), 1);
    assert_eq!(back.apply_deadline, Duration::from_secs(5));
}
