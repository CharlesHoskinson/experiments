# Shrinking patterns

Proptest's value comes from shrinking — when a property fails, proptest reduces the failing input to the minimal counterexample. Write strategies that shrink well.

## Prefer ranges over arbitrary integers

```rust
// Good: shrinks toward 0
0u32..1024

// Bad: shrinks within u32 space — minimum may not be informative
any::<u32>()
```

## Prefer bounded collections

```rust
// Good: shrinks toward empty Vec
prop::collection::vec(any::<u8>(), 0..256)

// Bad: huge spaces shrink slowly
prop::collection::vec(any::<u8>(), 0..usize::MAX)
```

## Filter sparingly (high reject rates kill performance)

If your generator needs to filter, restructure to generate-valid-by-construction instead:

```rust
// Bad: rejects ~50% of inputs
any::<u32>().prop_filter("must be even", |n| n % 2 == 0)

// Good: only generates evens
(0u32..u32::MAX / 2).prop_map(|n| n * 2)
```

## Pin seeds for known-failing cases

Once proptest finds a failing input, copy it into a `proptest_attr_macro::regression!` block (or a plain `#[test]`) to lock the regression in place even if proptest changes its RNG.

## Use `prop_assert_eq!` not `assert_eq!`

`prop_assert_eq!` plays nicely with shrinking (recoverable failure); `assert_eq!` panics and skips shrink steps.
