# Network faults under Turmoil

Turmoil exposes fine-grained control over the simulated network. Use these patterns to inject faults.

## Partition

```rust
sim.partition("node1", "node2");
sim.partition("node1", "node3");
// node1 is now isolated from {node2, node3}; nodes 2 and 3 can still talk.

// Heal:
sim.repair("node1", "node2");
sim.repair("node1", "node3");
```

## Latency

```rust
sim.set_link_latency("node1", "node2", Duration::from_millis(150));
```

## Packet loss

```rust
sim.set_link_loss_rate("node1", "node2", 0.1); // 10% packets dropped
```

## Time advance (deterministic clock)

```rust
sim.elapse(Duration::from_secs(5));
```

## Crash and restart

```rust
sim.crash("node2");
sim.elapse(Duration::from_secs(1));
sim.bounce("node2"); // restart with same address; state lost unless persisted
```

## Determinism contract

Every Turmoil test takes a seed (default: deterministic). Re-running with the same seed must produce byte-identical output. If a test passes once and fails on re-run, the test is using non-deterministic state (probably `std::time` or a non-tokio runtime).
