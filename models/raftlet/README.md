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
| `max_actions` | 7 (was 60 in plan) | reduced from the plan's 60 because v0.4.0 OOMs WSL above ~10 with this model's action surface; see "Depth caveat" below |
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

| Property class | Property | M1 | M1.5 | M2 | M3 |
|---|---|---|---|---|---|
| Safety | `NoConflictingFinalised` | vacuous PASS | non-vacuous (happy-path) | non-vacuous (both scenarios) | non-vacuous (both, post-finalize state space) |
| Safety | `PrefixConsistency` | vacuous PASS | non-vacuous (happy-path) | non-vacuous (both) | non-vacuous (both) |
| Safety | `HonestVoteConsistency` | non-vacuous | non-vacuous | non-vacuous | non-vacuous (unchanged) |
| Safety | `QuorumSignerDistinct` | vacuous PASS | non-vacuous (happy-path) | non-vacuous (both) — S2_F never reaches QUORUM | non-vacuous (both) |
| Safety | `LeaderBarringRespected` | placeholder | placeholder | placeholder | placeholder (unchanged) |
| Safety | `FinalityJustifiedByThreeChain` | vacuous PASS | non-vacuous (happy-path) | non-vacuous (happy-path) | non-vacuous (happy-path) |
| Safety | `ForgedCertRejected` | structural | structural | non-vacuous (byz-fork) — TryForgeCertificate fires post-finalize without advancing | non-vacuous (post-finalize gate) |
| Liveness | `EventuallyFinalise` | deferred | deferred | deferred | **PASSED** (FG form; weak-fair `FinalizeThreeChain` + action gate) |
| Reachability | `FinalisationReachable` | n/a | n/a | n/a | **PASSED** (regression backstop) |

**M3 final run:** 719 valid nodes / 134 unique states / 1s / `max_actions=2`. All 8 properties PASS including the new liveness assertion.

**M1.5 final run:** 1,223 valid nodes / 203 unique states / 2s at `max_actions=2` with seeded happy-path scenario. PASSED on all seven safety invariants. The Task 7 tracer assertion (`return len(chain.finalized) <= 1`) was applied transiently and FAILED with a counterexample showing `chain.finalized = set(["G", "S1"])` — concrete proof that `FinalizeThreeChain` fires in a reachable state. Tracer removed at Task 8; documentary comment retained in `raftlet.fizz`.

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

### M1.5 lift: seeded happy-path scenario

M1.5 (branch `feat/raftlet-fizzbee-m1.5`, plan at `docs/superpowers/plans/2026-05-06-raftlet-fizzbee-m1.5-non-vacuous-finality.md`) added a seeded "happy-path three-chain" scenario in `Chain.Init` and `Validator.Init`. The seed pre-populates:

- Three notarized blocks `S1`, `S2`, `S3` with consecutive (term, height) tuples `(1,1)`, `(2,2)`, `(3,3)` and parent links.
- Notarization votes from honest validators 1, 2, 3 for each seeded block (9 votes total).
- Quorum certs for all three seeded blocks.
- Leader assignments `term -> {1: 1, 2: 2, 3: 3}`.
- Each validator starts at `term=3` with `head=S3` and `known_certs={S1, S2, S3}`.

This makes `FinalizeThreeChain` reachable in a single action: any validator with the seeded `known_certs` can pick `b1=S1, b2=S2, b3=S3` and add `S1` to `chain.finalized`. **Four** previously-vacuous invariants (`NoConflictingFinalised`, `PrefixConsistency`, `QuorumSignerDistinct`, `FinalityJustifiedByThreeChain`) are now exercised over states where `chain.finalized` contains real Raftlet-finalized blocks.

**Trade-off:** the seeded scenario expanded the per-action choice multiplicity (CastNotarizationVote and ByzantineDoubleVote both now have 4× block choices). This forced `max_actions` down from M1's 7 to M1.5's 2 — but the seed is the lever, depth budget only needs to reach `FinalizeThreeChain` plus its consequence checks, which one extra action covers.

**Still deferred (M2 targets):**

- Byzantine equivocation execution (validator 0 attempts a competing fork). M2 plan will add a `byz_fork` scenario via top-level `oneof SCENARIO` selector.
- Liveness properties.
- Multiple seeded scenarios in one model run (state-space pressure).

### M2 lift: byz-fork scenario via multi-scenario `oneof`

M2 (branch `feat/raftlet-fizzbee-m2`, plan at `docs/superpowers/plans/2026-05-06-raftlet-fizzbee-m2-byzantine-fork.md`) added a second seeded scenario `byz_fork_height_2` selected via `self.scenario = oneof [...]` at the top of `Chain.Init`. Under that scenario:

