# Report format

Phase 5 emits this exact markdown structure. Append to `c:/experiments/var/test-runs/<UTC-timestamp>.md` and post a one-line summary to chat.

## Schema

```markdown
# Test run report — <target> @ <commit-sha>
**Plan approved at:** <timestamp>
**Frameworks invoked:** <comma-separated list>
**Frameworks skipped:** <comma-separated list>
**Skip rationale per matrix:** <Q1=yes/no, Q2=yes/no, ...>

## Results
| Skill | Tests written | Passed | Failed | Time | Notes |
|---|---|---|---|---|---|
| <skill_1>    | <N>         | <p>/<n> | <f>/<n> | <t>   | <one-line note> |

## Soundness-negative tests (Adversary class)
| Case | Expected | Observed | Status |
|---|---|---|---|
| <one-line case>          | reject | reject | ✅ |
| <another case>           | reject | accept | ❌ |

## P0 alerts
<None | one bullet per P0 with offending input bytes>

## Coverage delta vs prior run
+ <new tests added>
- <regressions, if any>

STATUS: <GREEN | AMBER | P0_REGRESSION | PLAN_DRIFT>
```

## STATUS computation rules (apply in order)

1. **PLAN_DRIFT** — A soundness-negative case planned in Phase 2 is missing from the Soundness-negative table. Surface which case was dropped.
2. **P0_REGRESSION** — Any Soundness-negative row has Expected=reject and Observed=accept, OR any previously-green test from a prior run is now red.
3. **AMBER** — Non-soundness tests failed (Failed > 0 in Results table) but no soundness violation.
4. **GREEN** — All tests pass; soundness-negatives correctly reject.

## Behavior per status

- `GREEN` → print "ready to commit" + one-line summary to chat
- `AMBER` → print failures + suggest next steps; user judgment call
- `P0_REGRESSION` → print offending bytes/test name; refuse "ready to commit"; demand investigation
- `PLAN_DRIFT` → print missing case; refuse to proceed; re-run plan with the case included

## var/ storage

Reports go to `c:/experiments/var/test-runs/<UTC-timestamp>.md`. Filename format: `2026-05-04T14-23-07Z.md` (UTC, hyphens for colons).

If `c:/experiments/var/` is not in `.gitignore`, print this warning on first run:

> ⚠ `var/` is not in `.gitignore`. Add it before running again to avoid committing test reports.

Do not auto-commit reports.
