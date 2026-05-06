# Aggressively shrunk constants for fast counterexample reproduction.
# Use with:
#   ~/.claude/skills/crypto-consensus-fizzbee/scripts/check-small.sh \
#     models/raftlet/raftlet.fizz \
#     models/raftlet/preinit/shrink.py
N = 4           # cannot drop below 3f+1 = 4 without breaking the safety theorem
F = 1
QUORUM = 3
MAX_TERM = 2    # one election + one rotation
MAX_HEIGHT = 3  # smallest that allows a three-chain (heights 1, 2, 3)
MAX_BATCHES = 2
