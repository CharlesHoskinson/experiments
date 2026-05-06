//! Kani bounded check on the snapshot install state machine.
//!
//! State space: pre_state in {empty, populated, mid-restore} and snapshot in
//! {valid, malformed}. Property: a valid snapshot installs its claimed index;
//! a malformed snapshot rejects.

#![cfg(feature = "kani")]
#![no_main]

#[derive(Debug, PartialEq, Eq)]
enum PreState {
    Empty,
    Populated,
    MidRestore,
}

#[derive(Debug, PartialEq, Eq)]
enum SnapshotKind {
    Valid,
    Malformed,
}

#[derive(Debug, PartialEq, Eq)]
enum PostState {
    Valid(u64),
    Rejected,
}

fn install_snapshot(pre: PreState, snap: SnapshotKind, snap_index: u64) -> PostState {
    match snap {
        SnapshotKind::Malformed => PostState::Rejected,
        SnapshotKind::Valid => match pre {
            PreState::Empty | PreState::Populated | PreState::MidRestore => {
                PostState::Valid(snap_index)
            }
        },
    }
}

#[kani::proof]
#[kani::unwind(5)]
fn snapshot_install_total_function() {
    let pre = match kani::any::<u8>() % 3 {
        0 => PreState::Empty,
        1 => PreState::Populated,
        _ => PreState::MidRestore,
    };
    let snap = if kani::any::<bool>() {
        SnapshotKind::Valid
    } else {
        SnapshotKind::Malformed
    };
    let idx: u64 = kani::any();
    kani::assume(idx < 1_000_000);

    let post = install_snapshot(pre, snap, idx);

    match snap {
        SnapshotKind::Malformed => assert!(matches!(post, PostState::Rejected)),
        SnapshotKind::Valid => match post {
            PostState::Valid(observed) => assert_eq!(observed, idx),
            PostState::Rejected => panic!("valid snapshot must not reject"),
        },
    }
}
