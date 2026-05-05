use super::*;

#[test]
fn submit_outcome_accepted_round_trip() {
    let v = SubmitOutcome {
        accepted: true,
        applied_index: Some(42),
        reject_reason: None,
    };
    let s = serde_json::to_string(&v).unwrap();
    let back: SubmitOutcome = serde_json::from_str(&s).unwrap();
    assert_eq!(v, back);
}

#[test]
fn submit_outcome_rejected_round_trip() {
    let v = SubmitOutcome {
        accepted: false,
        applied_index: None,
        reject_reason: Some("verify".into()),
    };
    let s = serde_json::to_string(&v).unwrap();
    let back: SubmitOutcome = serde_json::from_str(&s).unwrap();
    assert_eq!(v, back);
}

#[test]
fn node_state_round_trip() {
    let v = NodeState {
        node_id: 2,
        role: NodeRole::Leader,
        leader_id: Some(2),
        last_log_id: Some(LogIdView { term: 4, index: 93 }),
        applied_index: 93,
        nullifier_count: 287,
        starstream_utxo_count: 287,
    };
    let s = serde_json::to_string(&v).unwrap();
    let back: NodeState = serde_json::from_str(&s).unwrap();
    assert_eq!(v, back);
}

#[test]
fn submit_outcome_schema_compiles() {
    let _schema = schemars::schema_for!(SubmitOutcome);
}

#[test]
fn node_state_schema_compiles() {
    let _schema = schemars::schema_for!(NodeState);
}
