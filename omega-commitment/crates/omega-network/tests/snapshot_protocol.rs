//! Snapshot wire-protocol tests.

use omega_network::snapshot::{
    chunk_snapshot_bytes, SnapshotAck, SnapshotFileReceiver, SnapshotFrame, SnapshotProtocolError,
    MAX_SNAPSHOT_CHUNK_BYTES,
};

#[test]
fn chunked_snapshot_frames_are_serial_and_bounded() {
    let bytes = vec![7_u8; MAX_SNAPSHOT_CHUNK_BYTES * 2 + 3];
    let frames = chunk_snapshot_bytes("snap-a", &bytes);

    assert_eq!(frames.len(), 5);
    let SnapshotFrame::Init(init) = &frames[0] else {
        panic!("first frame must be init");
    };
    assert_eq!(init.snapshot_id, "snap-a");
    assert_eq!(init.total_chunks, 3);
    assert_eq!(init.total_bytes, bytes.len() as u64);

    for (idx, frame) in frames[1..4].iter().enumerate() {
        let SnapshotFrame::Chunk(chunk) = frame else {
            panic!("middle frames must be chunks");
        };
        assert_eq!(chunk.snapshot_id, "snap-a");
        assert_eq!(chunk.chunk_idx, idx as u64);
        assert!(chunk.payload.len() <= MAX_SNAPSHOT_CHUNK_BYTES);
    }

    let SnapshotFrame::Finalize(finalize) = &frames[4] else {
        panic!("last frame must be finalize");
    };
    assert_eq!(finalize.snapshot_id, "snap-a");
    assert_eq!(finalize.sha3_of_full, init.sha3_of_full);
}

#[test]
fn file_receiver_persists_each_chunk_before_ack_and_installs_on_finalize() {
    let dir = tempfile::tempdir().expect("tempdir");
    let installed = dir.path().join("installed.sqlite");
    let bytes = vec![9_u8; MAX_SNAPSHOT_CHUNK_BYTES + 17];
    let frames = chunk_snapshot_bytes("snap-b", &bytes);
    let mut receiver = SnapshotFileReceiver::new(dir.path(), &installed);

    let ack = receiver.receive(frames[0].clone()).expect("init ack");
    assert_eq!(
        ack,
        SnapshotAck::Accepted {
            snapshot_id: "snap-b".to_string(),
            next_chunk_idx: 0,
        }
    );

    let ack = receiver
        .receive(frames[1].clone())
        .expect("first chunk ack");
    assert_eq!(
        ack,
        SnapshotAck::Accepted {
            snapshot_id: "snap-b".to_string(),
            next_chunk_idx: 1,
        }
    );
    let staged_path = receiver
        .active_staged_path()
        .expect("staged file exists after first chunk");
    assert_eq!(
        std::fs::metadata(staged_path).expect("metadata").len(),
        MAX_SNAPSHOT_CHUNK_BYTES as u64
    );

    receiver
        .receive(frames[2].clone())
        .expect("second chunk ack");
    let complete = receiver.receive(frames[3].clone()).expect("finalize ack");
    assert_eq!(
        complete,
        SnapshotAck::Complete {
            snapshot_id: "snap-b".to_string(),
            installed_path: installed.clone(),
        }
    );
    assert_eq!(std::fs::read(installed).expect("installed bytes"), bytes);
    assert!(receiver.active_staged_path().is_none());
}

#[test]
fn receiver_rejects_out_of_order_chunk() {
    let dir = tempfile::tempdir().expect("tempdir");
    let installed = dir.path().join("installed.sqlite");
    let bytes = vec![1_u8; MAX_SNAPSHOT_CHUNK_BYTES + 1];
    let frames = chunk_snapshot_bytes("snap-c", &bytes);
    let mut receiver = SnapshotFileReceiver::new(dir.path(), installed);

    receiver.receive(frames[0].clone()).expect("init ack");
    let error = receiver
        .receive(frames[2].clone())
        .expect_err("chunk 1 before chunk 0 fails");

    assert!(matches!(
        error,
        omega_network::snapshot::SnapshotReceiveError::Protocol(
            SnapshotProtocolError::OutOfOrderChunk {
                expected: 0,
                actual: 1
            }
        )
    ));
}

#[test]
fn receiver_rejects_hash_mismatch() {
    let dir = tempfile::tempdir().expect("tempdir");
    let installed = dir.path().join("installed.sqlite");
    let bytes = vec![2_u8; 64];
    let mut frames = chunk_snapshot_bytes("snap-d", &bytes);
    let finalize = match frames.pop().expect("finalize") {
        SnapshotFrame::Finalize(mut finalize) => {
            finalize.sha3_of_full[0] ^= 1;
            finalize
        }
        _ => panic!("last frame must be finalize"),
    };

    let mut receiver = SnapshotFileReceiver::new(dir.path(), installed);
    for frame in frames {
        receiver.receive(frame).expect("frame accepted");
    }
    let error = receiver
        .receive(SnapshotFrame::Finalize(finalize.clone()))
        .expect_err("hash mismatch fails");

    assert!(matches!(
        error,
        omega_network::snapshot::SnapshotReceiveError::Protocol(
            SnapshotProtocolError::HashMismatch
        )
    ));
}

#[test]
fn receiver_rejects_extra_zero_length_chunk_after_declared_total() {
    let dir = tempfile::tempdir().expect("tempdir");
    let installed = dir.path().join("installed.sqlite");
    let bytes = vec![4_u8; 32];
    let frames = chunk_snapshot_bytes("snap-extra", &bytes);
    let mut receiver = SnapshotFileReceiver::new(dir.path(), installed);

    receiver.receive(frames[0].clone()).expect("init ack");
    receiver.receive(frames[1].clone()).expect("chunk ack");
    let error = receiver
        .receive(SnapshotFrame::Chunk(
            omega_network::snapshot::SnapshotChunk {
                snapshot_id: "snap-extra".to_string(),
                chunk_idx: 1,
                payload: Vec::new(),
            },
        ))
        .expect_err("extra empty chunk fails");

    assert!(matches!(
        error,
        omega_network::snapshot::SnapshotReceiveError::Protocol(
            SnapshotProtocolError::UnexpectedChunk {
                total_chunks: 1,
                actual: 1
            }
        )
    ));
}

#[test]
fn leader_change_aborts_active_snapshot_and_discards_partial_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let installed = dir.path().join("installed.sqlite");
    let bytes = vec![3_u8; MAX_SNAPSHOT_CHUNK_BYTES + 1];
    let frames = chunk_snapshot_bytes("snap-e", &bytes);
    let mut receiver = SnapshotFileReceiver::new(dir.path(), installed);

    receiver.receive(frames[0].clone()).expect("init ack");
    receiver.receive(frames[1].clone()).expect("chunk ack");
    let staged_path = receiver
        .active_staged_path()
        .expect("staged file exists")
        .to_path_buf();

    assert!(receiver.abort_on_leader_change().expect("abort succeeds"));
    assert!(!staged_path.exists());
    assert!(receiver.active_staged_path().is_none());

    let error = receiver
        .receive(frames[2].clone())
        .expect_err("chunk without restarted init fails");
    assert!(matches!(
        error,
        omega_network::snapshot::SnapshotReceiveError::Protocol(
            SnapshotProtocolError::NoActiveSnapshot
        )
    ));
}
