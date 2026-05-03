---
date: 2026-05-03
kind: design-doc
topic: README expansion — LoganNet (mock currency / local sim ledger) + Goblins (agentic framework)
status: drafted
---

# Design: README expansion — LoganNet + Goblins sections

## Why

The repo just gained two pieces that the README does not yet name:

1. **LoganNet** — the local 3-node Raft cluster spun up by `omega-toy-consensus` + `omega-mock-ledger` + `omega-api` (designed in the `add-proof-experiment-harness` OpenSpec change). The mock currency on this cluster is **LGN**, the unit of value carried by emitted Starstream UTxOs after a claim applies. LoganNet is the local sim ledger; LGN is the unit on the local sim ledger. Neither claims any relationship to real Cardano, real Omega mainnet, or real money.

2. **Goblins** — the parameterizable agentic framework (Gemma-4 E4B via Ollama; designed in the `add-goblin-agentic-framework` OpenSpec change). Six default roles role-play on LoganNet to produce realistic load mixes for benchmarking and to surface harness regressions. We have hero artwork ready.

Outside readers landing on the README today see Ouroboros Omega framed as a clean-slate PQ Cardano redesign and a working v0.10 commitment workspace. They have no anchor for what "running it" actually looks like. LoganNet and Goblins together give them that anchor: a name for the test harness, a name for the unit, a picture of who plays on it.

## Decisions

### Naming

- **LoganNet** is the local sim ledger. Always capitalised, no space, no hyphen.
- **LGN** is the unit. Three letters, all caps, no symbol.
- LGN has **zero** monetary claim. It is local, synthetic, and never bridges anywhere. The README must say so explicitly (one sentence) so nobody mistakes it for a token launch.
- The four-layer Omega architecture, the seven sub-trees, the Ω-Commitment, the C1-C8 verifier — all unchanged. LoganNet is *where* the prototype runs; the protocol design is unchanged.

### New sections in `README.md`, in order

After the existing "What the Plonky3 verifier proves" section, insert two top-level sections, then the existing "How to read this repo" continues unchanged:

1. **`## LoganNet — the local simulation ledger`**
2. **`## Goblins — the agentic load mix`**

### Section 1 content (`## LoganNet`)

Four paragraphs plus one ASCII topology diagram.

- **¶1 Framing**: what LoganNet is, what LGN is, what neither of them is. Two-sentence disclaimer that LGN has no value and never bridges to real Cardano.
- **¶2 Topology**: 3 openraft nodes on `127.0.0.1:{4001,4002,4003}` (libp2p) and `:{8001,8002,8003}` (omega-api HTTP). One rusqlite database per node (WAL mode, mpsc-actor writer, no separate mutex). Gossipsub dropped from v0.1; Raft `AppendEntries` is authoritative. Cluster is not production — it is a developer-laptop quorum.
- **¶3 Genesis + LGN semantics**: synthetic Ω-Commitment pinned at startup as a JSON file via the omega-commitment-core Tree::build_v1 path; Blake3 + v2 domain tags everywhere; LGN is the `value` field on emitted Starstream UTxOs after a `claim_utxo` applies. Multi-claim folding is v0.2; v0.1 is one claim per submission.
- **¶4 How to run a proof experiment**: ≤ 8 copy-paste shell commands. `cargo build --workspace`, generate synthetic genesis fixture, start three nodes in three terminals, run `omega-experiment prove`, run `omega-experiment submit`, run `omega-experiment state`. Cross-link to `cardano-wiki/wiki/pages/omega-testnet-e2e-plan.md` and `openspec/changes/add-proof-experiment-harness/`.
- **ASCII diagram**: 3-node topology with libp2p ports, HTTP ports, SQLite paths, omega-experiment client at the top.

### Section 2 content (`## Goblins`)

Three paragraphs + hero image + one role table + one cultural note.

