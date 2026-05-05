# Actor modeling for Crypsinous + Chronos + Minotaur

Abstract the composite Ouroboros consensus into 4 actor types. Drop crypto-level detail; keep only what's needed for protocol-level properties.

## Actor types

| Actor | Responsibility | Abstract state |
|---|---|---|
| `Leader` | Proposes blocks for its slot | `(current_slot, current_term, last_proposed)` |
| `Follower` | Votes / appends entries | `(current_slot, current_term, log_len, committed_idx)` |
| `Attestor` | Threshold-encryption committee member | `(committee_id, decrypted_payloads)` |
| `Mempool` | Holds encrypted client transactions | `(pending: Vec<EncTx>, decrypted: Vec<DecTx>)` |

## Message types

```rust
enum Msg {
    Propose { slot: u64, term: u64, block_hash: Hash },
    Vote { slot: u64, term: u64, voter: NodeId },
    AppendEntries { term: u64, prev_idx: u64, entries: Vec<Entry> },
    Decrypt { tx_hash: Hash, attestor: NodeId },
}
```

## Bounded model

```rust
const MAX_SLOTS: u64 = 5;
const MAX_NODES: usize = 4;
const MAX_PARTITIONS: usize = 2;
```

Larger bounds explode the state space.

## Sample property: single leader per slot

```rust
Property::<MyModel, _>::always("at most one leader per slot", |_, state| {
    state.network.iter()
        .filter_map(|m| if let Msg::Propose { slot, .. } = m { Some(*slot) } else { None })
        .all_unique()
})
```

## Sample property: liveness (eventually committed)

```rust
Property::<MyModel, _>::eventually("submitted tx eventually committed", |_, state| {
    state.mempool.decrypted.iter().all(|tx| state.applied.contains(&tx.hash))
})
```
