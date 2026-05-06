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

Final M1 run: **17,281 valid nodes, 3,845 unique states, 17.8s, PASSED on all invariants** at `max_actions=7`.

| Property class | Property | Status |
|---|---|---|
| Safety | `NoConflictingFinalised` | PASSED (vacuous — see depth caveat below) |
| Safety | `PrefixConsistency` | PASSED (vacuous) |
| Safety | `HonestVoteConsistency` | PASSED (non-vacuous; honest vote guards exercised) |
| Safety | `QuorumSignerDistinct` | PASSED (vacuous; no certs form at depth 7) |
| Safety | `LeaderBarringRespected` | PASSED (placeholder — relies on dynamic guard in `CastElectionVote`) |
| Safety | `FinalityJustifiedByThreeChain` | PASSED (vacuous) |
| Safety | `ForgedCertRejected` | PASSED (Byzantine forge action is a no-op; structural guard) |
| Liveness | `EventuallyFinalise` | deferred to follow-up plan (see "Deferred work" below) |

### Depth caveat (M1 honest reporting)

At the M1 bound `max_actions=7`, election scaffolding consumes most of the action budget (StartElection + 3× CastElectionVote + InstallLeaderCertificate + AdvanceTerm ≈ 6 actions per term rotation). The model never accumulates three notarization certificates in any reachable state, so `FinalizeThreeChain` does not fire. This means:

- The headline safety property `NoConflictingFinalised` holds **structurally** (the action+invariant code is correct) but is exercised only over the trivial `chain.finalized = {GENESIS}` set.
- The Byzantine surface (`ByzantineEquivocate`, `ByzantineDoubleVote`, `TryForgeCertificate`) is **encoded** but its guards never fire at depth 7 because validator 0 (the sole Byzantine node under the symmetry reduction) is never elected leader within the action budget.
- `HonestVoteConsistency` IS non-vacuous: honest votes do fire and the slot-deduplication guard is checked across all reachable states.

Lifting the depth caveat requires one of:

1. Larger `max_actions` (currently OOMs WSL above ~10).
2. Pre-seeding `chain.leader_for_term` and `chain.certs` in `Init` to start a scenario closer to a three-chain candidate.
3. Symmetry annotations in the v0.4.0-correct API (the plan's `symmetry.nominal([...])` syntax does not match v0.4.0; see `raftlet.fizz` comment block).

These are the natural targets for an M1.5 / M2 refinement.

### v0.4.0 limitations encountered (worth knowing for follow-up work)

| Limitation | Workaround used |
|---|---|
| Top-level `atomic func` not callable from role actions | inline helper bodies |
| Tuple-unpack in for-loops not supported (`for k, v in d.items():`) | iterate keys, look up explicitly |
| Cross-instance role mutation not supported (`v.term = x` from another action) | each validator self-updates via `AdvanceTerm` |
| Records inside shared role sets occasionally raise "unhashable type" | `chain.notar_votes` and `chain.election_votes` are LISTS (allow duplicates) |
| `symmetry.nominal([...string...])` rejected in v0.4.0 | symmetry deferred; documented inline in `.fizz` |
| `byz: oneof [True, False]` per-validator + cross-instance reads cause state explosion | hardcoded `byz_nodes = set([0])` (sound under validator-ID symmetry for f=1) |

## Deferred work (post-M1)

Liveness modelling is deferred to a follow-up plan. The intended properties are:

- `EventuallyFinalise`: under fair scheduling, bounded faults, and eventually-delivered messages, finality advances.
- `BatchClockProgress`: under `BatchClock.ClientBatch`, election rounds eventually complete when batches are available.

Reasons for deferring:

1. Liveness checks require weak-fair scheduling annotations on `ProposeBlock`, `CastNotarizationVote`, `FormNotarizationCertificate`, `FinalizeThreeChain`, `StartElection`, `CastElectionVote`, `InstallLeaderCertificate`, and `AdvanceTerm`. Adding these is mechanical but expands the state space.
2. Liveness is more sensitive to bound choices than safety — a too-tight `MAX_TERM` produces vacuous PASS results. M1's depth-7 already produces vacuous safety PASSES; liveness would amplify the issue.
3. The Raftlet paper's liveness argument depends on dissemination and synchrony assumptions that need to be modelled explicitly (raftlet.md line 430). Doing this well requires its own design pass.
4. The depth caveat above must be lifted first — liveness over a model that doesn't even reach a three-chain is not informative.

The follow-up plan should land at `docs/superpowers/plans/<date>-raftlet-fizzbee-liveness-spec.md` after the depth caveat is addressed.