- **Hero image** at top of section: `![Six goblin roles](assets/goblins-hero.png)`. Alt text describes all six roles for screen readers.
- **¶1 Framing**: goblins are autonomous agents that role-play on LoganNet to produce realistic load mixes and surface harness regressions. N is parameterizable per role. They use Gemma-4 E4B locally via Ollama for planning; CI uses a deterministic `MockLlmClient` so the framework runs without GPU. Goblins are simulation tools, not part of the protocol.
- **Role table** (6 rows): role | one-line behaviour | the failure mode it surfaces.
  - Holder — single-leaf claim_utxo — surfaces happy path bugs.
  - Whale — batched ClaimCollection of K leaves — surfaces batched-prove memory + latency.
  - Adversary — replay / tampered proof / malformed CBOR — surfaces verifier regressions (panics the runner if accepted).
  - Lurker — passive observer with LLM-generated summaries — gives human readers a skim of a 30-min run.
  - SnapshotServer — hosts a libp2p protocol serving Merkle paths — stand-in for the future T5 mirror partnerchain.
  - Validator — runs outside the cluster, requests controlled disruptions (pause node, sever network, force snapshot) — surfaces consensus brittleness.
- **¶2 How to run a goblin simulation**: `ollama pull gemma4:e4b`, `omega-goblins run --holders 5 --adversaries 2 --lurkers 1 --duration 30s --llm http://127.0.0.1:11434`, observe metrics at `:9090/metrics`. Mock-LLM mode for CI: `--llm mock`. Cross-link to `openspec/changes/add-goblin-agentic-framework/`.
- **¶3 Cultural note**: the Adversary panics the runner if it gets *accepted* — that's a feature, not a bug, because Adversaries are how we surface harness regressions. The Validator runs *outside* the Raft cluster via a sidecar admin channel and never impersonates a Raft node. The Lurker writes plain-English summaries every M ticks so a human reading the log can skim what happened.

### Edits to existing README content

- **Artifact table at top**: keep the four rows (`omega-commitment/`, `cardano-wiki/`, `audit/`, `skills/`). Do NOT add a "LoganNet" row — LoganNet is the *running* shape of `omega-commitment/`, not a separate subdirectory. The new sections introduce the name in prose.
- **Status table** under "Status as of 2026-05-03": add two rows. One for "LoganNet local cluster prototype" tracking the harness OpenSpec change. One for "Goblin agentic framework" tracking the goblin OpenSpec change. Both rows reference the openspec change directories.
- **No other edits** to the four-layer architecture description, the C1-C8 verifier table, the transaction-flow phases, the Tracks T1-T12 list, or the To-do section. Those are correct as-is.

### Tone + humanizer pass

- Same voice as the existing README: terse, technically dense, no AI-tells. The Goblins section is allowed slightly more personality (the artwork sets the tone) but does NOT overshoot into "fun cute toy chain" framing that would obscure the technical contract.
- Final humanizer pass on the new prose to scrub: "vibrant", "showcase", "delve", "tapestry", "pivotal", em-dash overuse, rule-of-three patterns, inline-header bullet bolding, sycophantic openers.

### Image asset

- Source: `/c/Users/charl/OneDrive/Desktop/10c9e4ca-663f-4452-8462-0b213dc3c9d9.png` (provided by user this turn).
- Destination: `assets/goblins-hero.png`. Already copied as of this turn.
- Embed via standard markdown `![alt](assets/goblins-hero.png)`. No CDN, no external host. Repo is public so GitHub serves the image inline.
- Image is roughly 2.8 MB; acceptable. If GitHub-rendered README ever feels heavy we can add a smaller `goblins-hero-small.png` for the README and keep the hi-res in `assets/` for the Grafana dashboard / wiki.

## What this is not

- Not a token launch. LGN has no monetary claim, never will, the README says so explicitly.
- Not a redesign of Omega. The four-layer PQ stack, the seven sub-trees, the Ω-Commitment construction are unchanged.
- Not a separate workspace. LoganNet is the running shape of the existing `omega-commitment/` workspace plus the four new harness crates from `add-proof-experiment-harness` and the three from `add-goblin-agentic-framework`.
- Not a multi-machine deployment. Three ports on one developer box.

## Acceptance

- README has two new top-level sections: `## LoganNet` and `## Goblins`.
- LoganNet section contains the topology diagram, the LGN definition, the no-monetary-value disclaimer, the run-a-proof-experiment command sequence, and cross-links.
- Goblins section embeds `assets/goblins-hero.png`, has the 6-row role table, the run-a-goblin-simulation command sequence, and cross-links.
- Status table gains two rows for LoganNet + Goblins tracking.
- Existing sections (artifact table, four-layer architecture, C1-C8 verifier, transaction-flow phases, Tracks T1-T12, To-do, License) are unchanged.
- humanizer pass run on the new prose.
- README size grows by ~120-180 lines (current 483 → ~620). Acceptable.
- Wiki log entry recording the README expansion + image addition.
