//! Openraft network trait adapter tests.

use std::time::Duration;

use omega_network::rpc::{decode_cbor, encode_cbor, RaftRpcRequest, RaftRpcResponse};
use omega_network::LibP2pNetworkFactory;
use openraft::network::{RPCOption, RaftNetwork, RaftNetworkFactory};
use openraft::raft::{VoteRequest, VoteResponse};
use openraft::{BasicNode, Vote};
use tokio::sync::mpsc;

#[tokio::test]
async fn vote_rpc_uses_cbor_request_response_channel() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut factory = LibP2pNetworkFactory::new(tx);
    let mut client = factory.new_client(2, &BasicNode::new("/memory/2")).await;

    let actor = tokio::spawn(async move {
        let outbound = rx.recv().await.expect("outbound request");
        assert_eq!(outbound.target, 2);
        assert_eq!(outbound.node, BasicNode::new("/memory/2"));

        let request: RaftRpcRequest = decode_cbor(&outbound.payload).expect("decode request");
        let RaftRpcRequest::Vote(vote_request) = request else {
            panic!("request must be vote");
        };
        assert_eq!(vote_request, VoteRequest::new(Vote::new(5, 1), None));

        let response = RaftRpcResponse::Vote(VoteResponse::new(Vote::new(5, 2), None, true));
        let payload = encode_cbor(&response).expect("encode response");
        outbound.reply.send(Ok(payload)).expect("reply send");
    });

    let response = client
        .vote(
            VoteRequest::new(Vote::new(5, 1), None),
            RPCOption::new(Duration::from_secs(1)),
        )
        .await
        .expect("vote response");

    assert_eq!(response, VoteResponse::new(Vote::new(5, 2), None, true));
    actor.await.expect("actor joins");
}
