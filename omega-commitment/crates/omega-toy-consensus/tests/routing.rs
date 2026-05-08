//! Routing translator integration tests.

use omega_mock_ledger::LedgerError;
use omega_toy_consensus::routing::{translate_client_write_error, translate_ledger_error};
use openraft::error::{ClientWriteError, ForwardToLeader};

#[test]
fn forward_to_leader_with_known_leader_emits_not_leader_with_url() {
    let err = ClientWriteError::ForwardToLeader::<u64, openraft::BasicNode>(ForwardToLeader {
        leader_id: Some(2),
        leader_node: Some(openraft::BasicNode::default()),
    });
    let obj = translate_client_write_error(err, |id| Some(format!("http://127.0.0.1:800{id}")));
    assert_eq!(obj.code(), -32000);
    let data = obj.data().unwrap();
    let v: serde_json::Value = serde_json::from_str(data.get()).unwrap();
    assert_eq!(v["leader_id"], 2);
    assert_eq!(v["leader_rpc_url"], "http://127.0.0.1:8002");
}

#[test]
fn forward_to_leader_unknown_leader_emits_not_leader_without_url() {
    let err = ClientWriteError::ForwardToLeader::<u64, openraft::BasicNode>(ForwardToLeader {
        leader_id: None,
        leader_node: None,
    });
    let obj = translate_client_write_error(err, |_| None);
    assert_eq!(obj.code(), -32000);
    let data = obj.data().unwrap();
    let v: serde_json::Value = serde_json::from_str(data.get()).unwrap();
    assert!(v["leader_id"].is_null());
    assert!(v["leader_rpc_url"].is_null());
}

#[test]
fn ledger_replay_emits_neg_32003_with_hint() {
    let err = LedgerError::Replay {
        sub_tree_id: 1,
        leaf_index: 42,
    };
    let obj = translate_ledger_error(err);
    assert_eq!(obj.code(), -32003);
    let data = obj.data().unwrap();
    let v: serde_json::Value = serde_json::from_str(data.get()).unwrap();
    assert_eq!(v["sub_tree_id"], 1);
    assert_eq!(v["leaf_index"], 42);
}

#[test]
fn ledger_writer_closed_is_transient() {
    let obj = translate_ledger_error(LedgerError::WriterClosed);
    assert_eq!(obj.code(), -32004);
    let data = obj.data().unwrap();
    let v: serde_json::Value = serde_json::from_str(data.get()).unwrap();
    assert_eq!(v["retryable"], true);
}
