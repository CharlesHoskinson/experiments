//! Two `RaftSwarm` instances exchange a real raft RPC over loopback libp2p.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use libp2p::Multiaddr;
use omega_network::inbound::{PeerEntry, RaftSwarm};
use omega_network::rpc::{OmegaNetworkError, RaftRpcRequest, RaftRpcResponse};
use omega_network::{InboundRaftHandler, OutboundRaftRequest};
use openraft::raft::{AppendEntriesRequest, AppendEntriesResponse, VoteRequest, VoteResponse};
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

struct StubAppendHandler {
    node_id: u64,
}

#[async_trait]
impl InboundRaftHandler for StubAppendHandler {
    async fn handle(&self, req: RaftRpcRequest) -> Result<RaftRpcResponse, OmegaNetworkError> {
        match req {
            RaftRpcRequest::AppendEntries(_) => Ok(RaftRpcResponse::AppendEntries(
                AppendEntriesResponse::Success,
            )),
            RaftRpcRequest::Vote(_) => Ok(RaftRpcResponse::Vote(VoteResponse::new(
                Vote::new(1, self.node_id),
                None,
                true,
            ))),
            _ => Err(OmegaNetworkError::Codec("unexpected request".into())),
        }
    }
}

struct AlwaysErrHandler;

#[async_trait]
impl InboundRaftHandler for AlwaysErrHandler {
    async fn handle(&self, _req: RaftRpcRequest) -> Result<RaftRpcResponse, OmegaNetworkError> {
        Err(OmegaNetworkError::Codec("simulated handler failure".into()))
    }
}

fn vote_payload(term: u64, voter: u64) -> Vec<u8> {
    omega_network::rpc::encode_cbor(&RaftRpcRequest::Vote(VoteRequest {
        vote: Vote::new(term, voter),
        last_log_id: None,
    }))
    .unwrap()
}

