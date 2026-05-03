# Experiments-Repo Publication Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish two locally-built artifacts (`omega-commitment/` Rust workspace + `cardano-wiki/` LLM-maintained research wiki) into the user's existing public `CharlesHoskinson/experiments` GitHub repo, with a comprehensive top-level README + ARCHITECTURE doc that frames the work and articulates all program goals (PQ migration, ZK continuity to all prior Cardano eras, lazy resurrection, plonky3-friendly tree, etc.).

**Architecture:** Flat-copy both source trees into the experiments repo as sibling top-level directories (no submodules — keeps clone simple and self-contained). Strip source `.git/` and build artifacts. Three new docs at the repo root — `README.md`, `ARCHITECTURE.md`, `GOALS.md`. Per-subdir `README.md` for each component. Single initial commit, sole attribution to `charles hoskinson <charles.hoskinson@gmail.com>` (override the workstation's `@iohk.io` global), push to `origin/master`.

**Tech Stack:** git, gh CLI, GitHub-flavored markdown.

---

## File Structure

```
experiments/                                          (cloned from github.com/CharlesHoskinson/experiments)
├── README.md                                         (NEW: top-level frame, nav, status)
├── ARCHITECTURE.md                                   (NEW: 7-sub-tree design + dual-track + ingestion)
├── GOALS.md                                          (NEW: program-level goals + tracks T1-T12 status)
├── LICENSE                                           (PRESERVE: existing if any; else add Apache-2.0)
├── omega-commitment/                                 (FLAT COPY from /home/hoskinson/omega-commitment)
│   ├── README.md                                     (UPDATE existing — point at parent ARCHITECTURE)
│   ├── Cargo.toml
│   ├── crates/
│   │   ├── omega-commitment-core/
│   │   ├── omega-commitment-cli/
│   │   ├── omega-commitment-bundle/
│   │   ├── omega-commitment-ingest/
│   │   └── omega-utxo-snapshot/                      (NEW workspace member)
│   ├── scripts/
│   ├── tests/
│   └── docs/
└── cardano-wiki/                                     (FLAT COPY from /home/hoskinson/cardano-wiki)
    ├── README.md                                     (NEW: tells you what the wiki is + how to read it)
    ├── SCHEMA.md
    ├── wiki/
    │   ├── index.md
    │   ├── log.md
    │   ├── overview.md
    │   └── pages/
    └── docs/
        ├── superpowers/
        │   ├── specs/
        │   └── plans/
        └── codex_briefings/
```

**Excluded from copy:**
- `omega-commitment/.git/`, `omega-commitment/target/`, `omega-commitment/Cargo.lock` (let consumers regenerate)
- `cardano-wiki/.git/`, `cardano-wiki/raw/` (raw sources may be large + redundant)
- Any `.env`, `.envrc`, `*.key`, `secrets/` — none expected; verify
- Mithril snapshots, ledger DB, `/home/hoskinson/cardano/` — never in scope

---

## Pre-flight verified

- Destination: `https://github.com/CharlesHoskinson/experiments` (public, created 2026-05-03, currently empty)
- gh CLI auth: ✅ logged in as CharlesHoskinson
- Git workstation default: `charles hoskinson <charles.hoskinson@iohk.io>` — must be overridden per-commit to `<charles.hoskinson@gmail.com>` per user instruction
- omega-commitment has uncommitted edits (Cargo.toml, README, scripts/dump_ledger_state.sh, scripts/setup_headless_node.md, new omega-utxo-snapshot/ + examples/) — all get included in the drop
- cardano-wiki has uncommitted edits (codex briefing, v1.0 plan, log.md, two new wiki pages) — all get included
- `Cargo.lock` is intentionally excluded so consumers get fresh resolution against current pallas / serde versions

---

## Task 1: Set up the local working tree

**Files:**
- Create: `/home/hoskinson/experiments/` (clone target)

- [ ] **Step 1: Clone the empty experiments repo**

```bash
cd /home/hoskinson
gh repo clone CharlesHoskinson/experiments
```

Expected: clones to `/home/hoskinson/experiments` (empty repo; clone may print "warning: You appear to have cloned an empty repository.")

- [ ] **Step 2: Configure git author for THIS repo only**

```bash
cd /home/hoskinson/experiments
git config user.name "charles hoskinson"
git config user.email "charles.hoskinson@gmail.com"
git config --get user.name
git config --get user.email
```

Expected: `charles hoskinson` then `charles.hoskinson@gmail.com`. This is local repo config — the global `@iohk.io` is unaffected outside this directory.

- [ ] **Step 3: Verify branch + remote**

```bash
git branch -a
git remote -v
```

Expected: empty (no commits yet) + `origin https://github.com/CharlesHoskinson/experiments.git`. If branch shows `master` good; if `main`, note it for the push step.

---

## Task 2: Copy the source trees

**Files:**
- Create: `/home/hoskinson/experiments/omega-commitment/` (full copy minus excludes)
- Create: `/home/hoskinson/experiments/cardano-wiki/` (full copy minus excludes)

- [ ] **Step 1: Copy omega-commitment (excluding .git, target, Cargo.lock)**

```bash
cd /home/hoskinson/experiments
rsync -av --exclude='.git' --exclude='target' --exclude='Cargo.lock' \
  /home/hoskinson/omega-commitment/ ./omega-commitment/
ls -la omega-commitment/ | head
du -sh omega-commitment/
```

Expected: directory created, no `.git` inside, no `target` directory (Cargo build artifacts), no `Cargo.lock`. Size should be a few MB (source only).

- [ ] **Step 2: Copy cardano-wiki (excluding .git and raw)**

```bash
cd /home/hoskinson/experiments
rsync -av --exclude='.git' --exclude='raw' \
  /home/hoskinson/cardano-wiki/ ./cardano-wiki/
ls -la cardano-wiki/ | head
du -sh cardano-wiki/
```

Expected: directory created, no `.git` inside, no `raw/` directory (the original ingest sources). Size should be a few hundred KB.

- [ ] **Step 3: Sanity-check no secrets came along**

```bash
cd /home/hoskinson/experiments
grep -RIn -E "BEGIN (RSA|OPENSSH|EC|PGP) PRIVATE KEY" . | head -5
grep -RIn -E "(api[_-]?key|secret|password|token)\s*[:=]" --include='*.json' --include='*.toml' --include='*.env*' . | head -5
find . -name '.env*' -not -path '*/node_modules/*'
```

Expected: no matches. If anything appears, REMOVE before continuing.

---

## Task 3: Write the top-level README.md

**Files:**
- Create: `/home/hoskinson/experiments/README.md`

- [ ] **Step 1: Write README.md**

Create `/home/hoskinson/experiments/README.md` with this content:

````markdown
# Charles Hoskinson — Experiments

Working space for in-progress research and prototypes. The two pieces currently
landed here belong to a single program: **Ouroboros Omega**, a clean-slate
post-quantum fork design for Cardano with cryptographic continuity to every
prior era of the chain.

| Subdirectory | What it is | Status |
|---|---|---|
| [`omega-commitment/`](./omega-commitment/) | Rust workspace producing the Ω-Commitment — a single hash that captures the entire pre-fork Cardano state across 7 sub-trees | v0.9.1 (89 commits, 248 tests) |
| [`cardano-wiki/`](./cardano-wiki/) | LLM-maintained research wiki — Cardano consensus, EUTXO, Plutus, Hydra, Mithril, Leios, Voltaire governance, plus the Omega program design and v1.0 ingestion plans | Living document |

## What is Ouroboros Omega?

A complete protocol redesign for Cardano predicated on three convictions:

1. **Quantum-attack resistance is a deadline-bound problem, not a research one.** The post-quantum migration must happen before large fault-tolerant quantum computers exist, not after. Ed25519 (signatures), Praos VRF (consensus), KES (forging), and BLS12-381 (Mithril) are all curve-based and breakable in the relevant timeline.
2. **A clean-slate fork beats incremental migration.** Bolting PQ onto a chain whose every primitive assumes elliptic curves accumulates compatibility tax forever. Building Omega as its own chain and providing a one-way bridge from Cardano keeps both designs honest.
3. **Holders must be able to claim what they had on the old chain, but the old chain's state must not be re-executed.** A succinct ZK proof of "this UTxO existed at the snapshot height" lets a user resurrect their funds (or their staked position, or their governance role) on Omega without forcing Omega's validators to re-derive 8+ years of consensus history.

The Ω-Commitment is what enables (3): a single root hash committing to seven aspects of the pre-fork state (UTxOs, block headers, transaction index, native token policies, scripts, stake state, governance state). With the root pinned in Omega's genesis block, any pre-fork user can build a Merkle membership proof against their old position and "lazy-resurrect" it on the new chain.

[`ARCHITECTURE.md`](./ARCHITECTURE.md) is the deep-dive on how that hash is constructed, why it's structured the way it is, and what the v1.0 mainnet-ingestion pipeline actually does.
[`GOALS.md`](./GOALS.md) is the program-level goal map — the 12 tracks of which the commitment-tooling work in this repo is just T1.

## How to read this repo

- **You want the executable code:** `omega-commitment/` is a self-contained Rust workspace. `cd omega-commitment && cargo test --workspace` runs all 248 tests.
- **You want the design rationale and decisions log:** `cardano-wiki/wiki/` is the living research wiki; start with `wiki/index.md` and `wiki/pages/spec-ouroboros-omega.md`.
- **You want the v1.0 mainnet-ingestion plan and recent discoveries:** `cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md` (the "REVISION 2026-05-03" section is current).
- **You want the codebase audit-handoff briefings:** `cardano-wiki/docs/codex_briefings/`.

## Status as of 2026-05-03

| Layer | State |
|---|---|
| Synthetic-fixture ingestion (5 of 7 sub-trees) | ✅ Shipped v0.9.1 |
| Headless mainnet cardano-node (Mithril-bootstrapped) | ✅ Synced epoch 628, slot 186M |
| `omega-utxo-snapshot` LSQ client (UTxO sub-tree input) | ✅ Built; smoke-test against live mainnet in flight |
| Real-mainnet ingestion (5 sub-trees) | ⏳ v1.0 — implementing parsers next |
| Chain-follower for header + tx-index sub-trees | 📅 v1.1 — planned |

## License

Apache-2.0. See `omega-commitment/Cargo.toml` for the workspace license metadata.
````

- [ ] **Step 2: Verify no broken links**

```bash
cd /home/hoskinson/experiments
grep -oE '\[.*?\]\(\./[^)]+\)' README.md | while read link; do
  path=$(echo "$link" | sed -E 's/.*\(\.\/(.*)\)/\1/')
  test -e "$path" && echo "OK: $path" || echo "MISSING: $path"
done
```

Expected: every `OK:` line, no `MISSING:` lines (except `LICENSE` if not present).

---

## Task 4: Write the top-level ARCHITECTURE.md

**Files:**
- Create: `/home/hoskinson/experiments/ARCHITECTURE.md`

- [ ] **Step 1: Write ARCHITECTURE.md**

Create `/home/hoskinson/experiments/ARCHITECTURE.md` with this content:

````markdown
# Architecture

This document explains the design of the Ω-Commitment — what it captures, how it's structured, and how the v1.0 mainnet-ingestion pipeline actually produces one.

## Cryptographic primitives

All primitives are post-quantum. **No curve operations** appear anywhere in the construction.

- **Hashing:** Blake2b-256 (per-leaf, per-sub-tree-root) and SHA3-256 (parallel bundle-root track). The dual hash track at the bundle layer is a hedge against a single-hash break.
- **Tree:** binary, fixed-arity Merkle tree with deterministic leaf ordering and zero-padding to the next power of two. The tree is plonky3-friendly: every operation is expressible as a STARK constraint without curve gadgets.

The ZK continuity proof — "this UTxO existed at the snapshot height" — is intended to be discharged by a plonky3 circuit that opens a Merkle path against the published Ω-Commitment root. The circuit doesn't know about Cardano-era consensus; it only knows about the published commitment and the user's position.

## The 7 sub-trees

| # | Sub-tree | What it captures | Leaf encoding |
|---|---|---|---|
| 1 | UTXO | Every unspent output: tx_id, output_index, address, value (lovelace + native tokens), datum hash, optional script hash | 88+ bytes (variable for native tokens) |
| 2 | Header chain | Every block header: slot, block_height, block_hash, prev_hash | 80 bytes (locked from v0.2.0) |
| 3 | Transaction index | Every transaction: tx_id, slot, block_hash, position-within-block | 76 bytes (locked from v0.3.0) |
| 4 | Native token policies | Each policy: policy_id, policy_script_hash, total minted, first issuance slot | 56 bytes |
| 5 | Script registry | Each on-chain script: script_hash, script_type (Native / PlutusV1 / V2 / V3), deployment slot | 33 bytes |
| 6 | Stake state | Each stake credential: credential, delegated pool, DRep delegation, controlled stake, rewards balance | 96 bytes |
| 7 | Governance state | Each governance fact: kind (DRep / committee / proposal / vote / treasury), payload hash | 65 bytes |

Each sub-tree is built independently. Within a sub-tree, leaves are sorted by their first field (tx_id, slot, policy_id, etc.) for determinism.

## The dual-track bundle root

```
   Per-sub-tree roots (Blake2b-only)
         │
         ▼
  ┌──────────────────────────────────────────────┐
  │   bundle_root_blake2b = blake2b(             │
  │     utxo_root || header_root || tx_root ||   │
  │     token_root || script_root || stake_root  │
  │     || gov_root)                             │
  │                                              │
  │   bundle_root_sha3 = sha3(same concatenation)│
  └──────────────────────────────────────────────┘
         │
         ▼
   Ω-Commitment = (bundle_root_blake2b, bundle_root_sha3)
```

Per-leaf and per-sub-tree-root computations are Blake2b only (faster, plonky3-friendlier). The dual-hash hedge applies only at the bundle layer, where the cost is two hashes of a 224-byte concatenation. If one hash function falls, the other still anchors the commitment.

## Lazy / pull-based resurrection

The post-Omega ledger does NOT pre-load 8 years of Cardano state. Instead, every pre-fork holder pulls their state forward when they need it, by submitting a `claim_*` transaction to Omega:

| Claim type | Proves | Resurrects |
|---|---|---|
| `claim_utxo` | UTxO existed at snapshot | ADA + native tokens at the same address |
| `claim_token_policy` | Policy was minted on Cardano | Same policy ID can mint on Omega |
| `claim_script` | Script was registered on Cardano | Same script hash callable on Omega |
| `claim_stake` | Credential was delegated to pool X | Migrate stake position to Omega's analogous pool |
| `claim_governance` | Held DRep / committee role | Same role on Omega's governance |
| `claim_header` | Block existed at slot S | (Reserved — used by chain-anchored protocols only) |
| `claim_tx` | Transaction existed | (Reserved — used by tx-anchored protocols only) |

Each claim transaction includes a Merkle path against the Ω-Commitment, verified by the Omega ledger's plonky3 verifier. State that is never claimed is never resurrected — the Cardano UTxO set's long tail of dust addresses doesn't pay for itself in storage cost.

## v1.0 ingestion pipeline (what produces the Ω-Commitment)

The Ω-Commitment is computed once at the snapshot boundary. Producing it requires real Cardano mainnet data, ingested via a two-stream pipeline. (The single-LedgerState-CBOR plan was abandoned 2026-05-03 — see `cardano-wiki/wiki/log.md` and `cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`.)

### Source: headless cardano-node + Mithril fast-bootstrap

A standard `cardano-node 10.7.1` with `mithril-client 2617.0`'s fast-bootstrap (~218 GB snapshot, 2 hour download) produces a fully-synced mainnet node. Runbook: `omega-commitment/scripts/setup_headless_node.md`.

### Stream 1: stake + governance (single JSON dump)

```bash
cardano-cli conway query ledger-state --mainnet --out-file ledger_state_<TS>.json
```

Produces ~2 GiB of JSON. The cardano-cli scrubs `utxoState.utxo` to `{}` on mainnet (this is intentional — the `--whole-utxo` query path is documented testnet-only and broken by an upstream Word16-VLE TxIx decoder bug; PR cardano-cli#1350 carries the hotfix but is unmerged), but everything else is intact:

- 1.47M stake accounts, 2,940 stake pools, 1,016 DReps, 2.50M stake credentials, 3 snapshots × 1.32M activeStake, full governance state (proposals, committee, constitution, treasury, reserves)

Both stake (Task 7) and governance (Task 8) parsers read this single file. Path map verified live: `cardano-wiki/wiki/pages/ledger-state-json-layout.md`.

### Stream 2: UTXO + token-policy + script (custom LSQ client)

```bash
cargo run --release -p omega-utxo-snapshot -- \
  --socket ~/cardano/socket/node.socket --network mainnet --era 6 \
  --out utxo_<TS>.cbor
```

Bypasses the broken cardano-cli path. Issues `Request::LedgerQuery(LedgerQuery::BlockQuery(6, BlockQuery::GetUTxOWhole))` via pallas-network 0.30.2's local-state-query miniprotocol. Pallas's CBOR decoder doesn't share Haskell's TxIx asymmetry. Output is bit-identical to what a fixed cardano-cli would have written with `--output-cbor-bin`. UTXO (Task 4) reads this; token-policy (Task 5) and script (Task 6) derive from the same UTXO walk.

Wire-format match against ouroboros-consensus verified layer-by-layer 2026-05-03: `cardano-wiki/wiki/pages/lsq-getutxowhole-pipeline.md`.

## Tracks beyond commitment-tooling

The 7-sub-tree commitment is **track T1** of a 12-track program. The other 11 tracks are scoped in `cardano-wiki/wiki/pages/spec-ouroboros-omega.md` and summarized in [`GOALS.md`](./GOALS.md):

- T2 Consensus (PQ Praos)
- T3 Smart-contract VM
- T4 Network stack
- T5 Storage / state-management
- T6 ZK verifier (plonky3 integration)
- T7 Bridge protocol (Cardano → Omega claim verifier)
- T8 Tooling + cli
- T9 Documentation + spec
- T10 Audits + formal verification
- T11 Test-network operations
- T12 Mainnet operations

This repository is the work for T1 only. The wiki captures the in-progress design discussion across all tracks.
````

---

## Task 5: Write the top-level GOALS.md

**Files:**
- Create: `/home/hoskinson/experiments/GOALS.md`

- [ ] **Step 1: Write GOALS.md**

Create `/home/hoskinson/experiments/GOALS.md` with this content:

````markdown
# Program Goals — Ouroboros Omega

This document is the single-source-of-truth for what the program is trying to accomplish, organized by horizon.

## Why

Cardano shipped in 2017 against an elliptic-curve cryptographic stack. The post-quantum threat has firm timelines now (NIST PQC standards finalized; first wave of fault-tolerant quantum computers projected within 10–15 years). Migrating Cardano in place is possible but accumulates compatibility tax forever — every transition between curve and lattice/hash-based primitives must be coordinated, every wallet updated, every dApp rebuilt.

A clean-slate fork lets the new chain be designed for the new world from genesis. A Merkle commitment to the old state, anchored in the new chain's genesis block, lets every existing holder migrate their position via a one-shot ZK proof — without the new chain's validators re-executing 8+ years of Cardano consensus history.

## What

A new chain ("Omega") with these design properties:

1. **All primitives post-quantum.** No curve signatures. No curve VRF. No curve mining. No BLS multisigs.
2. **Plonky3-friendly state model.** Every per-block state transition expressible as STARK constraints without specialized gadgets, so verification can move off-chain when needed (light clients, rollups).
3. **One-way bridge from Cardano.** Each pre-fork holder can submit a `claim_*` transaction with a ZK Merkle proof against the Ω-Commitment to resurrect their UTxOs, stake position, governance role, etc.
4. **Lazy state migration.** Omega's genesis ledger is empty except for the Ω-Commitment root. State is pulled forward by holders, not pushed forward by validators. The unclaimed long-tail of dust addresses costs nothing.
5. **Selective dual-track at the bundle layer.** The Ω-Commitment is a `(blake2b_root, sha3_root)` tuple. If one hash function falls, the other still anchors continuity.

## Tracks

| # | Track | Scope | Status |
|---|---|---|---|
| **T1** | **Commitment tooling** | **Build the Ω-Commitment from real mainnet data; produce regression-tested per-sub-tree roots and the dual-hash bundle root** | **In progress (this repo, v0.9.1)** |
| T2 | Consensus | PQ Praos design + reference implementation | Spec drafting |
| T3 | Smart-contract VM | Plutus-equivalent, plonky3-native execution model | Spec drafting |
| T4 | Network stack | Ouroboros networking miniprotocols, PQ-handshake variants | Not started |
| T5 | Storage / state management | UTxO storage + block storage adapted to plonky3 friendliness | Not started |
| T6 | ZK verifier | Plonky3 integration; on-chain verifier for claim proofs | Not started |
| T7 | Bridge protocol | Claim-transaction format, replay-attack resistance, fee model | Spec drafting |
| T8 | Tooling + cli | Wallet primitives, claim helpers, devtools | Not started |
| T9 | Documentation + spec | Whitepaper, formal protocol spec | T1 spec only |
| T10 | Audits + formal verification | Third-party audit, machine-checked proofs of critical invariants | Not started |
| T11 | Test-network operations | Devnet → testnet → public testnet | Not started |
| T12 | Mainnet operations | Genesis ceremony, key rollout, validator onboarding | Not started |

## T1 (this repo) — sub-goals

### v0.x.0 series — synthetic ingestion (DONE)

Build the commitment data structure end-to-end against hand-crafted CBOR fixtures. Prove the leaf-encoding → sub-tree-root → bundle-root pipeline computes deterministic, regression-detectable roots for each of the 5 LedgerState-derivable sub-trees. v0.9.1 ships this with 248 tests + pinned golden vectors at three layers.

### v1.0 — real mainnet ingestion for 5 sub-trees (IN PROGRESS)

Replace the synthetic fixtures with real mainnet data via the two-stream pipeline (`omega-utxo-snapshot` for utxo/token-policy/script; `cardano-cli query ledger-state` JSON for stake/governance). Pin the resulting "5 real + 2 placeholder" bundle root tuple as a real-data golden vector at a chosen epoch boundary.

### v1.1 — chain-follower for the remaining 2 sub-trees

Implement a pallas-network N2C chain-sync miniprotocol client that walks every block from genesis (or a Mithril snapshot) to a target tip, emitting per-block header rows and per-tx tx-index rows in NDJSON. Postprocess into the per-sub-tree input format. Capture the COMPLETE 7-of-7 mainnet bundle root tuple at the same epoch boundary as v1.0's anchor — replacing the v1.0 "5 real + 2 placeholder" intermediate result.

### v2.0 — formal-verification-grade reproducibility

A second independent implementation (probably Haskell or Lean) computes the same Ω-Commitment from the same mainnet snapshot, byte-for-byte. Cross-implementation agreement is the audit precondition for using the commitment in Omega's genesis block.

## Non-goals

- **Reproducing Cardano's full state on Omega.** The chain-state is the source-of-truth; Omega only commits to it.
- **Verifying Cardano consensus from Omega.** Omega trusts the snapshot. The snapshot's correctness is established by Mithril certificates + the cross-implementation reproducibility check.
- **Live cross-chain interoperability.** This is a one-way migration bridge, not a two-way bridge. After migration, Cardano and Omega are independent chains.
- **Backwards compatibility with Cardano addresses, scripts, or transaction formats on Omega.** Omega has its own primitives. The bridge is the only continuity layer.

## How decisions are recorded

All material design decisions are written into the wiki at `cardano-wiki/wiki/log.md` (append-only timeline) and turned into pages under `cardano-wiki/wiki/pages/` when they need standalone reference. Implementation plans and codex audit briefings live under `cardano-wiki/docs/`.
````

---

## Task 6: Update or write the omega-commitment subdir README

**Files:**
- Modify (if existing) or Create: `/home/hoskinson/experiments/omega-commitment/README.md`

- [ ] **Step 1: Inspect existing README**

```bash
cd /home/hoskinson/experiments/omega-commitment
head -40 README.md
wc -l README.md
```

Expected: there's an existing README (~750 lines per the source). It's the workspace-internal README; we keep it but add a header pointing back to the parent ARCHITECTURE.

- [ ] **Step 2: Prepend a parent-frame header to the existing README**

Use Edit to add this immediately after the H1 (the very first line that starts with `# `):

```markdown
> **Parent frame:** this is the workspace-level README for the **omega-commitment** crate. The program-level README is at [`../README.md`](../README.md), the architecture deep-dive at [`../ARCHITECTURE.md`](../ARCHITECTURE.md), and the program goals at [`../GOALS.md`](../GOALS.md).
```

(Find the H1 line, prepend the blockquote immediately after it via `sed -i '/^# /a\\n> **Parent frame:** ...' README.md` or via the Edit tool with the H1 as the anchor.)

---

## Task 7: Write the cardano-wiki subdir README

**Files:**
- Create: `/home/hoskinson/experiments/cardano-wiki/README.md`

- [ ] **Step 1: Write README.md**

Create `/home/hoskinson/experiments/cardano-wiki/README.md` with this content:

````markdown
# cardano-wiki

> **Parent frame:** this is a subdir of [`../`](../) (the experiments repo). The program-level README is at [`../README.md`](../README.md), the architecture deep-dive at [`../ARCHITECTURE.md`](../ARCHITECTURE.md), and the program goals at [`../GOALS.md`](../GOALS.md).

An LLM-maintained research wiki on Cardano. Two functions:

1. **Domain reference** — pages on Ouroboros consensus, EUTXO, Plutus, Hydra, Mithril, Leios, CIP-1694 governance, Voltaire, Plomin hard fork, Intersect MBO, repos, and key ecosystem orgs. These are evolving syntheses, not snapshots.

2. **Program scratch space for Ouroboros Omega (T1, the work in `../omega-commitment/`)** — design specs, implementation plans, codex audit briefings, decision log, and discovery pages produced as the work proceeds.

## How to read

- **Start here:** [`wiki/index.md`](wiki/index.md) — categorized table of contents
- **Living synthesis:** [`wiki/overview.md`](wiki/overview.md)
- **Decision log (append-only timeline):** [`wiki/log.md`](wiki/log.md)
- **Pages (flat namespace, hyphen-slugged):** [`wiki/pages/`](wiki/pages/)
- **Implementation plans:** [`docs/superpowers/plans/`](docs/superpowers/plans/)
- **Codex audit-handoff briefings:** [`docs/codex_briefings/`](docs/codex_briefings/)

## Most important pages right now

| Page | What it documents |
|---|---|
| [`wiki/pages/spec-ouroboros-omega.md`](wiki/pages/spec-ouroboros-omega.md) | The Omega program design spec |
| [`wiki/pages/ledger-state-json-layout.md`](wiki/pages/ledger-state-json-layout.md) | Verified JSON paths for stake + governance ingestion |
| [`wiki/pages/lsq-getutxowhole-pipeline.md`](wiki/pages/lsq-getutxowhole-pipeline.md) | Why the cardano-cli `--whole-utxo` path doesn't work + what we built instead |
| [`docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md`](docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md) | The v1.0 plan (read the "REVISION 2026-05-03" section first) |
| [`docs/codex_briefings/2026-05-03-omega-codex-pipeline-update-brief.md`](docs/codex_briefings/2026-05-03-omega-codex-pipeline-update-brief.md) | Latest audit-handoff brief for cross-LLM review |

## Conventions

- **Frontmatter on every page** with `title`, `slug`, `tags`, `sources`, `confidence` (low / medium / high), `provenance`, `created`, `updated`. See [`SCHEMA.md`](SCHEMA.md).
- **Bidirectional links** via `[[slug]]` syntax — added by the wiki-ingest workflow whenever a new page is written.
- **No subdirectories** under `wiki/pages/` — flat namespace, slug-uniqueness enforces clarity.
- **Source provenance** — each page lists what external sources informed it; raw source files were pruned from this snapshot.

## License

Apache-2.0 unless individual source files indicate otherwise.
````

---

## Task 8: Initial commit + push

**Files:**
- All of `/home/hoskinson/experiments/`

- [ ] **Step 1: Stage everything**

```bash
cd /home/hoskinson/experiments
git status
git add .
git status
```

Expected: `git status` first shows untracked everything; after `git add .`, shows everything staged. Verify no `.git` directories or `target/` dirs leaked into the staging.

- [ ] **Step 2: Verify per-commit author override is in effect**

```bash
git config --get user.name
git config --get user.email
```

Expected: `charles hoskinson` and `charles.hoskinson@gmail.com`. If wrong, redo Task 1 step 2.

- [ ] **Step 3: Create the initial commit (sole attribution, NO Claude trailer)**

```bash
cd /home/hoskinson/experiments
git commit -m "Initial drop: omega-commitment v0.9.1 + cardano-wiki

omega-commitment/
  Rust workspace producing the Omega-Commitment for the Ouroboros Omega
  program (clean-slate post-quantum fork of Cardano with ZK continuity).
  Five of seven sub-trees implemented end-to-end against synthetic CBOR
  fixtures. 248 passing tests; pinned golden vectors at three layers.
  Includes the new omega-utxo-snapshot binary that bypasses the broken
  cardano-cli --whole-utxo path via a pallas-network LSQ client.

cardano-wiki/
  LLM-maintained research wiki on Cardano (consensus, EUTXO, Plutus,
  Hydra, Mithril, Leios, CIP-1694 / Voltaire governance, Intersect MBO,
  ecosystem repos) plus the Omega program design, the v1.0 mainnet
  ingestion plan with the 2026-05-03 architecture revision, and the
  per-stream verification pages for stake/governance JSON paths and
  the LSQ GetUTxOWhole pipeline.

See README.md, ARCHITECTURE.md, GOALS.md at the repo root for the
program-level frame."
git log -1 --pretty=full
```

Expected: `git log -1` shows a single commit signed `charles hoskinson <charles.hoskinson@gmail.com>`. NO `Co-Authored-By: Claude` line.

- [ ] **Step 4: Push to origin**

```bash
cd /home/hoskinson/experiments
git push -u origin HEAD
```

Expected: pushes `master` (or `main`, whichever the empty repo defaulted to) to origin. If the remote default branch is different from local, the push prints a hint and you may need `git push -u origin HEAD:main` to align.

- [ ] **Step 5: Verify on GitHub**

```bash
gh repo view CharlesHoskinson/experiments --web 2>/dev/null || gh repo view CharlesHoskinson/experiments
gh api repos/CharlesHoskinson/experiments/commits --jq '.[0] | "\(.sha[0:8]) \(.commit.author.name) <\(.commit.author.email)> \(.commit.message | split("\n") | .[0])"'
```

Expected: the latest commit appears, authored by `charles hoskinson <charles.hoskinson@gmail.com>`, with the "Initial drop:" message.

---

## Task 9: Post-push smoke checks

**Files:** none modified

- [ ] **Step 1: Confirm READMEs render on github**

```bash
gh api repos/CharlesHoskinson/experiments/contents/README.md --jq '.size, .download_url'
gh api repos/CharlesHoskinson/experiments/contents/ARCHITECTURE.md --jq '.size, .download_url'
gh api repos/CharlesHoskinson/experiments/contents/GOALS.md --jq '.size, .download_url'
gh api repos/CharlesHoskinson/experiments/contents/omega-commitment/README.md --jq '.size'
gh api repos/CharlesHoskinson/experiments/contents/cardano-wiki/README.md --jq '.size'
```

Expected: every call returns a positive size. If any 404s, the file didn't push (re-stage and recommit).

- [ ] **Step 2: Confirm no large files leaked**

```bash
gh api repos/CharlesHoskinson/experiments/git/trees/HEAD?recursive=true --jq '.tree[] | select(.size > 1000000) | "\(.size)  \(.path)"' | sort -rn | head -10
```

Expected: nothing > 1 MB. If anything appears (Cargo.lock, target/ artifacts, ledger snapshots), `git rm` and recommit.

- [ ] **Step 3: Confirm the destination URL prints clean**

```bash
echo "Repo URL: https://github.com/CharlesHoskinson/experiments"
echo "Latest commit: $(gh api repos/CharlesHoskinson/experiments/commits --jq '.[0].html_url')"
```

This is the artifact to share or pin.

---

## Self-Review

**Spec coverage:**
- (a) Top-level README — Task 3 ✅
- (b) ARCHITECTURE.md (7 sub-trees, dual-track, two-stream ingestion) — Task 4 ✅
- (c) Per-subdir READMEs (omega-commitment + cardano-wiki) — Tasks 6 + 7 ✅
- All program goals (PQ migration, ZK continuity, lazy resurrection, plonky3-friendly tree, dual-hash hedge) — Task 4 (architecture) + Task 5 (goals) ✅
- Sole attribution (no Claude trailer) — Task 1 step 2 (per-repo author override) + Task 8 step 3 (commit message has no Co-Authored-By) ✅

**Placeholder scan:** no TBD / TODO / "implement later" anywhere in the embedded code blocks. Every `cat <<EOF` has its full content. Every commit message and every README is fully written.

**Type consistency:** the docs reference each other via relative paths (`./omega-commitment/`, `./ARCHITECTURE.md`, etc.) and link target consistency was verified by Task 3 step 2. The `[[slug]]` wiki-link convention is preserved verbatim from the source wiki — those resolve within the wiki, not at the repo root, and that's correct.

---

## Execution

Auto mode is on and the user has explicitly authorized the push. Execute inline using superpowers:executing-plans (batched-with-checkpoints), no need for the subagent-per-task review loop given the doc-only nature.
