# Fuzz target design

## The target signature

Every cargo-fuzz target has this shape:

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Call the function under test. It must NOT panic on any input.
    // Panics are how libFuzzer detects bugs.
    let _ = your_decoder(data);
});
```

## Structured input via `arbitrary`

For decoders with structured input (e.g., a `(version: u8, payload: &[u8])` pair), use the `arbitrary` crate:

```rust
use arbitrary::Arbitrary;

#[derive(Arbitrary, Debug)]
struct Input {
    version: u8,
    payload: Vec<u8>,
}

fuzz_target!(|input: Input| {
    let _ = your_decoder(input.version, &input.payload);
});
```

## Adversary-class target (verifier shape)

For code whose contract is "must reject malformed input":

```rust
fuzz_target!(|data: &[u8]| {
    let parse_ok = is_well_formed(data);   // cheap structural check
    let verify_ok = verify_claim(data).is_ok();
    if verify_ok && !parse_ok {
        panic!("ADVERSARY: verifier accepted malformed input: {:02x?}", data);
    }
});
```

The Adversary panic is what gets surfaced in the orchestrator's P0_REGRESSION report.

## Targets to land for omega-commitment

| Target | Function under test | Crate |
|---|---|---|
| `lsq_cbor_decode` | `pallas-network LSQ response decoder` | `omega-utxo-snapshot` |
| `ledger_state_json_parse` | `serde_json::from_reader<LedgerState>` wrapper | `omega-commitment-ingest` |
| `header_ndjson_row` | header NDJSON line parser | `omega-commitment-ingest` (when v1.1 lands) |
| `omega_leaf_decode` | `Leaf::decode` for each sub-tree | `omega-commitment-core` |
| `claim_tx_decode` | `ClaimTx::decode` | `omega-claim-tx` |
| `verifier_adversary` | Adversary-shape on `verify_claim` | `omega-claim-verifier` |
