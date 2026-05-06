# Preinit shrink configs for debug runs

Python files here override top-level constants in `raftlet.fizz` for fast counterexample reproduction. Use with:

```bash
~/.claude/skills/crypto-consensus-fizzbee/scripts/check-small.sh \
  models/raftlet/raftlet.fizz \
  models/raftlet/preinit/<file>.py
```

See `~/.claude/skills/crypto-consensus-fizzbee/references/state-space-guide.md` for the full shrinking ladder.
