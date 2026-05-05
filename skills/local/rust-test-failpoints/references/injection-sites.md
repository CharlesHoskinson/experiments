# Injection sites

Where to plant `fail::fail_point!` calls in `c:\experiments\omega-commitment\` (and the future LoganNet crates).

## Production-code points (gated under cfg(feature = "failpoints"))

| Site | Point name | Failure modes |
|---|---|---|
| `omega-mock-ledger::WriterActor::apply` | `wal_fsync_fail` | `return(ENOSPC)`, `return(EIO)`, `panic!` |
| `omega-mock-ledger::Snapshot::write` | `snapshot_write_fail` | `return(ENOSPC)`, partial-write |
| `omega-mock-ledger::Snapshot::read` | `snapshot_read_truncated` | `return(EOF)` mid-stream |
| `omega-toy-consensus::libp2p_send` (planned) | `peer_send_fail` | `return(ConnectionReset)`, `delay(100ms)` |
| `omega-toy-consensus::raft_apply` (planned) | `apply_fail` | `return(StateError)`, `delay(50ms)` |
| `omega-claim-verifier::verify_proof` | `verify_busy` | `return(BusyTryAgain)` (transient) |

## Test-side activation

```rust
#[cfg(feature = "failpoints")]
#[test]
fn wal_fsync_failure_does_not_corrupt_state() {
    let _guard = fail::FailScenario::setup();
    fail::cfg("wal_fsync_fail", "return(ENOSPC)").unwrap();

    let actor = WriterActor::new_in_memory();
    let result = actor.apply(Tx::dummy(0));

    assert!(result.is_err(), "fsync failure must surface as error");
    // Adversary class: state must NOT show the apply as committed.
    assert_eq!(actor.committed_count(), 0, "partial apply must not commit");
}
```

## Adversary-class invariants

For each failpoint, name the soundness-negative invariant:

| Failpoint | Adversary invariant |
|---|---|
| `wal_fsync_fail` | Partial write must NOT show as committed |
| `snapshot_write_fail` | Partial snapshot must NOT be advertised as complete |
| `snapshot_read_truncated` | Reader must NOT silently return partial data |
| `peer_send_fail` | Sender must NOT report success on failed send |
| `verify_busy` | BusyTryAgain must NEVER be confused with verification success |

These invariants populate the orchestrator's Soundness-negative table.
