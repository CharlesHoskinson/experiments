# LoganNet 3-node Raft + libp2p test patterns

LoganNet is a local 3-node openraft + libp2p + rusqlite cluster (see `c:/experiments/README.md`). When LoganNet ships, these are the canonical Turmoil fixtures.

## 3-node fixture

```rust
fn three_node_loganet() -> turmoil::Sim<'static> {
    let mut sim = turmoil::Builder::new().build();
    sim.host("node1", || async {
        omega_toy_consensus::run("node1", &["node2", "node3"]).await
    });
    sim.host("node2", || async {
        omega_toy_consensus::run("node2", &["node1", "node3"]).await
    });
    sim.host("node3", || async {
        omega_toy_consensus::run("node3", &["node1", "node2"]).await
    });
    sim
}
```

## Test: single leader emerges

```rust
#[turmoil::test]
async fn single_leader_emerges() {
    let mut sim = three_node_loganet();
    sim.elapse(Duration::from_secs(2));

    let leaders: Vec<_> = ["node1", "node2", "node3"]
        .iter()
        .filter(|n| sim.host(n).is_leader())
        .collect();

    assert_eq!(leaders.len(), 1, "exactly one leader");
}
```

## Test: partition tolerance

```rust
#[turmoil::test]
async fn partitioned_minority_does_not_commit() {
    let mut sim = three_node_loganet();
    sim.elapse(Duration::from_secs(2));

    // Isolate node1 from the other two.
    sim.partition("node1", "node2");
    sim.partition("node1", "node3");

    // Node 1, now in a 1-node minority, must not commit.
    let result = sim.host("node1").submit_claim(dummy_claim());
    sim.elapse(Duration::from_secs(5));
    assert!(result.await.is_err(), "minority partition must not commit");
}
```

## Test: snapshot-mid-leader-change (Adversary class)

Property: applied state on the new leader matches the snapshot's committed state. Soundness-negative case: snapshot install on follower must not silently overwrite uncommitted entries that were already replicated.
