//! CBOR Raft RPC envelope tests.

use omega_network::rpc::{
    decode_cbor, encode_cbor, OmegaNetworkError, RaftRpcRequest, RaftRpcResponse,
    MAX_RAFT_RPC_BYTES,
};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::{CommittedLeaderId, LogId, SnapshotMeta, StoredMembership, Vote};

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

#[test]
fn raft_append_entries_request_round_trips_through_cbor_envelope() {
    // Construct an empty-entries AppendEntries — its serde shape includes
    // every field that a real heartbeat carries, including
    // `prev_log_id`, `leader_commit`, and the typed `vote` envelope.
    let request: AppendEntriesRequest<omega_mock_ledger::OmegaRaftTypeConfig> =
        AppendEntriesRequest {
            vote: Vote::new(7, 1),
            prev_log_id: Some(LogId::new(CommittedLeaderId::new(7, 1), 12)),
            leader_commit: Some(LogId::new(CommittedLeaderId::new(7, 1), 11)),
            entries: vec![],
        };
    let envelope = RaftRpcRequest::AppendEntries(Box::new(request));

    let encoded = encode_cbor(&envelope).expect("encode request");
    let decoded: RaftRpcRequest = decode_cbor(&encoded).expect("decode request");

    let RaftRpcRequest::AppendEntries(back) = decoded else {
        panic!("decoded request must be append_entries");
    };
    assert_eq!(back.vote, Vote::new(7, 1));
    assert_eq!(
        back.prev_log_id,
        Some(LogId::new(CommittedLeaderId::new(7, 1), 12))
    );
    assert!(back.entries.is_empty());
}

#[test]
fn raft_append_entries_response_round_trips_through_cbor_envelope() {
    let response: AppendEntriesResponse<u64> = AppendEntriesResponse::Success;
    let envelope = RaftRpcResponse::AppendEntries(response);
    let encoded = encode_cbor(&envelope).expect("encode response");
    let decoded: RaftRpcResponse = decode_cbor(&encoded).expect("decode response");
    let RaftRpcResponse::AppendEntries(back) = decoded else {
        panic!("decoded response must be append_entries");
    };
    assert!(matches!(back, AppendEntriesResponse::Success));
}

#[test]
fn raft_install_snapshot_request_round_trips_through_cbor_envelope() {
    let request: InstallSnapshotRequest<omega_mock_ledger::OmegaRaftTypeConfig> =
        InstallSnapshotRequest {
            vote: Vote::new(9, 1),
            meta: SnapshotMeta {
                last_log_id: Some(LogId::new(CommittedLeaderId::new(9, 1), 100)),
                last_membership: StoredMembership::default(),
                snapshot_id: "snap-test".into(),
            },
            offset: 0,
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
            done: true,
        };
    let envelope = RaftRpcRequest::InstallSnapshot(Box::new(request));

    let encoded = encode_cbor(&envelope).expect("encode request");
    let decoded: RaftRpcRequest = decode_cbor(&encoded).expect("decode request");

    let RaftRpcRequest::InstallSnapshot(back) = decoded else {
        panic!("decoded request must be install_snapshot");
    };
    assert_eq!(back.meta.snapshot_id, "snap-test");
    assert_eq!(back.data, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    assert!(back.done);
}

#[test]
fn raft_install_snapshot_response_round_trips_through_cbor_envelope() {
    let response: InstallSnapshotResponse<u64> = InstallSnapshotResponse {
        vote: Vote::new(9, 1),
    };
    let envelope = RaftRpcResponse::InstallSnapshot(response);
    let encoded = encode_cbor(&envelope).expect("encode response");
    let decoded: RaftRpcResponse = decode_cbor(&encoded).expect("decode response");
    let RaftRpcResponse::InstallSnapshot(back) = decoded else {
        panic!("decoded response must be install_snapshot");
    };
    assert_eq!(back.vote, Vote::new(9, 1));
}

#[test]
fn decode_cbor_rejects_payloads_over_max_envelope_bytes() {
    // Any byte sequence longer than the cap is rejected before ciborium runs.
    let too_big = vec![0u8; MAX_RAFT_RPC_BYTES + 1];
    let err: Result<RaftRpcRequest, _> = decode_cbor(&too_big);
    let Err(OmegaNetworkError::Oversize { actual, max }) = err else {
        panic!("expected Oversize, got {err:?}");
    };
    assert_eq!(actual, MAX_RAFT_RPC_BYTES + 1);
    assert_eq!(max, MAX_RAFT_RPC_BYTES);
}

#[test]
fn decode_cbor_rejects_pathologically_nested_input() {
    // Build a CBOR document that is just nested arrays past the recursion
    // limit. The encoded form is `[[[...[]]]]` with depth > MAX_CBOR_RECURSION.
    let depth = 256_usize; // > MAX_CBOR_RECURSION (64)
    let mut cbor = vec![0x81_u8; depth]; // array(1) repeated `depth` times
    cbor.push(0x80); // empty array — the innermost element
    let err: Result<Vec<()>, _> = decode_cbor(&cbor);
    assert!(matches!(err, Err(OmegaNetworkError::Codec(_))));
}
