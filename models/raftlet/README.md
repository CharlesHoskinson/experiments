# Raftlet FizzBee model

A paper-faithful FizzBee safety model for **Raftlet**, the CS244B 2024 Byzantine-resistant Raft variant.

- Source paper: `docs/references/raftlet-a-byzantine-fault-tolerant-raft.pdf`
- Implementation plan: `raftlet.md`
- Plan executing this work: `docs/superpowers/plans/2026-05-05-raftlet-fizzbee-safety-spec.md`
- Skill used to author this model: `crypto-consensus-fizzbee` (`~/.claude/skills/crypto-consensus-fizzbee/`), playbook `playbooks/bft-quorum.md` extended with Raftlet's notarization + three-chain finality rules.
- FizzBee CLI version checked against: **v0.4.0** (release 2026-03-12; `fizz` does not expose a `--version` flag, version pinned to release tag).

## Paper-to-model matrix

| Paper concept | Model representation | Abstracted away |
|---|---|---|
| Validator (`n = 3f + 1`) | `role Validator` with `byz: bool` | identity keys, network endpoints |
| Block (term, height, parent, batch, leader, sig) | `record(term, height, parent, leader, batch_id)` | canonical encoding bytes, payload bytes |
| Proposer signature | unforgeable signer token (`leader` field of block) | RSA / SLH-DSA / hash-XMSS bytes |
| Notarization vote | `record(term, height, block_id, voter)` | wire format, transcript domain separation |
| Notarization certificate | `set` of `2f+1` distinct voter records over the same `(term, height, block_id)` | aggregate signature shape, Merkle compression |
| Three-chain finality | `FinalizeThreeChain` action: notarized blocks at `(t, h)`, `(t+1, h+1)`, `(t+2, h+2)` finalise the first | wall-clock time, persistence layer |
| Leader barring | `served_or_skipped: set` per validator | signed availability evidence bytes |
| Batch trigger | `oneof` choice of `BatchClock.ClientBatch | BatchClock.ExternalTick` | actual client request stream |
| Byzantine equivocation | `ByzantineEquivocate` action gated by `self.byz` | covert channel modelling |
| Byzantine double-vote | `ByzantineDoubleVote` action gated by `self.byz` | timing-side-channel modelling |

## Bounds

These are the M1 starting bounds. Tighten via `preinit/shrink.py` for debug; widen only when a property genuinely needs more.

| Constant | Value | Reason |
|---|---|---|
| `N` (validators) | 4 | smallest `n = 3f+1` |
| `F` | 1 | smallest meaningful Byzantine count |
| `QUORUM` | 3 | `2f+1` for `f=1` |
| `MAX_TERM` | 3 | enough to express two leader rotations |
| `MAX_HEIGHT` | 4 | enough to form a three-chain (heights 1, 2, 3) plus one extension |
| `MAX_BATCHES` | 3 | enough to feed three proposed blocks |
| `max_actions` | 60 | protocol-shaped, not toy-shaped |
| `max_concurrent_actions` | 1 | start serial; raise only if needed |

## Assumptions made explicit

These are not in the paper as numbered assumptions but are necessary for the model to be sound. Surface them so future readers know what is load-bearing.

1. **Static membership.** No view-change of the validator set. Justified: raftlet.md non-goal #4 (line 38).
2. **Unforgeable signatures.** A signer token only appears on a vote/proposal when the signer (or a corrupted-byz signer) authored it. Real signature forgery is out of scope for FizzBee.
3. **Network is fully asynchronous up to the bound.** Messages can be reordered, delayed, or dropped within `max_actions`. No partition-recovery model in M1.
4. **Batch trigger is non-deterministic.** `BatchClock.ClientBatch` is modelled as a `oneof` choice rather than a request-count counter. This preserves the protocol's behaviour without modelling client-side state.
5. **Three-chain finality is the only commit rule.** Single-QC commit is explicitly **not** allowed. This is the central Raftlet difference from PBFT/HotStuff.

## How to run

From the project root:

```bash
# Exhaustive bounded check (the headline safety check)
~/.claude/skills/crypto-consensus-fizzbee/scripts/check.sh \
  models/raftlet/raftlet.fizz

# Shrunk-bounds repro for debugging
~/.claude/skills/crypto-consensus-fizzbee/scripts/check-small.sh \
  models/raftlet/raftlet.fizz \
  models/raftlet/preinit/shrink.py

# Simulation with a few seeds (cheaper bug-finding)
~/.claude/skills/crypto-consensus-fizzbee/scripts/simulate.sh \
  models/raftlet/raftlet.fizz 42
~/.claude/skills/crypto-consensus-fizzbee/scripts/simulate.sh \
  models/raftlet/raftlet.fizz 7
```

## Counterexamples

Accepted counterexamples that mapped to Rust core requirements (per raftlet.md lines 350–360) live in `counterexamples/`. Each entry is one trace summary plus the corresponding `raftlet-core` requirement.

## Status

| Property class | Property | Status |
|---|---|---|
| Safety | `NoConflictingFinalised` | TODO (Task 10) |
| Safety | `PrefixConsistency` | TODO (Task 12) |
| Safety | `HonestVoteConsistency` | TODO (Task 13) |
| Safety | `QuorumSignerDistinct` | TODO (Task 8) |
| Safety | `LeaderBarringRespected` | TODO (Task 15) |
| Safety | `FinalityJustifiedByThreeChain` | TODO (Task 14) |
| Safety | `ForgedCertRejected` | TODO (Task 16) |
| Liveness | `EventuallyFinalise` | deferred to follow-up plan |
