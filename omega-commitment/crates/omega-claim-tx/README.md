# omega-claim-tx

Claim transaction wire types for the local proof experiment harness.

This crate has no I/O and no async runtime dependency. It defines the Rust types for a single-leaf `ClaimTx::Utxo`, a batched `ClaimTx::Collection`, and a deterministic CBOR codec used by the later prover, verifier, ledger, network, and CLI crates.

## Wire format

`ClaimTx::to_cbor` returns one versioned CBOR envelope:

```text
[ version, payload, checksum ]
```

Fields:

| Field | CBOR type | Meaning |
|---|---|---|
| `version` | unsigned integer | Current value: `1` |
| `payload` | byte string | Canonical inner claim payload |
| `checksum` | 32-byte string | `blake3(payload)` |

`ClaimTx::from_cbor` rejects unsupported versions, checksum mismatches, trailing bytes, indefinite arrays, unknown variants, wrong fixed byte lengths, and non-canonical payload encodings. The checksum is there so a byte flip inside a proof, witness, root, or nullifier byte string is caught at the codec boundary instead of silently decoding into a different valid claim.

The inner payload is fixed-order CBOR. All arrays are definite length. All integers are unsigned. All 32-byte hashes and addresses are CBOR byte strings.

```text
ClaimTx =
  [ 0, ClaimUtxo ]
  [ 1, ClaimCollection ]

ClaimUtxo =
  [ ClaimPublicInputs, ClaimWitness, proof_bytes ]

ClaimCollection =
  [ [ClaimPublicInputs...], [ClaimWitness...], proof_bytes ]

ClaimPublicInputs =
  [ sub_tree_id, leaf_index, bundle_root_blake3, nullifier, recipient_starstream_addr ]

ClaimWitness =
  [ leaf_payload, [merkle_sibling_hash...], signing_key_proof ]
```

The collection form requires the public input count to match the witness count. v0.1 treats `proof_bytes` as opaque Plonky3 bytes; verifier semantics live in `omega-claim-verifier`.
