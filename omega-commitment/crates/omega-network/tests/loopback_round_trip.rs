//! Two `RaftSwarm` instances exchange a real raft RPC over loopback libp2p.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use libp2p::Multiaddr;
use omega_network::inbound::{PeerEntry, RaftSwarm};
use omega_network::rpc::{OmegaNetworkError, RaftRpcRequest, RaftRpcResponse};
use omega_network::{InboundRaftHandler, OutboundRaftRequest};
use openraft::raft::{VoteRequest, VoteResponse};
use openraft::{BasicNode, Vote};
use tokio::sync::{mpsc, oneshot};

struct StubVoteHandler {
    node_id: u64,
}

#[async_trait]
impl InboundRaftHandler for StubVoteHandler {
    async fn handle(&self, req: RaftRpcRequest) -> Result<RaftRpcResponse, OmegaNetworkError> {
        match req {
            RaftRpcRequest::Vote(_) => Ok(RaftRpcResponse::Vote(VoteResponse::new(
                Vote::new(7, self.node_id),
                None,
                true,
            ))),
            _ => Err(OmegaNetworkError::Codec("unexpected request".into())),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_swarms_round_trip_vote() {
    let listen_1: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    let listen_2: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();

    let (_out_tx_1, out_rx_1) = mpsc::channel::<OutboundRaftRequest>(8);
    let (out_tx_2, out_rx_2) = mpsc::channel::<OutboundRaftRequest>(8);

    let mut swarm_1 = RaftSwarm::new(
        listen_1,
        vec![],
        out_rx_1,
        Arc::new(StubVoteHandler { node_id: 1 }),
    )
    .await
    .unwrap();

    let peer_id_1 = swarm_1.local_peer_id();
    let address_1 = swarm_1.first_listen_address().await;

    let swarm_2 = RaftSwarm::new(
        listen_2,
        vec![PeerEntry {
            node_id: 1,
            peer_id: peer_id_1,
            address: address_1,
        }],
        out_rx_2,
        Arc::new(StubVoteHandler { node_id: 2 }),
    )
    .await
    .unwrap();

    let _t1 = tokio::spawn(async move { swarm_1.run().await });
    let _t2 = tokio::spawn(async move { swarm_2.run().await });

    let (reply_tx, reply_rx) = oneshot::channel();
    let payload = omega_network::rpc::encode_cbor(&RaftRpcRequest::Vote(VoteRequest {
        vote: Vote::new(5, 2),
        last_log_id: None,
    }))
    .unwrap();

    out_tx_2
        .send(OutboundRaftRequest {
            target: 1,
            node: BasicNode::new("node1"),
            payload,
            reply: reply_tx,
        })
        .await
        .unwrap();

    let response_bytes = tokio::time::timeout(Duration::from_secs(10), reply_rx)
        .await
        .expect("response within 10s")
        .unwrap()
        .expect("transport ok");

    let response: RaftRpcResponse = omega_network::rpc::decode_cbor(&response_bytes).unwrap();
    let vote_response = match response {
        RaftRpcResponse::Vote(v) => v,
        _ => panic!("expected vote response"),
    };
    assert_eq!(vote_response.vote.leader_id().voted_for(), Some(1));
}
