# `stake_snapshot.cbor`

Hand-crafted CBOR fixture with 4 stake-state entries.

## Per-entry format

```
[ stake_credential_hash (28 bytes),
  delegated_pool (28 bytes),
  delegated_drep (CBOR array, length 1 or 2),
  rewards_lovelace (u64),
  is_pool_operator (u8) ]
```

`delegated_drep` is a Conway-era tagged enum (CIP-1694):

| Tag | Meaning              | Wire shape                        |
|-----|----------------------|-----------------------------------|
| 0   | None / no DRep       | `[0]`                             |
| 1   | KeyHash DRep         | `[1, 28-byte payload]`            |
| 2   | ScriptHash DRep      | `[2, 28-byte payload]`            |
| 3   | AlwaysAbstain        | `[3]`                             |
| 4   | AlwaysNoConfidence   | `[4]`                             |

All-zero pool means undelegated.

## Contents

| # | credential | pool          | drep                  | rewards     | is_pool_operator |
|---|------------|---------------|-----------------------|-------------|------------------|
| 0 | `1111…11`  | zero          | None                  | 0           | 0                |
| 1 | `2222…22`  | pool_a (0xAA) | None                  | 1_000_000   | 0                |
| 2 | `3333…33`  | pool_b (0xBB) | KeyHash(drep_a 0xCC)  | 5_000_000   | 0                |
| 3 | `4444…44`  | pool_a        | None                  | 100_000_000 | 1                |

Covers: undelegated / pool-only / pool+key-hash DRep / pool-operator.

## Regeneration

Re-pinned 2026-05-03 as part of Batch 2: Cardano semantic fidelity
(A3/F003). The fixture is hand-crafted CBOR; to re-emit it after a
schema change, build a small generator that mirrors the per-entry
shape above (a Python helper using `cbor_array`, `cbor_bytes`,
`cbor_uint` is sufficient).
