//! CBOR Raft RPC envelope tests.

use omega_network::rpc::{decode_cbor, encode_cbor, RaftRpcRequest, RaftRpcResponse};
use openraft::raft::{VoteRequest, VoteResponse};
use openraft::Vote;

#[test]
fn raft_vote_request_round_trips_through_cbor_envelope() {
    let vote = Vote::new(3, 2);
    let request = RaftRpcRequest::Vote(VoteRequest::new(vote, None));

    let encoded = encode_cbor(&request).expect("encode request");
    let decoded: RaftRpcRequest = decode_cbor(&encoded).expect("decode request");

    let RaftRpcRequest::Vote(decoded_vote) = decoded else {
        panic!("decoded request must be vote");
    };
    let RaftRpcRequest::Vote(expected_vote) = request else {
        panic!("expected request must be vote");
    };
    assert_eq!(decoded_vote, expected_vote);
}

#[test]
fn raft_vote_response_round_trips_through_cbor_envelope() {
    let vote = Vote::new(4, 1);
    let response = RaftRpcResponse::Vote(VoteResponse::new(vote, None, true));

    let encoded = encode_cbor(&response).expect("encode response");
    let decoded: RaftRpcResponse = decode_cbor(&encoded).expect("decode response");

    let RaftRpcResponse::Vote(decoded_vote) = decoded else {
        panic!("decoded response must be vote");
    };
    let RaftRpcResponse::Vote(expected_vote) = response else {
        panic!("expected response must be vote");
    };
    assert_eq!(decoded_vote, expected_vote);
}
