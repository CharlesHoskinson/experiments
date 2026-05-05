//! Discovery configuration tests.

use std::str::FromStr;

use omega_network::discovery::{mdns_service_name, DiscoveryConfig, MdnsMode, PeerAddress};

#[test]
fn mdns_service_name_is_salted_and_not_libp2p_default() {
    let genesis = b"omega genesis fixture";
    let salt_a = b"install-a";
    let salt_b = b"install-b";

    let service_a = mdns_service_name(genesis, salt_a);
    let service_a_again = mdns_service_name(genesis, salt_a);
    let service_b = mdns_service_name(genesis, salt_b);

    assert_eq!(service_a, service_a_again);
    assert_ne!(service_a, service_b);
    assert!(service_a.starts_with("_omega-experiment-"));
    assert!(service_a.ends_with("._udp.local"));
    assert_ne!(service_a, "_p2p._udp.local");
}

#[test]
fn discovery_config_disables_mdns_and_preserves_static_peers() {
    let peer = PeerAddress::from_str("2=/memory/2").expect("peer parses");
    let config = DiscoveryConfig::new(b"genesis", b"salt", true, vec![peer.clone()]);

    assert_eq!(config.mdns, MdnsMode::Disabled);
    assert_eq!(config.static_peers, vec![peer]);
}

#[test]
fn static_peer_parser_rejects_malformed_inputs() {
    assert!(PeerAddress::from_str("/memory/2").is_err());
    assert!(PeerAddress::from_str("node=/memory/2").is_err());
    assert!(PeerAddress::from_str("2=").is_err());
}
