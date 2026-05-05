# Strategies cookbook

Patterns for `proptest::strategy::Strategy` impls in `c:\experiments\omega-commitment\`.

## Sub-tree leaf generator

```rust
use proptest::prelude::*;

pub fn arb_utxo_leaf() -> impl Strategy<Value = UtxoLeaf> {
    (
        any::<[u8; 32]>(),                       // tx_id
        0u32..1024,                              // output_index (bounded for tractable shrinking)
        any::<[u8; 28]>(),                       // address_hash
        1u64..u64::MAX,                          // value (lovelace)
        prop::collection::vec(arb_asset(), 0..16), // asset bundle
    )
        .prop_map(|(tx_id, idx, addr, val, assets)| UtxoLeaf {
            tx_id,
            output_index: idx,
            address_hash: addr,
            value: val,
            assets,
        })
}

pub fn arb_asset() -> impl Strategy<Value = (PolicyId, AssetName, u64)> {
    (
        any::<[u8; 28]>(),
        prop::collection::vec(any::<u8>(), 0..32),
        1u64..u64::MAX,
    )
}
```

## Round-trip property

```rust
proptest! {
    #[test]
    fn cbor_roundtrip_utxo_leaf(leaf in arb_utxo_leaf()) {
        let bytes = minicbor::to_vec(&leaf).unwrap();
        let back: UtxoLeaf = minicbor::decode(&bytes).unwrap();
        prop_assert_eq!(leaf, back);
    }
}
```

## Soundness-negative property (rejects bad input)

```rust
proptest! {
    #[test]
    fn rejects_oversized_leaf(payload in prop::collection::vec(any::<u8>(), 4097..8192)) {
        let result = leaf_hash_v2(SUB_TREE_UTXO, 0, &payload);
        prop_assert!(result.is_err(), "oversized payload must be rejected");
    }
}
```

## Pinning the seed for regression

```rust
proptest! {
    #![proptest_config(ProptestConfig {
        cases: 4096,
        max_global_rejects: 65536,
        ..ProptestConfig::default()
    })]

    #[test]
    fn ...
}
```
