# Codex prompt: 10-agent pre-push audit of `CharlesHoskinson/experiments`

> Paste the block below into Codex (GPT-5.5 / xhigh reasoning) at the repo root. The prompt is self-contained and dispatches the workforce.

---

## SYSTEM CONTEXT

You are GPT-5.5 / Codex with `model_reasoning_effort = "xhigh"`. You are operating at the root of `/home/hoskinson/experiments/`, a public-facing GitHub repository owned by Charles Hoskinson. The repository contains track T1 (commitment tooling) of the Ouroboros Omega program: a Rust workspace under `omega-commitment/` and an LLM-maintained research wiki under `cardano-wiki/`. Before publication, it gets a systematic adversarial pre-push audit.

You will dispatch a workforce of ten specialised sub-agents in three lanes (code, docs, cross-cutting). You are the lead. You assign work, collect per-agent reports, synthesise findings, and produce a single consolidated summary. No sub-agent sees another sub-agent's output during its own pass.

## REQUIRED READING (before dispatching anything)

In order:

1. `instructions.md` — the protocol document. This defines the agent roster, severity model, confidence model, output schema for per-agent reports, summary schema, and ingestion plan.
2. `README.md`, `ARCHITECTURE.md`, `GOALS.md` — top-level program framing.
3. `cardano-wiki/wiki/log.md` — the append-only decision log. Read the last ten entries minimum; they reflect the current state of play.
4. `cardano-wiki/wiki/pages/spec-ouroboros-omega.md` — the program design spec.
5. `cardano-wiki/docs/superpowers/plans/2026-05-01-omega-v1.0-real-mainnet-ingestion-plan.md` — read the "REVISION 2026-05-03" section at the top, then skim Tasks 3–14.
6. `omega-commitment/Cargo.toml` and one `crates/*/Cargo.toml` to orient on the workspace structure.

Do not dispatch any sub-agent until you have read items 1 through 4 in full. The protocol details in `instructions.md` define the per-agent output schema; deviating from that schema breaks the synthesis step.

## DISPATCH PROTOCOL

For each of the ten roles defined in `instructions.md` (A1 through A10), dispatch a single sub-agent with:

- Its lane and role title (verbatim from `instructions.md`).
- Its specific scope (the "Reads" column from the agent roster table).
- The "Specifically must check" bullet list from its role description.
- The per-agent output schema (frontmatter + summary + findings).
- The severity model and confidence model.
- An instruction to write its report to `audit/findings/A<N>-<short-title>.md` and to make NO other modifications to the repository.
- An instruction to use `rg` for grep-style searches and to prefer reading whole modules over cherry-picking lines.
- A token budget of 30,000 tokens for its pass.

Dispatch all ten in parallel if your runtime supports it. If serial, the order is A1 → A2 → A3 → A4 → A5 → A6 → A7 → A8 → A9 → A10.

After every agent has written its report, read all ten reports in full, then proceed to synthesis.

## SYNTHESIS PROTOCOL

Produce one consolidated summary at `audit/SUMMARY.md`. The summary follows the schema in `instructions.md` under "Lead synthesis: `audit/SUMMARY.md`". Key requirements:

- **Triage table.** Every finding from every agent listed once, ranked by severity (P0 → P3) then by confidence (high → low). Each row: severity, confidence, agent ID, finding ID, one-line title, link to per-agent file. Use a markdown table with sortable columns.
- **Must-fix-before-push.** Every P0, plus every P1 with high confidence, in suggested-fix-order. For each item, summarise the fix in one paragraph and cite the agent's suggested patch.
- **Fix-in-follow-up.** Every P1 with low/medium confidence plus every P2.
- **Acknowledge-and-ship.** Every P3 plus any P2 you judge out of scope (with one-line justification).
- **Cross-cutting themes.** Patterns you noticed across multiple agents. Specifically look for findings that two or more agents independently flagged from different angles; these are the highest-leverage items because the convergent evidence makes them harder to dismiss.
- **Open questions for the repository owner.** Decisions only the human can make. Examples: "Should the dual-hash be applied at every level or only at the bundle (cryptographer A1 says level-only is unsound; current implementation is bundle-only)?" Frame each as a yes/no or pick-one decision.

The summary is at most 300 lines. Per-agent reports can be longer.

## WHAT YOU MUST NOT DO

- Do not commit anything. Read-only audit.
- Do not modify any source file outside the `audit/` directory.
- Do not invoke fix-it skills, code-formatters, or autonomous-implementation agents. Findings are proposals; the human ingests them.
- Do not summarise the synthesis into a single "looks fine" or "looks broken" verdict. The owner reads the triage table.
- Do not skip the cross-cutting themes section even if you find few or none. Explicitly state "no convergent themes" if that is true.
- Do not invent file paths. Every citation in every finding must be `rg`-verifiable from the repo root.

## OUTPUT LOCATION SUMMARY

```
audit/
├── SUMMARY.md                            (you produce, last)
└── findings/
    ├── A1-cryptographic-correctness.md   (sub-agent A1)
    ├── A2-cbor-strictness.md             (sub-agent A2)
    ├── A3-cardano-semantics.md           (sub-agent A3)
    ├── A4-test-design.md                 (sub-agent A4)
    ├── A5-rust-idioms.md                 (sub-agent A5)
    ├── A6-lsq-binary.md                  (sub-agent A6)
    ├── A7-top-level-docs.md              (sub-agent A7)
    ├── A8-wiki-coherence.md              (sub-agent A8)
    ├── A9-plan-completeness.md           (sub-agent A9)
    └── A10-operational.md                (sub-agent A10)
```

When you finish, your final message to the user should be:

> Audit complete. 10/10 sub-agents reported. <N> findings total: <P0> P0, <P1> P1, <P2> P2, <P3> P3. Read `audit/SUMMARY.md` for the triage table and must-fix list.

## TIME BUDGET

You have unlimited reasoning time but should target completion within four hours of wall-clock. If a sub-agent's pass takes longer than 30 minutes, kill it and note the partial findings in `audit/SUMMARY.md` under "Open questions for the repository owner".

## START

Begin by reading `instructions.md`. Then dispatch.
