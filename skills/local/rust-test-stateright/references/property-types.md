# Stateright property types

Stateright supports three property kinds. Each is appropriate for different invariants.

## Safety: `Property::always(...)`

"In every reachable state, X holds."

```rust
Property::<M, _>::always("no two leaders per term", |_, state| {
    let leaders_per_term: HashMap<u64, HashSet<NodeId>> = ...;
    leaders_per_term.values().all(|s| s.len() <= 1)
})
```

Use for: invariants, no-bad-state guarantees, mutual exclusion, no-double-spend.

## Liveness: `Property::eventually(...)`

"From every reachable state, X eventually holds along every fair path."

```rust
Property::<M, _>::eventually("submitted tx eventually committed", |_, state| {
    state.applied.contains(&tx_hash)
})
```

Use for: progress guarantees, no-deadlock, eventual delivery, leader election liveness.

Requires fairness assumptions to be useful (otherwise an "always idle" path satisfies the property trivially).

## Always-eventually: `Property::sometimes(...)`

"There exists a path where X holds." (Useful as a sanity check that your model can reach interesting states at all.)

```rust
Property::<M, _>::sometimes("partition heals", |_, state| {
    state.connected_components() == 1
})
```

Use for: model sanity checks, "this scenario is reachable" assertions.

## Adversary-class properties

Soundness-negative properties for the orchestrator's report:

```rust
Property::<M, _>::always("no two leaders ever for same term (Adversary)", |_, state| {
    no_duplicate_leaders(state)
})
```

If this property fails, the orchestrator's report shows STATUS: P0_REGRESSION.
