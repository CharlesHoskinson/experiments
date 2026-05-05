# Trigger suite

The 25 prompts that should activate `rust-test-orchestrator` and the 10 that should not. Use this as the manual test suite for T1.

## Should trigger (25)

1. "Write tests for `apply_transaction`"
2. "I need to test the LSQ decoder"
3. "Add coverage for the verifier"
4. "Help me test this module"
5. "How should I test `WriterActor`?"
6. "Test the LoganNet Raft cluster"
7. "Add property tests for leaf encoding"
8. "I want to fuzz the CBOR parser"
9. "Concurrency tests for the mpsc actor"
10. "Soundness tests for the verifier"
11. "Test that the verifier rejects malformed claims"
12. "Add a soundness-negative test for the nullifier set"
13. "Model-check the consensus protocol"
14. "Test the Raft leader election"
15. "Test the partition tolerance of LoganNet"
16. "Make sure no two leaders are ever elected"
17. "Add fuzz coverage to the JSON parser"
18. "Test the snapshot installation under leader change"
19. "Add proptest for the round-trip CBOR encoding"
20. "Test that snapshot reads handle truncation"
21. "Verify the leaf hash is collision-resistant under bounded inputs"
22. "Failure-inject the WAL fsync"
23. "Add Adversary-class tests"
24. "Test the actor under random schedules"
25. "Write tests covering the verifier circuit's public input handling"

## Should NOT trigger (10)

1. "Configure CI"
2. "Fix this clippy warning"
3. "What does cargo nextest do?"
4. "Set up coverage reporting"
5. "Rename a function"
6. "Explain this code"
7. "Add a benchmark with criterion"
8. "How do I install Kani?"
9. "Update Cargo.toml dependencies"
10. "Write documentation for this module"

## Manual T1 procedure

1. Open a fresh Claude Code session in `c:\experiments\`.
2. Paste each "should trigger" prompt; observe whether the orchestrator activates (look for "rust-test-orchestrator" being invoked).
3. Paste each "should NOT trigger" prompt; observe that the orchestrator does NOT activate.
4. Record results in `references/iteration-log.md` as a one-line entry per failure.

T1 passes when 25/25 positive triggers fire and 10/10 negative triggers do not.