fn append_entries_payload(term: u64, leader: u64) -> Vec<u8> {
    omega_network::rpc::encode_cbor(&RaftRpcRequest::AppendEntries(Box::new(
        AppendEntriesRequest {
            vote: Vote::new(term, leader),
            prev_log_id: None,
            entries: vec![],
            leader_commit: None,
        },
    )))
    .unwrap()
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_swarms_round_trip_append_entries() {
    let listen_1: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    let listen_2: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();

    let (_out_tx_1, out_rx_1) = mpsc::channel::<OutboundRaftRequest>(8);
    let (out_tx_2, out_rx_2) = mpsc::channel::<OutboundRaftRequest>(8);

    let mut swarm_1 = RaftSwarm::new(
        listen_1,
        vec![],
        out_rx_1,
        Arc::new(StubAppendHandler { node_id: 1 }),
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
        Arc::new(StubAppendHandler { node_id: 2 }),
    )
    .await
    .unwrap();

    let _t1 = tokio::spawn(async move { swarm_1.run().await });
    let _t2 = tokio::spawn(async move { swarm_2.run().await });

    let (reply_tx, reply_rx) = oneshot::channel();
    out_tx_2
        .send(OutboundRaftRequest {
            target: 1,
            node: BasicNode::new("node1"),
            payload: append_entries_payload(3, 2),
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
    assert!(matches!(
        response,
        RaftRpcResponse::AppendEntries(AppendEntriesResponse::Success)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn outbound_to_unknown_peer_returns_outbound_closed() {
    let listen_1: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    let (out_tx, out_rx) = mpsc::channel::<OutboundRaftRequest>(8);

    let swarm = RaftSwarm::new(
        listen_1,
        vec![],
        out_rx,
        Arc::new(StubVoteHandler { node_id: 1 }),
    )
    .await
    .unwrap();
    let _t = tokio::spawn(async move { swarm.run().await });

    let (reply_tx, reply_rx) = oneshot::channel();
    out_tx
        .send(OutboundRaftRequest {
            target: 99, // unknown
            node: BasicNode::new("node99"),
            payload: vote_payload(1, 1),
            reply: reply_tx,
        })
        .await
        .unwrap();

    let result = tokio::time::timeout(Duration::from_secs(2), reply_rx)
        .await
        .expect("reply within 2s")
        .unwrap();

    assert!(matches!(result, Err(OmegaNetworkError::OutboundClosed)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn handler_error_drops_response_so_requester_times_out() {
    use std::time::Duration;

    let listen_1: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    let listen_2: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    let (_out_tx_1, out_rx_1) = mpsc::channel::<OutboundRaftRequest>(8);
    let (out_tx_2, out_rx_2) = mpsc::channel::<OutboundRaftRequest>(8);

    let mut swarm_1 = RaftSwarm::with_request_timeout(
        listen_1,
        vec![],
        out_rx_1,
        Arc::new(AlwaysErrHandler),
        Duration::from_millis(500),
    )
    .await
    .unwrap();
    let peer_id_1 = swarm_1.local_peer_id();
    let address_1 = swarm_1.first_listen_address().await;

    let swarm_2 = RaftSwarm::with_request_timeout(
        listen_2,
        vec![PeerEntry {
            node_id: 1,
            peer_id: peer_id_1,
            address: address_1,
        }],
        out_rx_2,
        Arc::new(StubVoteHandler { node_id: 2 }),
        Duration::from_millis(500),
    )
    .await
    .unwrap();

    let _t1 = tokio::spawn(async move { swarm_1.run().await });
    let _t2 = tokio::spawn(async move { swarm_2.run().await });

    let (reply_tx, reply_rx) = oneshot::channel();
    out_tx_2
        .send(OutboundRaftRequest {
            target: 1,
            node: BasicNode::new("node1"),
            payload: vote_payload(1, 2),
            reply: reply_tx,
        })
        .await
        .unwrap();

    // The handler errors -> swarm 1 drops the response channel -> swarm 2's
    // request-response substream closes -> outbound failure surfaces as one of:
    //   - `Timeout` if request_timeout fires before the EOF is observed
    //   - `OutboundClosed` if the connection-closed event arrives first
    //   - `Codec("unexpected end of file")` if the substream EOF reaches the
    //     CBOR codec mid-read (libp2p's `OutboundFailure::Io`).
    // All three are a valid "no typed reply was fabricated" signal; what
    // matters for D.5 is that the response is NOT a wrong-typed
    // `RaftRpcResponse::Vote`.
    let result = tokio::time::timeout(Duration::from_secs(3), reply_rx)
        .await
        .expect("reply within 3s")
        .unwrap();
    assert!(
        result.is_err(),
        "handler error must NOT yield a typed Ok response: {result:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ten_concurrent_in_flight_requests_all_complete() {
    let listen_1: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    let listen_2: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    let (_out_tx_1, out_rx_1) = mpsc::channel::<OutboundRaftRequest>(32);
    let (out_tx_2, out_rx_2) = mpsc::channel::<OutboundRaftRequest>(32);

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

    // Warm up the libp2p connection with a single request before bursting
    // the concurrent batch — otherwise the first few requests can race the
    // dial and surface as `OutboundClosed`. A real raft cluster has the same
    // warm-up via heartbeats during election, so this is not a production
    // concern.
    let (warmup_tx, warmup_rx) = oneshot::channel();
    out_tx_2
        .send(OutboundRaftRequest {
            target: 1,
            node: BasicNode::new("node1"),
            payload: vote_payload(5, 2),
            reply: warmup_tx,
        })
        .await
        .unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(10), warmup_rx)
        .await
        .expect("warmup within 10s")
        .unwrap()
        .expect("warmup transport ok");

    let mut replies = Vec::new();
    for _ in 0..10 {
        let (reply_tx, reply_rx) = oneshot::channel();
        out_tx_2
            .send(OutboundRaftRequest {
                target: 1,
                node: BasicNode::new("node1"),
                payload: vote_payload(5, 2),
                reply: reply_tx,
            })
            .await
            .unwrap();
        replies.push(reply_rx);
    }

    for reply_rx in replies {
        let bytes = tokio::time::timeout(Duration::from_secs(10), reply_rx)
            .await
            .expect("reply within 10s")
            .unwrap()
            .expect("transport ok");
        let response: RaftRpcResponse = omega_network::rpc::decode_cbor(&bytes).unwrap();
        assert!(matches!(response, RaftRpcResponse::Vote(_)));
    }
}