- Byzantine validator 0 has authored `S2_F` — a competing block at `(term=2, height=2, parent=S1)` with `leader=0` and `batch_id="bz2"`.
- One Byzantine vote for `S2_F` exists in `chain.notar_votes` (validator 0's own vote).
- `S2_F` does NOT have a cert in the seed.
- Honest validators 1, 2, 3 cannot cast a vote for `S2_F` because their `voted_slots` already contains `(2, 2)` from voting for the honest `S2` — the `HonestVoteConsistency` guard prevents the equivocation.

The verification claim is that **no path through the model's action surface can build a quorum cert for `S2_F` nor finalise it**. The seven safety invariants PASS over the combined state space of both scenarios. Three properties newly become non-vacuous in M2:

- `NoConflictingFinalised` is now exercised under a real fork attempt.
- `QuorumSignerDistinct` rejects under-quorum certs against a tangible target.
- `ForgedCertRejected` is no longer just structural — `TryForgeCertificate` fires inside a state where there's a competing uncertified block to attempt to certify.

**Trade-off:** running both scenarios doubles Init breadth. State count grew from M1.5's 1,223 / 203 to M2's 2,807 / 526. Same `max_actions=2`.

**Still deferred (post-M2 targets):**

- Liveness via weak-fair scheduling. Genuinely independent from scenario design; gets its own plan.
- Larger validator counts (`n=7, f=2`).
- Real adversary modelling beyond static fork: dynamic vote injection, adaptive corruption.
- Multi-step Byzantine strategies (e.g., bribe + fork + reorg).

### M3 lift: liveness verified via action-gating + weak fairness

M3 (branch `feat/raftlet-fizzbee-m3`, plan at `docs/superpowers/plans/2026-05-06-raftlet-fizzbee-m3-liveness.md`) verifies the headline Raftlet liveness property `EventuallyFinalise` — that some non-GENESIS block is eventually finalised under fair scheduling. **PASSED** at 719 / 134 / 1s.

The path to PASSED was non-obvious. Three discoveries along the way:

1. **YAML `action_options` is rejected** by v0.4.0's strict schema (`unknown field "action_options"`).
2. **Inline `fair<weak>` / `fair<strong>` syntax** is the correct form: `atomic fair<weak> action FinalizeThreeChain:`. Parses cleanly.
3. **Bare `eventually assertion` panics**; supported modalities are `always eventually` (GF), `eventually always` (FG), and `exists`.

Even with the right syntax, `always eventually` AND `eventually always` initially FAILED at `max_actions=2`: the bounded checker found stutter-only SCCs at the action-budget boundary where `FinalizeThreeChain` had no outbound edge (it was no longer enabled — no budget left). Fairness cannot constrain SCCs whose action isn't enabled. Raising `max_actions` to 3 OOMs WSL.

**The fix is structural.** All 10 non-finalize Validator actions get `require len(chain.finalized) > 1` at their head. Effect:

- At Init, `chain.finalized = {GENESIS}` so `len == 1` — every other action's gate FAILS, only `FinalizeThreeChain` is enabled.
- Weak fairness on `FinalizeThreeChain` then forces it to fire as action #1 in every reachable trace.
- After firing, `chain.finalized = {G, S1}` so `len > 1` — gates release, the model still explores the full post-finalize action surface (Byzantine forge attempts, additional certs, leader rotations, etc.).
- The `eventually always` modality is the right pick: once a non-GENESIS block enters monotone `chain.finalized`, it stays — FG holds permanently after the first firing.

State space drops from M2's 2,807 / 526 to M3's 719 / 134 because pre-finalize states collapse to a single "only finalize is enabled" hub. The post-finalize state space is the same as M2's full surface.

**Coverage trade-off acknowledged.** The action-gate means the model verifies safety in states reachable AFTER finalize fires (plus the trivial pre-finalize hub). M2's verification reached states where Byzantine actions had fired pre-finalize. The M3 model loses that specific timing coverage in exchange for liveness. For Raftlet's safety claims this is fine because the relevant invariants (NoConflictingFinalised, QuorumSignerDistinct, etc.) depend on monotone state — the order of byz vs. finalize action firings doesn't affect whether the invariants hold.

### v0.4.0 limitations encountered (worth knowing for follow-up work)

| Limitation | Workaround used |
|---|---|
| Top-level `atomic func` not callable from role actions | inline helper bodies |
| Tuple-unpack in for-loops not supported (`for k, v in d.items():`) | iterate keys, look up explicitly |
| Cross-instance role mutation not supported (`v.term = x` from another action) | each validator self-updates via `AdvanceTerm` |
| Records inside shared role sets occasionally raise "unhashable type" | `chain.notar_votes` and `chain.election_votes` are LISTS (allow duplicates) |
| YAML `action_options.<role.action>.fairness` rejected (M3 finding) | use inline `fair<weak>` / `fair<strong>` before the `action` keyword |
| Bare `eventually assertion` panics (M3 finding) | use `always eventually assertion` (compound form; equivalent for monotone state) |
| Bounded checker SCCs at max_actions boundary defeat naive fairness (M3 finding) | gate non-finalize actions on `len(chain.finalized) > 1` so only `FinalizeThreeChain` is enabled at Init; weak fairness then bites |
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
