# PR #6 Review — `feat: add omega network transport`

Branch: `feat/omega-network-group5`
Reviewers: code-reviewer + security-reviewer + writing-skills (parallel agents); consolidation by Claude
Date: 2026-05-05
Verdict (pre-fix): **request changes** (1 High security, 3 P1 correctness, 3 P1 skill-pack docs, 10 P2)
Verdict (post-fix): **approve** (all P0/P1 closed in commits `b18ccf2` + `455b469`; 6 P2 deferred to follow-up — listed below)

---

## Summary

`omega-network` introduces the libp2p adapter layer for openraft 0.9: a CBOR
`RaftRpcRequest` / `RaftRpcResponse` envelope, a `RaftNetworkFactory` /
`RaftNetwork` impl over an mpsc actor seam, and a separate explicit-chunking
`SnapshotFileReceiver` with hash-bound install. The PR also bundles the
rust-test pack v0.1.0 (9 skill folders + CHANGELOG) on top.

Three parallel reviewers ran (code-reviewer, security-reviewer,
writing-skills audit). Consolidated 16 findings; fixed 10 in two follow-up
commits on the branch; deferred 6 P2 items to the inbound-actor PR or to
v0.2.0 of the rust-test pack. CI was already green; post-fix the test count
goes from 9 to 23 (omega-network) with no regressions. `cargo clippy
--all-targets -- -D warnings`, `cargo fmt --check`, and `cargo doc -p
omega-network --no-deps --document-private-items` all clean on pinned
1.95.0.

---

## Findings closed in this PR

### Security

- **N6-H1 (High)** — `decode_cbor` now caps envelope size at 16 MiB and
  recursion depth at 64 (constants exposed as `MAX_RAFT_RPC_BYTES` /
  `MAX_CBOR_RECURSION`). Closes the byzantine-peer DoS via
  `Vec::with_capacity(huge_advertised_len)` and via stack-overflowing
  deeply-nested arrays. Two new tests verify both rejections fire.
  (`crates/omega-network/src/rpc.rs`, +`MAX_RAFT_RPC_BYTES`,
  `+OmegaNetworkError::Oversize`.)
- **N6-M3 (Medium)** — `LibP2pNetworkFactory` now uses a bounded
  `mpsc::Sender` with default capacity 256 (`with_capacity(n)` for
  tuning). `try_send` surfaces saturation as
  `OmegaNetworkError::OutboundFull` rather than blocking. The
  `RaftNetwork` round-trip honours `RPCOption::hard_ttl()` via
  `tokio::time::timeout`, with a 5s fallback for absurdly small caller
  deadlines. (`crates/omega-network/src/network.rs`.)
- **N6-L1 (Low)** — Snapshot receiver fsyncs the parent directory after
  the staged-to-installed rename on Unix. NTFS is metadata-journaled so
  the call is `cfg(not(unix))`-skipped on Windows.
  (`crates/omega-network/src/snapshot.rs`, `+sync_parent_dir`.)
- **N6-L2 (Low)** — `discovery::PeerAddress::from_str` now eagerly parses
  the multiaddr into a typed `libp2p::Multiaddr`. Typos in the `--peers`
  flag surface at config-parse time, not at first-dial time. New
  `DiscoveryError::InvalidMultiaddr` variant.
  (`crates/omega-network/src/discovery.rs`.)

### Correctness

- **C-P1-1 / N6-M1** — The `lib.rs` "Tier of trust" section previously
  implied `RaftNetwork::install_snapshot` flowed through the chunking
  protocol; in v0.1 it does not. The crate-level docstring now honestly
  separates (a) the codec caps that ARE in place on every Raft RPC
  payload from (b) the `SnapshotFileReceiver` chunking protocol that the
  node-runner crate will drive directly when the openraft-installed path
  outgrows `MAX_RAFT_RPC_BYTES`. The architectural mismatch is named
  rather than implied-and-broken. (`crates/omega-network/src/lib.rs`.)
- **C-P1-2** — `libp2p` workspace dep is no longer dead-code; it's now
  used in `discovery.rs` for the typed `Multiaddr` parse. One fix, two
  findings closed.
- **C-P1-3** — Added 6 new round-trip tests: `AppendEntries` request +
  response, `InstallSnapshot` request + response, plus the two decoder
  bound rejections. `Vote` was the only round-trip tested before; now
  every variant of both enums is exercised. A serde-derive break on any
  openraft RPC type or on `LedgerCommand` will surface mechanically.
  (`crates/omega-network/tests/rpc_codec.rs`, +6 tests.)
