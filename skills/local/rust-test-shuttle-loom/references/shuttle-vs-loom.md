# Shuttle vs Loom — when to pick which

Both explore thread schedules. Different strengths.

## Loom

- **Exhaustive within a small bound.** Explores every interleaving up to `LOOM_MAX_PREEMPTIONS` preemption points.
- **Catches small subtle bugs.** If a bug exists in any schedule of the bounded model, Loom finds it.
- **Slow on large models.** State space grows exponentially.
- **Replaces `std::sync::*` types.** Code under test must use `loom::sync::*`.

## Shuttle

- **Randomised exploration.** Samples schedules; doesn't guarantee finding all bugs.
- **Fast on large models.** Scales to actors with many channels and threads.
- **Less invasive.** Often works by replacing only the test harness, not the code under test.
- **Better for liveness-shaped properties** (eventually-X) than Loom.

## Decision rule

| Code shape | Tool |
|---|---|
| Lock-free queue, RCU, atomic state machine | Loom |
| mpsc actor with N producers | Shuttle |
| Mutex graph with potential deadlock | Shuttle |
| Single atomic + bounded loop | Loom |
| Async-await futures coordination | Shuttle (Loom doesn't model async cleanly) |
| Anything > 5 threads or > 100 LoC under test | Shuttle |
| Anything ≤ 3 threads and < 50 LoC | Loom |

## When to use both

For a critical concurrent data structure, write Shuttle tests for breadth + Loom tests for depth on the smallest possible kernel. omega-commitment doesn't have any such structure today.
