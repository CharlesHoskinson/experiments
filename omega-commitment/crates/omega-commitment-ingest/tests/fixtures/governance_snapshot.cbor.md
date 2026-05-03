# `governance_snapshot.cbor`

Hand-crafted CBOR fixture with 5 governance facts, one per `kind`.

## Per-fact format

```
# kind 0..=3 (treasury, CC seat, ratified action, in-flight action):
[ kind (u8), key (32 bytes), value (16-byte big-endian u128), slot (u64) ]

# kind 4 (AccountState):
[ kind (u8), reserves (u64), treasury (u64), deposits (u64), fee_pot (u64) ]
```

Note: u128 values for the legacy four kinds come over CBOR as 16-byte
bytestrings (CBOR has no native u128). The fixture uses
`value.to_be_bytes()` for serialization and the ingestion parser uses
`read_u128_bytes` for decoding. The AccountState pots are plain CBOR
unsigned integers.

## Contents

| # | kind             | payload                                                                |
|---|------------------|------------------------------------------------------------------------|
| 0 | 0 treasury       | key=zero, value=1_700_000_000_000_000 lovelace, slot=100_000           |
| 1 | 1 CC seat        | key=`CCCC…CCCC`, value=5_500 (expiration epoch), slot=100_000          |
| 2 | 2 ratified       | key=`4444…4444`, value=packed(type=1, slot_ratified), slot=100_000     |
| 3 | 3 in-flight      | key=`6666…6666`, value=packed(type=2, slot_submitted), slot=100_000    |
| 4 | 4 AccountState   | reserves=13e15, treasury=1.7e15, deposits=5e10, fee_pot=250_000        |

Covers all five kind discriminants. AccountState is the new Conway-era
fact added in Batch 2 (A3/F004).

## Regeneration

Re-pinned 2026-05-03 as part of Batch 2: Cardano semantic fidelity
(A3/F004). The fixture is hand-crafted CBOR; to re-emit it after a
schema change, build a small generator that mirrors the per-entry
shape above (a Python helper using `cbor_array`, `cbor_bytes`,
`cbor_uint` is sufficient).