- **C-P2-1** — `RaftRpcRequest` and `RaftRpcResponse` carry
  `#[serde(rename_all = "snake_case")]` so the wire variant tag matches
  the diagnostic strings returned by `variant_name()`. One vocabulary
  across wire dumps and `WrongResponse` errors.

### Writing-skills (rust-test pack)

- **WS-P1-K1** — `rust-test-orchestrator/SKILL.md:58-60` had two Phase 5
  commands paired with Kani (nextest AND kani-bound.sh). Now exactly one:
  `bash skills/local/rust-test-kani/scripts/kani-bound.sh <crate>`.
  proptest gets nextest. Phase 5 vocabulary is unambiguous.
- **WS-P1-T1** — `rust-test-turmoil/references/loganet-patterns.md`
  examples that called `sim.host(n).is_leader()` and `.submit_claim()`
  are now explicitly marked as pseudocode with a paragraph pointing
  readers at the RPC-client pattern (the shape `omega-toy-consensus`
  Group 1 uses). Prevents copy-paste-then-wonder-why-it-won't-compile.
- **WS-P1-K2** — `rust-test-kani/scripts/kani-bound.sh` switched from
  `--output-format old` (deprecated in cargo-kani ≥ 0.50) to
  `--output-format regular`. No more deprecation warnings polluting
  orchestrator parsing.

### Doc / style

Module-level `//!` block on `network.rs` and `discovery.rs`. `# Errors`
blocks on every public Result-returning function. `# Examples` blocks
where applicable (`encode_cbor`, `decode_cbor`, `mdns_service_name`,
`DiscoveryConfig::new`). `# Soundness` block on
`SnapshotFileReceiver::receive` (the soundness-bearing apply gate)
following the preserves / closes / does-not-preserve triple structure.

`grep -rE "leverage|delve|underscore|harness the power|tapestry|key
insight|main theorem|proof strategy"` against the diff returns zero
matches. AI-tells policy clean.

---

## Findings deferred (P2 / follow-up)

These items are out of scope for the omega-network PR but are
real-and-flagged. Tracked here so they don't get lost.

| ID | Where it lives | Why deferred |
|---|---|---|
| **N6-M2** PeerId-to-NodeId binding | `omega-network` next PR (inbound actor) | The inbound dispatch path is not in this PR. When it lands, it MUST bind libp2p `PeerId` to openraft `NodeId` per genesis to close peer impersonation. Documented in `network.rs` module-level docs. |
| **N6-Info-1** Length-prefixed framing cap at codec layer | `omega-network` next PR (inbound actor) | The libp2p `request_response::Codec` will own on-wire framing; the `MAX_RAFT_RPC_BYTES` bound from this PR must be enforced at `read_request`/`read_response` *before* allocation. |
| **C-P2-2** Codec(String) collapses ciborium error structure | follow-up | Cosmetic — the inner `ciborium::de::Error<E>` is generic over the reader's error and awkward to embed via `#[from]`. Consider a `thiserror`-friendly wrapper if downstream pattern-matching becomes needed. |
| **WS-P2** ProptestConfig version pin | rust-test pack v0.2.0 | `ProptestConfig` is `#[non_exhaustive]` in newer proptest; struct-update syntax works but would be cleaner with `..ProptestConfig::default()`. |
| **WS-P2** init-fuzz-target.sh hardcoded SEED_DIR | rust-test pack v0.2.0 | Hardcoded to `../../tests/golden_vectors/per_leaf/utxo`; gated behind `if [[ -d ]]` so it's a no-op when not present, but worth a comment. |
| **WS-P2** madsim stub state | rust-test pack v0.2.0 | The skill is intentionally a placeholder until the first non-Tokio crate appears. |
| **WS-P2** stateright `.all_unique()` itertools dep | rust-test pack v0.2.0 | Cookbook example imports itertools without naming it; one-line note would close. |

---

## Verification (post-fix)

```
cargo build -p omega-network --tests             # clean
cargo test  -p omega-network --no-fail-fast      # 23/23 pass
cargo clippy -p omega-network --all-targets -- -D warnings   # clean
cargo fmt --check                                # clean
cargo doc  -p omega-network --no-deps --document-private-items  # clean
```

Test count: 23 (was 9). New: 6 round-trip tests, 2 decoder-bound rejection
tests, 1 zero-length-chunk-within-total snapshot test. No regressions.

---

## Recommendation

**approve** post-fix. All P0/P1 items closed on the branch in `b18ccf2`
("omega-network: address PR #6 review (security + correctness)") and
`455b469` ("rust-test pack: P1 doc fixes from PR #6 review"). The 6 P2
deferrals are tracked above and belong to either the inbound-actor PR or
to rust-test pack v0.2.0; none block this merge.
