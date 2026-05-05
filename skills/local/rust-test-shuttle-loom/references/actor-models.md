# Actor models

Patterns for testing the `omega-mock-ledger` (and future `omega-toy-consensus`) mpsc-actor designs under Shuttle.

## The WriterActor pattern

```rust
#[cfg(feature = "shuttle")]
use shuttle::sync::mpsc;
#[cfg(not(feature = "shuttle"))]
use std::sync::mpsc;

struct WriterActor {
    rx: mpsc::Receiver<Cmd>,
    db: Connection,
}

enum Cmd {
    Apply(Tx),
    Snapshot(Snapshot),
}
```

## Shuttle test for the WriterActor

```rust
#[cfg(feature = "shuttle")]
#[test]
fn writer_actor_no_deadlock() {
    shuttle::check_random(
        || {
            let (tx, rx) = shuttle::sync::mpsc::channel();
            let actor = WriterActor::new(rx, ":memory:");

            // Producer 1: apply transactions
            let tx1 = tx.clone();
            let h1 = shuttle::thread::spawn(move || {
                for i in 0..10 {
                    tx1.send(Cmd::Apply(Tx::dummy(i))).unwrap();
                }
            });

            // Producer 2: take snapshots
            let tx2 = tx.clone();
            let h2 = shuttle::thread::spawn(move || {
                for _ in 0..3 {
                    tx2.send(Cmd::Snapshot(Snapshot::default())).unwrap();
                }
            });

            // Drop the original tx so the actor exits when producers finish
            drop(tx);

            actor.run();
            h1.join().unwrap();
            h2.join().unwrap();
        },
        1000, // 1000 random schedules
    );
}
```

## Property: no message loss

```rust
#[cfg(feature = "shuttle")]
#[test]
fn writer_actor_no_message_loss() {
    shuttle::check_random(
        || {
            let (tx, rx) = shuttle::sync::mpsc::channel();
            let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let counter_clone = counter.clone();
            let actor = WriterActor::with_counter(rx, counter_clone);

            for i in 0..10 {
                tx.send(Cmd::Apply(Tx::dummy(i))).unwrap();
            }
            drop(tx);
            actor.run();

            assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 10);
        },
        1000,
    );
}
```

## Property: snapshot consistency

A snapshot taken between two applies must contain exactly one of them, never both, never neither (the apply-snapshot relation must be linearizable).
