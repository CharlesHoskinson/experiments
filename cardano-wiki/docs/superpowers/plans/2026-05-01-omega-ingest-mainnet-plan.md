# Omega v0.9.0 — Implement the Four Scaffolded Ingestion Paths

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the four `unimplemented!()` ingestion stubs from v0.8.0 (token-policy, script, stake, governance) with real implementations against extended hand-crafted CBOR fixtures. Lock ingestion-layer golden vectors for all five LedgerState-derivable sub-trees and pin a canonical "hybrid bundle" root that combines five CBOR-derived sub-trees with two JSON-derived ones.

**Architecture:** Build on v0.8.0's `omega-commitment-ingest` crate. Add an **extended UTXO CBOR fixture** with multi-asset bundles and script credentials; the existing UTXO ingestion supports both the v0.8.0 minimal format and the new 6-element extended format (backwards-compat). Token-policy and script ingestion walk the extended UTXO fixture's extension fields. Stake and governance get NEW dedicated CBOR fixtures + ingestion modules. Real Conway-era LedgerState parsing remains deferred to v1.0.

**Tech Stack:** Rust 1.79+, `pallas-codec::minicbor::Decoder`, plus the existing workspace dependency set. No new crates.

**Track:** T1 (Ω-Commitment Tooling) — ingestion sub-phase, completing the LedgerState-derivable subset.

**Locked design decisions honored (unchanged):**
- PQ-only crypto, Plonky3-friendly tree, selective dual-track at bundle layer, lazy/pull migration.
- Golden-vector regression net (introduced v0.8.0) is extended, not weakened.

---

## Honest scope statement

After this plan ships, **5 of 7 sub-trees have working CBOR-fixture ingestion** end-to-end. The remaining 2 (header, tx-index) need a chain-follower and stay scoped to a future plan.

**What v0.9.0 deliberately is NOT:** real Mithril snapshot parsing. Real Conway-era LedgerState CBOR is a multi-megabyte spec across 6+ cardano-ledger crates; pallas's LedgerState support is partial; Mithril snapshots are Cardano node DBs (immutable + ledger files), not single CBOR blobs. Bridging that gap is v1.0 work. The synthetic-CBOR ingestion path proves the architecture is sound and gives us deterministic golden vectors today.

---

## File structure (post-plan)

```
omega-commitment/
├── README.md                                                   (extended: v0.9.0 release notes)
├── crates/
│   ├── omega-commitment-core/                                  (unchanged)
│   ├── omega-commitment-cli/                                   (unchanged)
│   ├── omega-commitment-bundle/                                (unchanged)
│   └── omega-commitment-ingest/
│       ├── Cargo.toml                                          (version 0.9.0)
│       ├── src/
│       │   ├── lib.rs                                          (unchanged)
│       │   ├── main.rs                                         (unchanged)
│       │   ├── cbor.rs                                         (modify: add helpers for var-len bytes, maps, u128, optional)
│       │   ├── utxo.rs                                         (modify: support both 4-elem and 6-elem array fixtures)
│       │   ├── token_policy.rs                                 (rewrite: real impl walking UTXO multi-assets)
│       │   ├── script.rs                                       (rewrite: real impl walking UTXO script credentials)
│       │   ├── stake.rs                                        (rewrite: real impl parsing stake_snapshot.cbor)
│       │   └── governance.rs                                   (rewrite: real impl parsing governance_snapshot.cbor)
│       └── tests/
│           ├── fixtures/
│           │   ├── ledger_state_minimal.cbor                   (existing, unchanged)
│           │   ├── ledger_state_minimal.cbor.md                (existing)
│           │   ├── ledger_state_extended.cbor                  (NEW — 6-elem UTXOs with multi-assets + script creds)
│           │   ├── ledger_state_extended.cbor.md               (NEW)
│           │   ├── stake_snapshot.cbor                         (NEW)
│           │   ├── stake_snapshot.cbor.md                      (NEW)
│           │   ├── governance_snapshot.cbor                    (NEW)
│           │   └── governance_snapshot.cbor.md                 (NEW)
│           ├── utxo_ingest_integration.rs                      (modify: add tests for extended format)
│           ├── qa_pipeline.rs                                  (modify: extend with all 5 sub-trees)
│           ├── token_policy_ingest_integration.rs              (NEW)
│           ├── script_ingest_integration.rs                    (NEW)
│           ├── stake_ingest_integration.rs                     (NEW)
│           ├── governance_ingest_integration.rs                (NEW)
│           └── golden_ingest.rs                                (NEW — per-sub-tree pipeline roots + hybrid bundle root)
```

---

## Task 1: Extend `cbor.rs` with helpers for variable-length bytes, maps, u128, and optionals

**Files:**
- Modify: `crates/omega-commitment-ingest/src/cbor.rs`

The v0.8.0 `cbor.rs` only handles 32-byte arrays, u32/u64, and array headers. The extended fixtures need: 28-byte arrays (Cardano hashes), variable-length byte strings (asset names), map headers (multi-assets), 16-byte u128 (governance values as bytestrings per A5), and CBOR null detection (optional script credentials).

- [ ] **Step 1: Append helpers to `cbor.rs`**

Open `/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/cbor.rs`. Add these functions to the existing module (don't remove anything):

```rust
/// Read a 28-byte fixed-length byte string (Cardano Blake2b-224 hash).
pub fn read_28_bytes<'b>(d: &mut Decoder<'b>) -> Result<[u8; 28]> {
    let bytes = d
        .bytes()
        .map_err(|e| anyhow!("cbor: expected bytes ({e})"))?;
    if bytes.len() != 28 {
        return Err(anyhow!("cbor: expected 28-byte string, got {}", bytes.len()));
    }
    let mut out = [0u8; 28];
    out.copy_from_slice(bytes);
    Ok(out)
}

/// Read a 16-byte fixed-length byte string and decode as big-endian u128.
/// CBOR has no native u128 — the convention used by our extended fixtures
/// is: 16-byte bytestring, big-endian.
pub fn read_u128_bytes<'b>(d: &mut Decoder<'b>) -> Result<u128> {
    let bytes = d
        .bytes()
        .map_err(|e| anyhow!("cbor: expected bytes ({e})"))?;
    if bytes.len() != 16 {
        return Err(anyhow!("cbor: expected 16-byte u128 string, got {}", bytes.len()));
    }
    let mut buf = [0u8; 16];
    buf.copy_from_slice(bytes);
    Ok(u128::from_be_bytes(buf))
}

/// Read a u8 from a `Decoder` cursor.
pub fn read_u8<'b>(d: &mut Decoder<'b>) -> Result<u8> {
    let v = d.u8().map_err(|e| anyhow!("cbor: expected u8 ({e})"))?;
    Ok(v)
}

/// Read a variable-length byte string and copy it into a fresh `Vec<u8>`.
pub fn read_var_bytes<'b>(d: &mut Decoder<'b>) -> Result<Vec<u8>> {
    let bytes = d
        .bytes()
        .map_err(|e| anyhow!("cbor: expected bytes ({e})"))?;
    Ok(bytes.to_vec())
}

/// Read the header for a definite-length map and return its size.
pub fn read_map_len<'b>(d: &mut Decoder<'b>) -> Result<usize> {
    let len = d
        .map()
        .map_err(|e| anyhow!("cbor: expected map ({e})"))?
        .ok_or_else(|| anyhow!("cbor: expected definite-length map"))?;
    Ok(len as usize)
}

/// Probe the next CBOR datum: if it is `null`, consume it and return
/// `Ok(true)`. Otherwise leave the cursor at the datum and return
/// `Ok(false)`. Used for parsing optional fields like
/// `script_credential` which is either `null` or a 3-element array.
pub fn read_null_marker<'b>(d: &mut Decoder<'b>) -> Result<bool> {
    use pallas_codec::minicbor::data::Type;
    let ty = d.datatype().map_err(|e| anyhow!("cbor: peek failed ({e})"))?;
    if ty == Type::Null {
        d.null().map_err(|e| anyhow!("cbor: consume null ({e})"))?;
        Ok(true)
    } else {
        Ok(false)
    }
}
```

- [ ] **Step 2: Add unit tests for the new helpers**

Append to the existing `mod tests` in `cbor.rs` (inside the closing `}` of the test module):

```rust
    #[test]
    fn read_28_bytes_succeeds() {
        // CBOR for 28-byte string of 0xAAs: 0x58 0x1C [28 × 0xAA]
        let mut buf = vec![0x58, 0x1C];
        buf.extend_from_slice(&[0xAA; 28]);
        let mut d = Decoder::new(&buf);
        assert_eq!(read_28_bytes(&mut d).unwrap(), [0xAA; 28]);
    }

    #[test]
    fn read_28_bytes_fails_on_wrong_length() {
        let buf = vec![0x44, 0xDE, 0xAD, 0xBE, 0xEF];
        let mut d = Decoder::new(&buf);
        assert!(read_28_bytes(&mut d).is_err());
    }

    #[test]
    fn read_u128_bytes_round_trip() {
        // 16-byte big-endian encoding of 0x0102030405060708_090A0B0C0D0E0F10
        let mut buf = vec![0x50]; // CBOR bytestring header for 16 bytes
        let v: u128 = 0x0102030405060708_090A0B0C0D0E0F10u128;
        buf.extend_from_slice(&v.to_be_bytes());
        let mut d = Decoder::new(&buf);
        assert_eq!(read_u128_bytes(&mut d).unwrap(), v);
    }

    #[test]
    fn read_u128_bytes_fails_on_wrong_length() {
        let buf = vec![0x48, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]; // 8 bytes
        let mut d = Decoder::new(&buf);
        assert!(read_u128_bytes(&mut d).is_err());
    }

    #[test]
    fn read_u8_handles_small_int() {
        let buf = vec![0x07];
        let mut d = Decoder::new(&buf);
        assert_eq!(read_u8(&mut d).unwrap(), 7);
    }

    #[test]
    fn read_var_bytes_handles_short_string() {
        // 4-byte string 0xDEADBEEF
        let buf = vec![0x44, 0xDE, 0xAD, 0xBE, 0xEF];
        let mut d = Decoder::new(&buf);
        assert_eq!(read_var_bytes(&mut d).unwrap(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn read_map_len_reads_map_header() {
        // Empty map: 0xA0
        let buf = vec![0xA0];
        let mut d = Decoder::new(&buf);
        assert_eq!(read_map_len(&mut d).unwrap(), 0);
    }

    #[test]
    fn read_null_marker_consumes_null() {
        let buf = vec![0xF6]; // CBOR null
        let mut d = Decoder::new(&buf);
        assert!(read_null_marker(&mut d).unwrap());
    }

    #[test]
    fn read_null_marker_returns_false_on_non_null() {
        let buf = vec![0x05]; // CBOR uint 5
        let mut d = Decoder::new(&buf);
        assert!(!read_null_marker(&mut d).unwrap());
        // Cursor should still point at the uint.
        assert_eq!(d.u64().unwrap(), 5);
    }
```

- [ ] **Step 3: Verify**

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
cargo test -p omega-commitment-ingest cbor::tests 2>&1 | tail -20
cargo test --workspace 2>&1 | tail -5
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

Expected: 9 new cbor unit tests pass (4 from v0.8.0 + 9 new = 13). Workspace tests should still all pass.

If `cargo fmt-check` shows diffs, run `cargo fmt --all`.

If `pallas_codec::minicbor::data::Type` doesn't exist on the installed pallas version, try `pallas_codec::minicbor::data::Type::Null` directly or use the version check at `cargo doc -p pallas-codec --open`. As a fallback, replace `read_null_marker`'s impl with: probe via `d.probe().u8()` → if it fails, try `d.null()` directly. The pallas-codec used in v0.8.0 is 0.30.2 and it does expose `data::Type` — verify with `cargo doc` if needed.

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-ingest/src/cbor.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(ingest): cbor helpers for 28-byte hashes, var bytes, maps, u128, null marker"
```

---

## Task 2: Extended UTXO fixture (6-element format with multi-assets + script credentials)

**Files:**
- Create: `crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor` (binary)
- Create: `crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor.md`

The new fixture exercises every extension path: empty-multi-asset UTXO, single-asset UTXO, multi-asset UTXO with two policies, UTXO with a script credential. Total: 4 UTXOs.

- [ ] **Step 1: Write a generator to produce the fixture**

Create a temp Cargo project:

```bash
cd /home/hoskinson/omega-commitment
. "$HOME/.cargo/env"
mkdir -p /tmp/gen_extended
cat > /tmp/gen_extended/Cargo.toml << 'EOF'
[package]
name = "gen_extended"
version = "0.1.0"
edition = "2021"
[[bin]]
name = "gen_extended"
path = "src/main.rs"
EOF
mkdir -p /tmp/gen_extended/src
```

Write `/tmp/gen_extended/src/main.rs`:

```rust
use std::fs;

fn cbor_array_header(len: usize) -> Vec<u8> {
    if len < 24 { vec![0x80u8 | len as u8] }
    else if len < 256 { vec![0x98, len as u8] }
    else { vec![0x99, (len >> 8) as u8, len as u8] }
}

fn cbor_map_header(len: usize) -> Vec<u8> {
    if len < 24 { vec![0xA0u8 | len as u8] }
    else if len < 256 { vec![0xB8, len as u8] }
    else { vec![0xB9, (len >> 8) as u8, len as u8] }
}

fn cbor_bytes_header(len: usize) -> Vec<u8> {
    if len < 24 { vec![0x40u8 | len as u8] }
    else if len < 256 { vec![0x58, len as u8] }
    else { vec![0x59, (len >> 8) as u8, len as u8] }
}

fn cbor_uint(v: u64) -> Vec<u8> {
    if v < 24 { vec![v as u8] }
    else if v <= 0xff { vec![0x18, v as u8] }
    else if v <= 0xffff { vec![0x19, (v >> 8) as u8, v as u8] }
    else if v <= 0xffff_ffff {
        let mut o = vec![0x1a];
        o.extend_from_slice(&(v as u32).to_be_bytes());
        o
    } else {
        let mut o = vec![0x1b];
        o.extend_from_slice(&v.to_be_bytes());
        o
    }
}

fn cbor_null() -> Vec<u8> { vec![0xF6] }

fn cbor_bytes(b: &[u8]) -> Vec<u8> {
    let mut o = cbor_bytes_header(b.len());
    o.extend_from_slice(b);
    o
}

/// One asset entry: asset_name => quantity_u64
fn cbor_asset(name: &[u8], qty: u64) -> Vec<u8> {
    let mut o = cbor_bytes(name);
    o.extend(cbor_uint(qty));
    o
}

/// Multi-asset bundle: { policy_id_28 => { asset_name => qty, ... } }
/// Pass an empty slice to emit an empty map.
fn cbor_multi_assets(entries: &[(&[u8; 28], &[(&[u8], u64)])]) -> Vec<u8> {
    let mut o = cbor_map_header(entries.len());
    for (policy, assets) in entries {
        o.extend(cbor_bytes(*policy));
        let mut inner = cbor_map_header(assets.len());
        for (name, qty) in *assets {
            inner.extend(cbor_asset(name, *qty));
        }
        o.extend(inner);
    }
    o
}

/// Script credential: [script_hash_28, language_u8, script_size_u32]
fn cbor_script_cred(hash: &[u8; 28], language: u8, size: u32) -> Vec<u8> {
    let mut o = cbor_array_header(3);
    o.extend(cbor_bytes(hash));
    o.extend(cbor_uint(language as u64));
    o.extend(cbor_uint(size as u64));
    o
}

/// One extended UTXO: 6-element array.
fn extended_utxo(
    tx_id: &[u8; 32],
    out_idx: u64,
    addr: &[u8; 32],
    val: u64,
    multi_assets: Vec<u8>,
    script_credential: Vec<u8>,
) -> Vec<u8> {
    let mut o = cbor_array_header(6);
    o.extend(cbor_bytes(tx_id));
    o.extend(cbor_uint(out_idx));
    o.extend(cbor_bytes(addr));
    o.extend(cbor_uint(val));
    o.extend(multi_assets);
    o.extend(script_credential);
    o
}

fn main() {
    // Pre-defined synthetic policy ids and script hashes.
    let policy_a: [u8; 28] = [0xAA; 28];
    let policy_b: [u8; 28] = [0xBB; 28];
    let script_one: [u8; 28] = [0xCC; 28];
    let script_two: [u8; 28] = [0xDD; 28];

    let mut buf = Vec::new();
    buf.extend(cbor_array_header(4));

    // UTXO 0: bare (no multi-assets, no script credential).
    buf.extend(extended_utxo(
        &[0x11; 32], 0, &[0xA0; 32], 1_000_000,
        cbor_multi_assets(&[]),
        cbor_null(),
    ));

    // UTXO 1: single-policy single-asset.
    buf.extend(extended_utxo(
        &[0x22; 32], 1, &[0xA1; 32], 5_000_000,
        cbor_multi_assets(&[(&policy_a, &[(b"COIN", 100)])]),
        cbor_null(),
    ));

    // UTXO 2: two policies, multiple assets each, plus a script credential
    //         (Plutus V2 = language 2, size 1024 bytes).
    buf.extend(extended_utxo(
        &[0x33; 32], 0, &[0xA2; 32], 25_000_000,
        cbor_multi_assets(&[
            (&policy_a, &[(b"COIN", 50), (b"NFT", 1)]),
            (&policy_b, &[(b"TOKEN", 999)]),
        ]),
        cbor_script_cred(&script_one, 2, 1024),
    ));

    // UTXO 3: bare value but with a different script credential
    //         (native multisig = language 0, size 256 bytes).
    buf.extend(extended_utxo(
        &[0x44; 32], 0, &[0xA3; 32], 10_000_000,
        cbor_multi_assets(&[]),
        cbor_script_cred(&script_two, 0, 256),
    ));

    let dest = "/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor";
    fs::write(dest, &buf).unwrap();
    println!("wrote {} bytes to {}", buf.len(), dest);
}
```

Run:

```bash
cd /tmp/gen_extended && cargo run --release
cd /home/hoskinson/omega-commitment
ls -la crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor
xxd crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor | head -8
```

Expected: ~400-byte file. xxd should show `0x84` (CBOR array of 4) followed by the first UTXO's `0x86` (CBOR array of 6).

- [ ] **Step 2: Document the fixture**

Write `/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor.md`:

```markdown
# `ledger_state_extended.cbor`

A hand-crafted extended CBOR fixture containing 4 UTXOs in the v0.9.0
6-element extended format. Adds multi-asset bundles and optional script
credentials on top of v0.8.0's 4-element minimal format.

## Per-UTXO format

```
[ tx_id (32 bytes), output_index (u64), address (32 bytes),
  value_lovelace (u64),
  multi_assets (CBOR map: { policy_id_28 => { asset_name_var => qty_u64 } }),
  script_credential (CBOR null OR [script_hash_28, language_u8, script_size_u32]) ]
```

Both extension fields are mandatory (always present) but `multi_assets`
may be an empty map and `script_credential` may be CBOR null.

## Contents

| # | tx_id | value | multi_assets | script_credential |
|---|---|---|---|---|
| 0 | `1111…11` | 1_000_000 | empty | null |
| 1 | `2222…22` | 5_000_000 | policy_a:{COIN→100} | null |
| 2 | `3333…33` | 25_000_000 | policy_a:{COIN→50,NFT→1}, policy_b:{TOKEN→999} | script_one (Plutus V2, 1024 B) |
| 3 | `4444…44` | 10_000_000 | empty | script_two (native multi-sig, 256 B) |

Where:
- `policy_a` = 28×0xAA
- `policy_b` = 28×0xBB
- `script_one` = 28×0xCC
- `script_two` = 28×0xDD

## Derived sub-tree contents

After ingestion, the fixture should produce:

- **utxo** sub-tree: 4 entries (one per UTXO; assets and script_credential
  fields are not surfaced in the UTXO sub-tree's JSON output — they're
  consumed by the token-policy and script sub-trees).
- **token-policy** sub-tree: 2 entries (policy_a and policy_b).
  total_supply_at_h = sum across all assets and UTXOs:
    - policy_a: 100 + 50 + 1 = 151
    - policy_b: 999
- **script** sub-tree: 2 entries (script_one Plutus V2 1024 B, script_two
  native 256 B).

## Regeneration

The fixture is generated by the Rust helper documented in Task 2,
Step 1 of `2026-05-01-omega-ingest-mainnet-plan.md`.
```

- [ ] **Step 3: Verify the fixture exists and is non-empty**

```bash
file crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor
wc -c crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor
xxd crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor | head -3
```

Expected: ~400-byte binary file. First byte should be `0x84` (CBOR array of 4).

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor \
        crates/omega-commitment-ingest/tests/fixtures/ledger_state_extended.cbor.md
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test(ingest): add extended CBOR fixture (multi-assets + script credentials)"
```

---

## Task 3: UTXO ingestion supports both 4-element and 6-element fixtures (backwards-compat)

**Files:**
- Modify: `crates/omega-commitment-ingest/src/utxo.rs`

The existing `ingest_utxos` parses 4-element arrays. Update it to detect array length: if it's 4, use the v0.8.0 path; if it's 6, parse the extended fields and discard them (UTXO sub-tree output is unchanged). This keeps v0.8.0 fixtures + tests working AND lets the same function handle the v0.9.0 extended fixture.

- [ ] **Step 1: Update `ingest_utxos`**

Open `/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/utxo.rs`. Replace the body of the for-loop in `ingest_utxos` so it dispatches on array length:

Find the existing block (in the for-loop):

```rust
        expect_array(&mut d, 4)?;
        let tx_id = read_32_bytes(&mut d)?;
        let output_index = u32::try_from(read_u64(&mut d)?)
            .map_err(|_| anyhow::anyhow!("output_index too large for u32"))?;
        let address_hash = read_32_bytes(&mut d)?;
        let value_lovelace = read_u64(&mut d)?;
        utxos.push(Utxo {
            tx_id,
            output_index,
            address_hash,
            value_lovelace,
            assets: Vec::new(),
            datum_hash: None,
        });
```

Replace it with the dispatch on array length:

```rust
        let arity = read_array_len(&mut d)?;
        if arity != 4 && arity != 6 {
            return Err(anyhow::anyhow!(
                "utxo entry must be 4-elem (v0.8 minimal) or 6-elem (v0.9 extended), got {arity}"
            ));
        }
        let tx_id = read_32_bytes(&mut d)?;
        let output_index = u32::try_from(read_u64(&mut d)?)
            .map_err(|_| anyhow::anyhow!("output_index too large for u32"))?;
        let address_hash = read_32_bytes(&mut d)?;
        let value_lovelace = read_u64(&mut d)?;
        if arity == 6 {
            // Skip multi_assets (CBOR map) and script_credential (CBOR null
            // or 3-element array). The extended fields are consumed by the
            // token-policy and script ingestion paths, not the UTXO sub-tree.
            skip_multi_assets(&mut d)?;
            skip_script_credential(&mut d)?;
        }
        utxos.push(Utxo {
            tx_id,
            output_index,
            address_hash,
            value_lovelace,
            assets: Vec::new(),
            datum_hash: None,
        });
```

The previous code used `expect_array(&mut d, 4)`. We replace it with `read_array_len` so we can dispatch.

You'll need to update the import line at the top of the file. Find:

```rust
use crate::cbor::{expect_array, read_32_bytes, read_array_len, read_u64};
```

And replace with:

```rust
use crate::cbor::{
    read_28_bytes, read_32_bytes, read_array_len, read_map_len, read_null_marker,
    read_u32, read_u64, read_u8, read_var_bytes,
};
```

(`expect_array` may no longer be used in this file — leave the import as `read_array_len` only or trim down to whatever set is actually consumed; the rule is no unused imports. Run cargo build to confirm.)

Now add the two skip helpers at the bottom of the file (still inside the same module, before `#[cfg(test)] mod tests`):

```rust
/// Skip a multi-asset bundle (a CBOR map: { policy_28 => { name => u64 } }).
/// Used by UTXO ingestion to bypass extension fields it doesn't surface.
fn skip_multi_assets(d: &mut pallas_codec::minicbor::Decoder<'_>) -> anyhow::Result<()> {
    let n_policies = read_map_len(d)?;
    for _ in 0..n_policies {
        let _policy: [u8; 28] = read_28_bytes(d)?;
        let n_assets = read_map_len(d)?;
        for _ in 0..n_assets {
            let _name: Vec<u8> = read_var_bytes(d)?;
            let _qty: u64 = read_u64(d)?;
        }
    }
    Ok(())
}

/// Skip a script credential, which is either CBOR null or a 3-element
/// array [script_hash_28, language_u8, script_size_u32].
fn skip_script_credential(d: &mut pallas_codec::minicbor::Decoder<'_>) -> anyhow::Result<()> {
    if read_null_marker(d)? {
        return Ok(());
    }
    let arity = read_array_len(d)?;
    if arity != 3 {
        return Err(anyhow::anyhow!(
            "script_credential array must be 3-elem, got {arity}"
        ));
    }
    let _hash: [u8; 28] = read_28_bytes(d)?;
    let _language: u8 = read_u8(d)?;
    let _size: u32 = read_u32(d)?;
    Ok(())
}
```

- [ ] **Step 2: Add unit tests for the extended path**

Append to the existing `mod tests` in `utxo.rs`:

```rust
    #[test]
    fn ingest_extended_fixture_path_yields_same_utxo_shape() {
        // Build a minimal extended (6-elem) fixture inline and confirm
        // the UTXO sub-tree output is identical to what the 4-elem path
        // would produce — extension fields are skipped at the UTXO layer.
        fn cbor_array_header(len: usize) -> Vec<u8> {
            if len < 24 { vec![0x80u8 | len as u8] }
            else if len < 256 { vec![0x98, len as u8] }
            else { vec![0x99, (len >> 8) as u8, len as u8] }
        }
        fn cbor_bytes_header(len: usize) -> Vec<u8> {
            if len < 24 { vec![0x40u8 | len as u8] }
            else if len < 256 { vec![0x58, len as u8] }
            else { vec![0x59, (len >> 8) as u8, len as u8] }
        }
        fn cbor_uint(v: u64) -> Vec<u8> {
            if v < 24 { vec![v as u8] }
            else if v <= 0xff { vec![0x18, v as u8] }
            else if v <= 0xffff { vec![0x19, (v >> 8) as u8, v as u8] }
            else if v <= 0xffff_ffff {
                let mut o = vec![0x1a];
                o.extend_from_slice(&(v as u32).to_be_bytes());
                o
            } else {
                let mut o = vec![0x1b];
                o.extend_from_slice(&v.to_be_bytes());
                o
            }
        }
        fn cbor_bytes(b: &[u8]) -> Vec<u8> {
            let mut o = cbor_bytes_header(b.len());
            o.extend_from_slice(b);
            o
        }
        // One extended UTXO with empty multi-assets and null script credential.
        let mut buf = Vec::new();
        buf.extend(cbor_array_header(1));
        buf.extend(cbor_array_header(6));
        buf.extend(cbor_bytes(&[0x11; 32]));
        buf.extend(cbor_uint(0));
        buf.extend(cbor_bytes(&[0x22; 32]));
        buf.extend(cbor_uint(1_000_000));
        buf.push(0xA0); // empty map
        buf.push(0xF6); // null

        let out = ingest_utxos(&buf).unwrap();
        assert_eq!(out.utxos.len(), 1);
        assert_eq!(out.utxos[0].tx_id, [0x11; 32]);
        assert_eq!(out.utxos[0].output_index, 0);
        assert_eq!(out.utxos[0].address_hash, [0x22; 32]);
        assert_eq!(out.utxos[0].value_lovelace, 1_000_000);
        assert!(out.utxos[0].assets.is_empty());
        assert!(out.utxos[0].datum_hash.is_none());
    }

    #[test]
    fn ingest_real_extended_fixture_succeeds() {
        let cbor = std::fs::read(
            "tests/fixtures/ledger_state_extended.cbor"
        ).expect("extended fixture readable");
        let out = ingest_utxos(&cbor).unwrap();
        assert_eq!(out.utxos.len(), 4);
        // UTXO outputs are independent of multi-assets/script_credential.
        assert_eq!(out.utxos[0].value_lovelace, 1_000_000);
        assert_eq!(out.utxos[1].value_lovelace, 5_000_000);
        assert_eq!(out.utxos[2].value_lovelace, 25_000_000);
        assert_eq!(out.utxos[3].value_lovelace, 10_000_000);
    }

    #[test]
    fn ingest_rejects_5_elem_arity() {
        // 5-element UTXO is not a recognized format.
        let buf = vec![0x81, 0x85, 0x40, 0x40, 0x40, 0x40, 0x40];
        assert!(ingest_utxos(&buf).is_err());
    }
```

- [ ] **Step 3: Verify**

```bash
cargo test -p omega-commitment-ingest utxo::tests 2>&1 | tail -10
cargo test --workspace 2>&1 | tail -5    # 192 (189 prior + 3 new utxo unit tests)
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

The existing `ingest_minimal_fixture` test from v0.8.0 should still pass — it uses the 4-elem format, and the new dispatch handles both arity 4 and 6.

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-ingest/src/utxo.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(ingest): UTXO ingestion accepts both 4-elem and 6-elem CBOR fixtures"
```

---

## Task 4: Token-policy ingestion (walks extended UTXO multi-assets)

**Files:**
- Modify: `crates/omega-commitment-ingest/src/token_policy.rs`

Replace the v0.8.0 `unimplemented!()` body with a real implementation that parses the extended UTXO fixture, walks each UTXO's multi_assets map, sums quantities per policy_id across all assets and UTXOs, and emits a `TokenPolicyOutput`.

For first_issuance_slot: the simplified fixture doesn't carry per-policy timing; pin all policies to slot 0 and document this as a v1.0 limitation.

- [ ] **Step 1: Replace `token_policy.rs`**

Replace the entire contents of `/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/token_policy.rs` with:

```rust
//! Token-policy sub-tree ingestion from the v0.9.0 extended CBOR fixture.
//!
//! Walks each UTXO's `multi_assets` map and aggregates per-`policy_id`:
//!   - `total_supply_at_h` = sum of quantities across all assets in all
//!     UTXOs that mention this policy.
//!   - `first_issuance_slot` = pinned to `0` (the simplified fixture
//!     does not carry per-policy timing data; real-data ingestion will
//!     pull this from chain history in v1.0).
//!
//! Output policies are sorted by `policy_id` to make the output stable.

use crate::cbor::{
    read_28_bytes, read_32_bytes, read_array_len, read_map_len, read_null_marker,
    read_u32, read_u64, read_u8, read_var_bytes,
};
use anyhow::Result;
use omega_commitment_core::token_policy_leaf::TokenPolicy;
use pallas_codec::minicbor::Decoder;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct TokenPolicyOutput {
    pub policies: Vec<TokenPolicy>,
}

/// Ingest token-policy entries by walking the extended UTXO fixture's
/// per-UTXO multi-asset bundles.
///
/// Only the v0.9.0 extended (6-elem) fixture format carries multi-asset
/// data. If the input is the v0.8.0 minimal (4-elem) format, no policies
/// are emitted (the result is `policies: []`).
pub fn ingest_token_policies(cbor: &[u8]) -> Result<TokenPolicyOutput> {
    let mut totals: BTreeMap<[u8; 28], u128> = BTreeMap::new();
    let mut d = Decoder::new(cbor);
    let n = read_array_len(&mut d)?;
    for _ in 0..n {
        let arity = read_array_len(&mut d)?;
        if arity != 4 && arity != 6 {
            return Err(anyhow::anyhow!(
                "utxo entry must be 4-elem or 6-elem, got {arity}"
            ));
        }
        let _tx_id = read_32_bytes(&mut d)?;
        let _out_idx = read_u64(&mut d)?;
        let _addr = read_32_bytes(&mut d)?;
        let _value = read_u64(&mut d)?;
        if arity == 4 {
            // Minimal format carries no multi-assets; nothing to walk.
            continue;
        }
        // Extended format: walk multi_assets map, then skip script_credential.
        let n_policies = read_map_len(&mut d)?;
        for _ in 0..n_policies {
            let policy: [u8; 28] = read_28_bytes(&mut d)?;
            let n_assets = read_map_len(&mut d)?;
            let mut policy_total: u128 = 0;
            for _ in 0..n_assets {
                let _name: Vec<u8> = read_var_bytes(&mut d)?;
                let qty: u64 = read_u64(&mut d)?;
                policy_total = policy_total
                    .checked_add(qty as u128)
                    .ok_or_else(|| anyhow::anyhow!("token-policy total_supply overflow u128"))?;
            }
            let entry = totals.entry(policy).or_insert(0u128);
            *entry = entry
                .checked_add(policy_total)
                .ok_or_else(|| anyhow::anyhow!("token-policy total_supply overflow u128"))?;
        }
        // Consume script credential (null or 3-elem array).
        if !read_null_marker(&mut d)? {
            let arity = read_array_len(&mut d)?;
            if arity != 3 {
                return Err(anyhow::anyhow!("script_credential arity {arity} != 3"));
            }
            let _hash: [u8; 28] = read_28_bytes(&mut d)?;
            let _language: u8 = read_u8(&mut d)?;
            let _size: u32 = read_u32(&mut d)?;
        }
    }
    let policies: Vec<TokenPolicy> = totals
        .into_iter()
        .map(|(policy_id, total_supply_at_h)| TokenPolicy {
            policy_id,
            first_issuance_slot: 0,
            total_supply_at_h,
        })
        .collect();
    Ok(TokenPolicyOutput { policies })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extended_fixture_bytes() -> Vec<u8> {
        std::fs::read("tests/fixtures/ledger_state_extended.cbor").unwrap()
    }

    #[test]
    fn ingest_extended_fixture_yields_two_policies() {
        let out = ingest_token_policies(&extended_fixture_bytes()).unwrap();
        assert_eq!(out.policies.len(), 2);
        // Policies are sorted by policy_id, so policy_a (0xAA…) comes before policy_b (0xBB…).
        assert_eq!(out.policies[0].policy_id, [0xAA; 28]);
        assert_eq!(out.policies[1].policy_id, [0xBB; 28]);
    }

    #[test]
    fn total_supply_aggregated_correctly() {
        let out = ingest_token_policies(&extended_fixture_bytes()).unwrap();
        // policy_a: COIN(100) from UTXO 1 + COIN(50) + NFT(1) from UTXO 2 = 151
        assert_eq!(out.policies[0].total_supply_at_h, 151);
        // policy_b: TOKEN(999) from UTXO 2
        assert_eq!(out.policies[1].total_supply_at_h, 999);
    }

    #[test]
    fn first_issuance_slot_pinned_to_zero() {
        let out = ingest_token_policies(&extended_fixture_bytes()).unwrap();
        for p in &out.policies {
            assert_eq!(
                p.first_issuance_slot, 0,
                "synthetic-fixture limitation: pin all policies to slot 0"
            );
        }
    }

    #[test]
    fn minimal_fixture_yields_zero_policies() {
        let cbor = std::fs::read("tests/fixtures/ledger_state_minimal.cbor").unwrap();
        let out = ingest_token_policies(&cbor).unwrap();
        assert!(out.policies.is_empty(), "v0.8 minimal fixture has no multi-assets");
    }

    #[test]
    fn deterministic_across_runs() {
        let cbor = extended_fixture_bytes();
        let a = ingest_token_policies(&cbor).unwrap();
        let b = ingest_token_policies(&cbor).unwrap();
        let a_json = serde_json::to_string(&a).unwrap();
        let b_json = serde_json::to_string(&b).unwrap();
        assert_eq!(a_json, b_json);
    }
}
```

- [ ] **Step 2: Verify**

```bash
cargo test -p omega-commitment-ingest token_policy::tests 2>&1 | tail -10
cargo test --workspace 2>&1 | tail -5    # 196
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

The 5 new token_policy tests should pass. The previously `#[ignore]`d v0.8.0 stub test is now a real test (it ran the `unimplemented!()` body via `let _ = ...`); since the function no longer panics, the ignored stub now passes if uningored. Update the v0.8.0 scaffold test to remove `#[ignore]`:

In `token_policy.rs`'s test module, find the v0.8.0 stub (if it's still there from the v0.8.0 scaffold task):

```rust
    #[test]
    #[ignore = "scaffold: requires real Conway LedgerState parsing"]
    fn ingest_token_policies_minimal_fixture() {
        let _ = ingest_token_policies(&[0x80]);
    }
```

The full `token_policy.rs` rewrite above doesn't include this test (we replaced the file entirely). So it's already gone — good.

- [ ] **Step 3: Commit**

```bash
git add crates/omega-commitment-ingest/src/token_policy.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(ingest): token-policy ingestion from extended UTXO multi-assets"
```

---

## Task 5: Script ingestion (walks extended UTXO script credentials)

**Files:**
- Modify: `crates/omega-commitment-ingest/src/script.rs`

Real implementation that walks each UTXO's script_credential field, deduplicates by script_hash, and emits a `ScriptOutput`.

For deployment_slot: same simplified-fixture limitation as token-policy. Pin all to slot 0.

- [ ] **Step 1: Replace `script.rs`**

Replace `/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/script.rs` with:

```rust
//! Script-registry sub-tree ingestion from the v0.9.0 extended CBOR fixture.
//!
//! Walks each UTXO's optional `script_credential` field, deduplicates by
//! `script_hash`, and emits the script-registry leaf entries. Output is
//! sorted by `script_hash` for stability.
//!
//! `deployment_slot` is pinned to `0` (the simplified fixture does not
//! carry per-script deployment timing; real-data ingestion will pull
//! this from chain history in v1.0).

use crate::cbor::{
    read_28_bytes, read_32_bytes, read_array_len, read_map_len, read_null_marker,
    read_u32, read_u64, read_u8, read_var_bytes,
};
use anyhow::Result;
use omega_commitment_core::script_registry_leaf::ScriptEntry;
use pallas_codec::minicbor::Decoder;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct ScriptOutput {
    pub scripts: Vec<ScriptEntry>,
}

/// Ingest script-registry entries by walking the extended UTXO
/// fixture's per-UTXO `script_credential` field. Deduplicates by
/// `script_hash`; if the same hash appears with different metadata
/// across UTXOs, the first occurrence wins.
pub fn ingest_scripts(cbor: &[u8]) -> Result<ScriptOutput> {
    let mut seen: BTreeMap<[u8; 28], ScriptEntry> = BTreeMap::new();
    let mut d = Decoder::new(cbor);
    let n = read_array_len(&mut d)?;
    for _ in 0..n {
        let arity = read_array_len(&mut d)?;
        if arity != 4 && arity != 6 {
            return Err(anyhow::anyhow!(
                "utxo entry must be 4-elem or 6-elem, got {arity}"
            ));
        }
        let _tx_id = read_32_bytes(&mut d)?;
        let _out_idx = read_u64(&mut d)?;
        let _addr = read_32_bytes(&mut d)?;
        let _value = read_u64(&mut d)?;
        if arity == 4 {
            continue;
        }
        // Skip multi-assets first, then read script credential.
        let n_policies = read_map_len(&mut d)?;
        for _ in 0..n_policies {
            let _policy: [u8; 28] = read_28_bytes(&mut d)?;
            let n_assets = read_map_len(&mut d)?;
            for _ in 0..n_assets {
                let _name: Vec<u8> = read_var_bytes(&mut d)?;
                let _qty: u64 = read_u64(&mut d)?;
            }
        }
        if read_null_marker(&mut d)? {
            continue;
        }
        let arity = read_array_len(&mut d)?;
        if arity != 3 {
            return Err(anyhow::anyhow!("script_credential arity {arity} != 3"));
        }
        let script_hash: [u8; 28] = read_28_bytes(&mut d)?;
        let language: u8 = read_u8(&mut d)?;
        let script_size_bytes: u32 = read_u32(&mut d)?;
        seen.entry(script_hash).or_insert(ScriptEntry {
            script_hash,
            deployment_slot: 0,
            script_size_bytes,
            language,
        });
    }
    Ok(ScriptOutput {
        scripts: seen.into_values().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extended_fixture_bytes() -> Vec<u8> {
        std::fs::read("tests/fixtures/ledger_state_extended.cbor").unwrap()
    }

    #[test]
    fn ingest_extended_fixture_yields_two_scripts() {
        let out = ingest_scripts(&extended_fixture_bytes()).unwrap();
        assert_eq!(out.scripts.len(), 2);
        // Sorted by script_hash, so script_one (0xCC…) precedes script_two (0xDD…).
        assert_eq!(out.scripts[0].script_hash, [0xCC; 28]);
        assert_eq!(out.scripts[1].script_hash, [0xDD; 28]);
    }

    #[test]
    fn script_metadata_preserved() {
        let out = ingest_scripts(&extended_fixture_bytes()).unwrap();
        // script_one: Plutus V2 (language=2), 1024 bytes
        assert_eq!(out.scripts[0].language, 2);
        assert_eq!(out.scripts[0].script_size_bytes, 1024);
        // script_two: native multi-sig (language=0), 256 bytes
        assert_eq!(out.scripts[1].language, 0);
        assert_eq!(out.scripts[1].script_size_bytes, 256);
    }

    #[test]
    fn deployment_slot_pinned_to_zero() {
        let out = ingest_scripts(&extended_fixture_bytes()).unwrap();
        for s in &out.scripts {
            assert_eq!(s.deployment_slot, 0);
        }
    }

    #[test]
    fn minimal_fixture_yields_zero_scripts() {
        let cbor = std::fs::read("tests/fixtures/ledger_state_minimal.cbor").unwrap();
        let out = ingest_scripts(&cbor).unwrap();
        assert!(out.scripts.is_empty());
    }

    #[test]
    fn deterministic_across_runs() {
        let cbor = extended_fixture_bytes();
        let a = ingest_scripts(&cbor).unwrap();
        let b = ingest_scripts(&cbor).unwrap();
        let a_json = serde_json::to_string(&a).unwrap();
        let b_json = serde_json::to_string(&b).unwrap();
        assert_eq!(a_json, b_json);
    }
}
```

- [ ] **Step 2: Verify**

```bash
cargo test -p omega-commitment-ingest script::tests 2>&1 | tail -10    # 5 tests pass
cargo test --workspace 2>&1 | tail -5    # 201
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

- [ ] **Step 3: Commit**

```bash
git add crates/omega-commitment-ingest/src/script.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(ingest): script-registry ingestion from extended UTXO script credentials"
```

---

## Task 6: Stake fixture + ingestion

**Files:**
- Create: `crates/omega-commitment-ingest/tests/fixtures/stake_snapshot.cbor`
- Create: `crates/omega-commitment-ingest/tests/fixtures/stake_snapshot.cbor.md`
- Modify: `crates/omega-commitment-ingest/src/stake.rs`

Generate a hand-crafted CBOR fixture with 4 stake entries covering the full state space (undelegated, pool-only, pool+DRep, pool-operator). Implement `ingest_stake` to parse it and emit a `StakeOutput`.

- [ ] **Step 1: Generate the fixture**

Create a temp project:

```bash
mkdir -p /tmp/gen_stake/src
cat > /tmp/gen_stake/Cargo.toml << 'EOF'
[package]
name = "gen_stake"
version = "0.1.0"
edition = "2021"
[[bin]]
name = "gen_stake"
path = "src/main.rs"
EOF
```

Write `/tmp/gen_stake/src/main.rs`:

```rust
use std::fs;

fn cbor_array_header(len: usize) -> Vec<u8> {
    if len < 24 { vec![0x80u8 | len as u8] } else if len < 256 { vec![0x98, len as u8] } else { vec![0x99, (len >> 8) as u8, len as u8] }
}
fn cbor_bytes_header(len: usize) -> Vec<u8> {
    if len < 24 { vec![0x40u8 | len as u8] } else if len < 256 { vec![0x58, len as u8] } else { vec![0x59, (len >> 8) as u8, len as u8] }
}
fn cbor_uint(v: u64) -> Vec<u8> {
    if v < 24 { vec![v as u8] } else if v <= 0xff { vec![0x18, v as u8] }
    else if v <= 0xffff { vec![0x19, (v >> 8) as u8, v as u8] }
    else if v <= 0xffff_ffff { let mut o = vec![0x1a]; o.extend_from_slice(&(v as u32).to_be_bytes()); o }
    else { let mut o = vec![0x1b]; o.extend_from_slice(&v.to_be_bytes()); o }
}
fn cbor_bytes(b: &[u8]) -> Vec<u8> { let mut o = cbor_bytes_header(b.len()); o.extend_from_slice(b); o }

fn stake_entry(cred: &[u8;28], pool: &[u8;28], drep: &[u8;28], rewards: u64, is_op: u8) -> Vec<u8> {
    let mut o = cbor_array_header(5);
    o.extend(cbor_bytes(cred));
    o.extend(cbor_bytes(pool));
    o.extend(cbor_bytes(drep));
    o.extend(cbor_uint(rewards));
    o.extend(cbor_uint(is_op as u64));
    o
}

fn main() {
    let zero: [u8;28] = [0u8;28];
    let pool_a: [u8;28] = [0xAA;28];
    let pool_b: [u8;28] = [0xBB;28];
    let drep_a: [u8;28] = [0xCC;28];

    let mut buf = Vec::new();
    buf.extend(cbor_array_header(4));
    // Entry 0: fully undelegated, zero rewards, not pool operator.
    buf.extend(stake_entry(&[0x11;28], &zero, &zero, 0, 0));
    // Entry 1: delegated to pool_a only.
    buf.extend(stake_entry(&[0x22;28], &pool_a, &zero, 1_000_000, 0));
    // Entry 2: delegated to pool_b + DRep_a.
    buf.extend(stake_entry(&[0x33;28], &pool_b, &drep_a, 5_000_000, 0));
    // Entry 3: pool operator (delegated to own pool, rewards account).
    buf.extend(stake_entry(&[0x44;28], &pool_a, &zero, 100_000_000, 1));

    let dest = "/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/fixtures/stake_snapshot.cbor";
    fs::write(dest, &buf).unwrap();
    println!("wrote {} bytes", buf.len());
}
```

Run:

```bash
cd /tmp/gen_stake && cargo run --release
cd /home/hoskinson/omega-commitment
ls -la crates/omega-commitment-ingest/tests/fixtures/stake_snapshot.cbor
```

Expected: ~390-byte file.

- [ ] **Step 2: Document the fixture**

Write `/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/fixtures/stake_snapshot.cbor.md`:

```markdown
# `stake_snapshot.cbor`

Hand-crafted CBOR fixture with 4 stake-state entries.

## Per-entry format

```
[ stake_credential_hash (28 bytes), delegated_pool (28 bytes),
  delegated_drep (28 bytes), rewards_lovelace (u64),
  is_pool_operator (u8) ]
```

All-zero pool means undelegated; all-zero DRep means no DRep delegation.

## Contents

| # | credential | pool | drep | rewards | is_pool_operator |
|---|---|---|---|---|---|
| 0 | `1111…11` | zero | zero | 0 | 0 |
| 1 | `2222…22` | pool_a (0xAA) | zero | 1_000_000 | 0 |
| 2 | `3333…33` | pool_b (0xBB) | drep_a (0xCC) | 5_000_000 | 0 |
| 3 | `4444…44` | pool_a | zero | 100_000_000 | 1 |

Covers: undelegated / pool-only / pool+DRep / pool-operator.

## Regeneration

Generated by the helper in Task 6, Step 1 of
`2026-05-01-omega-ingest-mainnet-plan.md`.
```

- [ ] **Step 3: Replace `stake.rs`**

Replace `/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/stake.rs` with:

```rust
//! Stake-state sub-tree ingestion from a hand-crafted stake_snapshot.cbor.
//!
//! Top-level CBOR array of 5-element entries:
//!   [ stake_credential_hash (28), delegated_pool (28),
//!     delegated_drep (28), rewards_lovelace (u64),
//!     is_pool_operator (u8) ]
//!
//! Maps 1:1 onto `omega_commitment_core::stake_state_leaf::StakeEntry`.

use crate::cbor::{read_28_bytes, read_array_len, read_u64, read_u8};
use anyhow::Result;
use omega_commitment_core::stake_state_leaf::StakeEntry;
use pallas_codec::minicbor::Decoder;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct StakeOutput {
    pub stake_entries: Vec<StakeEntry>,
}

pub fn ingest_stake(cbor: &[u8]) -> Result<StakeOutput> {
    let mut d = Decoder::new(cbor);
    let n = read_array_len(&mut d)?;
    let mut stake_entries = Vec::with_capacity(n);
    for _ in 0..n {
        let arity = read_array_len(&mut d)?;
        if arity != 5 {
            return Err(anyhow::anyhow!(
                "stake entry must be 5-elem, got {arity}"
            ));
        }
        let stake_credential_hash = read_28_bytes(&mut d)?;
        let delegated_pool = read_28_bytes(&mut d)?;
        let delegated_drep = read_28_bytes(&mut d)?;
        let rewards_lovelace = read_u64(&mut d)?;
        let is_pool_operator = read_u8(&mut d)?;
        stake_entries.push(StakeEntry {
            stake_credential_hash,
            delegated_pool,
            delegated_drep,
            rewards_lovelace,
            is_pool_operator,
        });
    }
    Ok(StakeOutput { stake_entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        std::fs::read("tests/fixtures/stake_snapshot.cbor").unwrap()
    }

    #[test]
    fn ingest_yields_four_entries() {
        let out = ingest_stake(&fixture()).unwrap();
        assert_eq!(out.stake_entries.len(), 4);
    }

    #[test]
    fn entry_zero_is_undelegated() {
        let out = ingest_stake(&fixture()).unwrap();
        let e = &out.stake_entries[0];
        assert_eq!(e.stake_credential_hash, [0x11; 28]);
        assert_eq!(e.delegated_pool, [0u8; 28]);
        assert_eq!(e.delegated_drep, [0u8; 28]);
        assert_eq!(e.rewards_lovelace, 0);
        assert_eq!(e.is_pool_operator, 0);
    }

    #[test]
    fn entry_three_is_pool_operator() {
        let out = ingest_stake(&fixture()).unwrap();
        let e = &out.stake_entries[3];
        assert_eq!(e.stake_credential_hash, [0x44; 28]);
        assert_eq!(e.delegated_pool, [0xAA; 28]);
        assert_eq!(e.rewards_lovelace, 100_000_000);
        assert_eq!(e.is_pool_operator, 1);
    }

    #[test]
    fn deterministic_across_runs() {
        let cbor = fixture();
        let a = ingest_stake(&cbor).unwrap();
        let b = ingest_stake(&cbor).unwrap();
        let a_json = serde_json::to_string(&a).unwrap();
        let b_json = serde_json::to_string(&b).unwrap();
        assert_eq!(a_json, b_json);
    }

    #[test]
    fn rejects_wrong_arity() {
        // 3-elem stake entry inside a 1-elem outer array.
        let buf = vec![0x81, 0x83, 0x40, 0x40, 0x40];
        assert!(ingest_stake(&buf).is_err());
    }
}
```

- [ ] **Step 4: Verify**

```bash
cargo test -p omega-commitment-ingest stake::tests 2>&1 | tail -10    # 5 tests pass
cargo test --workspace 2>&1 | tail -5    # 206
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-ingest/tests/fixtures/stake_snapshot.cbor \
        crates/omega-commitment-ingest/tests/fixtures/stake_snapshot.cbor.md \
        crates/omega-commitment-ingest/src/stake.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(ingest): stake-state ingestion + hand-crafted stake_snapshot.cbor fixture"
```

---

## Task 7: Governance fixture + ingestion

**Files:**
- Create: `crates/omega-commitment-ingest/tests/fixtures/governance_snapshot.cbor`
- Create: `crates/omega-commitment-ingest/tests/fixtures/governance_snapshot.cbor.md`
- Modify: `crates/omega-commitment-ingest/src/governance.rs`

Generate a hand-crafted CBOR fixture with 4 governance facts (one per kind: treasury, CC seat, ratified action, in-flight action). Implement `ingest_governance` parsing the format from A5: `[kind (u8), key (32), value (16-byte u128 BE), slot (u64)]`.

- [ ] **Step 1: Generate the fixture**

```bash
mkdir -p /tmp/gen_gov/src
cat > /tmp/gen_gov/Cargo.toml << 'EOF'
[package]
name = "gen_gov"
version = "0.1.0"
edition = "2021"
[[bin]]
name = "gen_gov"
path = "src/main.rs"
EOF
```

Write `/tmp/gen_gov/src/main.rs`:

```rust
use std::fs;

fn cbor_array_header(len: usize) -> Vec<u8> {
    if len < 24 { vec![0x80u8 | len as u8] } else if len < 256 { vec![0x98, len as u8] } else { vec![0x99, (len >> 8) as u8, len as u8] }
}
fn cbor_bytes_header(len: usize) -> Vec<u8> {
    if len < 24 { vec![0x40u8 | len as u8] } else if len < 256 { vec![0x58, len as u8] } else { vec![0x59, (len >> 8) as u8, len as u8] }
}
fn cbor_uint(v: u64) -> Vec<u8> {
    if v < 24 { vec![v as u8] } else if v <= 0xff { vec![0x18, v as u8] }
    else if v <= 0xffff { vec![0x19, (v >> 8) as u8, v as u8] }
    else if v <= 0xffff_ffff { let mut o = vec![0x1a]; o.extend_from_slice(&(v as u32).to_be_bytes()); o }
    else { let mut o = vec![0x1b]; o.extend_from_slice(&v.to_be_bytes()); o }
}
fn cbor_bytes(b: &[u8]) -> Vec<u8> { let mut o = cbor_bytes_header(b.len()); o.extend_from_slice(b); o }

fn fact(kind: u8, key: &[u8;32], value: u128, slot: u64) -> Vec<u8> {
    let mut o = cbor_array_header(4);
    o.extend(cbor_uint(kind as u64));
    o.extend(cbor_bytes(key));
    o.extend(cbor_bytes(&value.to_be_bytes()));
    o.extend(cbor_uint(slot));
    o
}

fn main() {
    let mut buf = Vec::new();
    buf.extend(cbor_array_header(4));
    // kind=0 treasury: key=zeros, value=lovelace balance
    buf.extend(fact(0, &[0u8; 32], 1_700_000_000_000_000u128, 100_000));
    // kind=1 CC seat: key=member credential, value=expiration epoch
    let mut cc_key = [0u8; 32];
    cc_key[..28].copy_from_slice(&[0x11; 28]);
    buf.extend(fact(1, &cc_key, 500u128, 100_000));
    // kind=2 ratified gov action: key=tx_id, value=packed type+slot
    let ratified_key = [0xAA; 32];
    let ratified_value = (1u128) | ((100_000u128) << 16);
    buf.extend(fact(2, &ratified_key, ratified_value, 100_000));
    // kind=3 in-flight gov action
    let inflight_key = [0xBB; 32];
    let inflight_value = (2u128) | ((100_001u128) << 16);
    buf.extend(fact(3, &inflight_key, inflight_value, 100_000));

    let dest = "/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/fixtures/governance_snapshot.cbor";
    fs::write(dest, &buf).unwrap();
    println!("wrote {} bytes", buf.len());
}
```

Run:

```bash
cd /tmp/gen_gov && cargo run --release
cd /home/hoskinson/omega-commitment
ls -la crates/omega-commitment-ingest/tests/fixtures/governance_snapshot.cbor
```

Expected: ~210-byte file.

- [ ] **Step 2: Document the fixture**

Write `/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/fixtures/governance_snapshot.cbor.md`:

```markdown
# `governance_snapshot.cbor`

Hand-crafted CBOR fixture with 4 governance facts, one per `kind`.

## Per-fact format

```
[ kind (u8), key (32 bytes), value (16-byte big-endian u128), slot (u64) ]
```

Note: u128 values come over CBOR as 16-byte bytestrings (CBOR has no
native u128). The fixture uses `value.to_be_bytes()` for serialization
and the ingestion parser uses `read_u128_bytes` for decoding.

## Contents

| # | kind | key | value | slot |
|---|---|---|---|---|
| 0 | 0 (treasury) | all-zero | 1_700_000_000_000_000 | 100_000 |
| 1 | 1 (CC seat) | `1111…1100…00` | 500 (epoch) | 100_000 |
| 2 | 2 (ratified) | `AAAA…AA` | packed(type=1, slot=100_000) | 100_000 |
| 3 | 3 (in-flight) | `BBBB…BB` | packed(type=2, slot=100_001) | 100_000 |

Covers all four kind discriminants.

## Regeneration

Generated by the helper in Task 7, Step 1 of
`2026-05-01-omega-ingest-mainnet-plan.md`.
```

- [ ] **Step 3: Replace `governance.rs`**

Replace `/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/src/governance.rs` with:

```rust
//! Governance-state sub-tree ingestion from a hand-crafted
//! governance_snapshot.cbor.
//!
//! Top-level CBOR array of 4-element entries:
//!   [ kind (u8), key (32 bytes), value (16-byte u128 big-endian),
//!     slot (u64) ]
//!
//! Maps 1:1 onto `omega_commitment_core::governance_state_leaf::GovernanceFact`.

use crate::cbor::{read_32_bytes, read_array_len, read_u128_bytes, read_u64, read_u8};
use anyhow::Result;
use omega_commitment_core::governance_state_leaf::GovernanceFact;
use pallas_codec::minicbor::Decoder;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GovernanceOutput {
    pub facts: Vec<GovernanceFact>,
}

pub fn ingest_governance(cbor: &[u8]) -> Result<GovernanceOutput> {
    let mut d = Decoder::new(cbor);
    let n = read_array_len(&mut d)?;
    let mut facts = Vec::with_capacity(n);
    for _ in 0..n {
        let arity = read_array_len(&mut d)?;
        if arity != 4 {
            return Err(anyhow::anyhow!(
                "governance fact must be 4-elem, got {arity}"
            ));
        }
        let kind = read_u8(&mut d)?;
        let key = read_32_bytes(&mut d)?;
        let value = read_u128_bytes(&mut d)?;
        let slot = read_u64(&mut d)?;
        facts.push(GovernanceFact { kind, key, value, slot });
    }
    Ok(GovernanceOutput { facts })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        std::fs::read("tests/fixtures/governance_snapshot.cbor").unwrap()
    }

    #[test]
    fn ingest_yields_four_facts() {
        let out = ingest_governance(&fixture()).unwrap();
        assert_eq!(out.facts.len(), 4);
    }

    #[test]
    fn all_four_kinds_present() {
        let out = ingest_governance(&fixture()).unwrap();
        let kinds: std::collections::HashSet<u8> = out.facts.iter().map(|f| f.kind).collect();
        for k in 0..=3 {
            assert!(kinds.contains(&k));
        }
    }

    #[test]
    fn treasury_fact_decoded_correctly() {
        let out = ingest_governance(&fixture()).unwrap();
        let treasury = out.facts.iter().find(|f| f.kind == 0).unwrap();
        assert_eq!(treasury.key, [0u8; 32]);
        assert_eq!(treasury.value, 1_700_000_000_000_000u128);
        assert_eq!(treasury.slot, 100_000);
    }

    #[test]
    fn deterministic_across_runs() {
        let cbor = fixture();
        let a = ingest_governance(&cbor).unwrap();
        let b = ingest_governance(&cbor).unwrap();
        let a_json = serde_json::to_string(&a).unwrap();
        let b_json = serde_json::to_string(&b).unwrap();
        assert_eq!(a_json, b_json);
    }

    #[test]
    fn rejects_wrong_arity() {
        let buf = vec![0x81, 0x82, 0x00, 0x40];
        assert!(ingest_governance(&buf).is_err());
    }
}
```

- [ ] **Step 4: Verify**

```bash
cargo test -p omega-commitment-ingest governance::tests 2>&1 | tail -10    # 5 tests pass
cargo test --workspace 2>&1 | tail -5    # 211
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

- [ ] **Step 5: Commit**

```bash
git add crates/omega-commitment-ingest/tests/fixtures/governance_snapshot.cbor \
        crates/omega-commitment-ingest/tests/fixtures/governance_snapshot.cbor.md \
        crates/omega-commitment-ingest/src/governance.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "feat(ingest): governance-state ingestion + hand-crafted governance_snapshot.cbor fixture"
```

---

## Task 8: End-to-end 5-sub-tree QA pipeline + integration tests

**Files:**
- Create: `crates/omega-commitment-ingest/tests/token_policy_ingest_integration.rs`
- Create: `crates/omega-commitment-ingest/tests/script_ingest_integration.rs`
- Create: `crates/omega-commitment-ingest/tests/stake_ingest_integration.rs`
- Create: `crates/omega-commitment-ingest/tests/governance_ingest_integration.rs`
- Modify: `crates/omega-commitment-ingest/tests/qa_pipeline.rs`

Per-sub-tree integration tests + extended end-to-end pipeline test that runs all 5 ingestion paths and asserts each produces a non-zero root.

- [ ] **Step 1: Write the four per-sub-tree integration tests**

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/token_policy_ingest_integration.rs`:

```rust
//! Integration test for token-policy ingestion against the extended fixture.

use omega_commitment_core::tree::MerkleTree;
use omega_commitment_ingest::token_policy::ingest_token_policies;
use std::{fs, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ledger_state_extended.cbor")
}

#[test]
fn extended_fixture_yields_two_policies_via_integration() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_token_policies(&cbor).unwrap();
    assert_eq!(out.policies.len(), 2);
}

#[test]
fn token_policy_root_is_nonzero() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_token_policies(&cbor).unwrap();
    let leaves: Vec<_> = out.policies.iter().map(|p| p.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_ne!(root, [0u8; 32]);
}
```

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/script_ingest_integration.rs`:

```rust
//! Integration test for script ingestion against the extended fixture.

use omega_commitment_core::tree::MerkleTree;
use omega_commitment_ingest::script::ingest_scripts;
use std::{fs, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ledger_state_extended.cbor")
}

#[test]
fn extended_fixture_yields_two_scripts_via_integration() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_scripts(&cbor).unwrap();
    assert_eq!(out.scripts.len(), 2);
}

#[test]
fn script_root_is_nonzero() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_scripts(&cbor).unwrap();
    let leaves: Vec<_> = out.scripts.iter().map(|s| s.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_ne!(root, [0u8; 32]);
}
```

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/stake_ingest_integration.rs`:

```rust
//! Integration test for stake ingestion against stake_snapshot.cbor.

use omega_commitment_core::tree::MerkleTree;
use omega_commitment_ingest::stake::ingest_stake;
use std::{fs, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/stake_snapshot.cbor")
}

#[test]
fn stake_fixture_yields_four_entries_via_integration() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_stake(&cbor).unwrap();
    assert_eq!(out.stake_entries.len(), 4);
}

#[test]
fn stake_root_is_nonzero() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_stake(&cbor).unwrap();
    let leaves: Vec<_> = out.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_ne!(root, [0u8; 32]);
}
```

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/governance_ingest_integration.rs`:

```rust
//! Integration test for governance ingestion against governance_snapshot.cbor.

use omega_commitment_core::tree::MerkleTree;
use omega_commitment_ingest::governance::ingest_governance;
use std::{fs, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/governance_snapshot.cbor")
}

#[test]
fn governance_fixture_yields_four_facts_via_integration() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_governance(&cbor).unwrap();
    assert_eq!(out.facts.len(), 4);
}

#[test]
fn governance_root_is_nonzero() {
    let cbor = fs::read(fixture_path()).unwrap();
    let out = ingest_governance(&cbor).unwrap();
    let leaves: Vec<_> = out.facts.iter().map(|f| f.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_ne!(root, [0u8; 32]);
}
```

- [ ] **Step 2: Extend `qa_pipeline.rs` with a 5-sub-tree pipeline test**

Append to `/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/qa_pipeline.rs`:

```rust

#[test]
fn full_pipeline_five_sub_trees_from_cbor() {
    use omega_commitment_ingest::{
        governance::ingest_governance, script::ingest_scripts, stake::ingest_stake,
        token_policy::ingest_token_policies,
    };

    let extended_cbor = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/ledger_state_extended.cbor")
    ).unwrap();
    let stake_cbor = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/stake_snapshot.cbor")
    ).unwrap();
    let gov_cbor = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/governance_snapshot.cbor")
    ).unwrap();

    // 1) UTXO sub-tree from extended fixture.
    let utxo_out = omega_commitment_ingest::utxo::ingest_utxos(&extended_cbor).unwrap();
    assert_eq!(utxo_out.utxos.len(), 4);
    let utxo_leaves: Vec<_> = utxo_out.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    let utxo_root = omega_commitment_core::tree::MerkleTree::build(utxo_leaves).root();
    assert_ne!(utxo_root, [0u8; 32]);

    // 2) Token-policy sub-tree derived from extended fixture.
    let tp_out = ingest_token_policies(&extended_cbor).unwrap();
    assert_eq!(tp_out.policies.len(), 2);
    let tp_leaves: Vec<_> = tp_out.policies.iter().map(|p| p.leaf_hash()).collect();
    let tp_root = omega_commitment_core::tree::MerkleTree::build(tp_leaves).root();
    assert_ne!(tp_root, [0u8; 32]);

    // 3) Script sub-tree derived from extended fixture.
    let s_out = ingest_scripts(&extended_cbor).unwrap();
    assert_eq!(s_out.scripts.len(), 2);
    let s_leaves: Vec<_> = s_out.scripts.iter().map(|s| s.leaf_hash()).collect();
    let s_root = omega_commitment_core::tree::MerkleTree::build(s_leaves).root();
    assert_ne!(s_root, [0u8; 32]);

    // 4) Stake sub-tree from stake_snapshot.cbor.
    let st_out = ingest_stake(&stake_cbor).unwrap();
    assert_eq!(st_out.stake_entries.len(), 4);
    let st_leaves: Vec<_> = st_out.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    let st_root = omega_commitment_core::tree::MerkleTree::build(st_leaves).root();
    assert_ne!(st_root, [0u8; 32]);

    // 5) Governance sub-tree from governance_snapshot.cbor.
    let g_out = ingest_governance(&gov_cbor).unwrap();
    assert_eq!(g_out.facts.len(), 4);
    let g_leaves: Vec<_> = g_out.facts.iter().map(|f| f.leaf_hash()).collect();
    let g_root = omega_commitment_core::tree::MerkleTree::build(g_leaves).root();
    assert_ne!(g_root, [0u8; 32]);

    // Sanity: each sub-tree root is distinct.
    let roots = [utxo_root, tp_root, s_root, st_root, g_root];
    for i in 0..roots.len() {
        for j in (i + 1)..roots.len() {
            assert_ne!(roots[i], roots[j], "sub-tree {} and {} produced same root", i, j);
        }
    }
}
```

- [ ] **Step 3: Verify**

```bash
cargo test -p omega-commitment-ingest 2>&1 | tail -15
cargo test --workspace 2>&1 | tail -5    # 220 (211 + 8 per-sub-tree integration + 1 qa_pipeline)
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-ingest/tests/token_policy_ingest_integration.rs \
        crates/omega-commitment-ingest/tests/script_ingest_integration.rs \
        crates/omega-commitment-ingest/tests/stake_ingest_integration.rs \
        crates/omega-commitment-ingest/tests/governance_ingest_integration.rs \
        crates/omega-commitment-ingest/tests/qa_pipeline.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test(ingest): per-sub-tree integration tests + 5-sub-tree end-to-end pipeline"
```

---

## Task 9: Pin ingestion-layer golden vectors

**Files:**
- Create: `crates/omega-commitment-ingest/tests/golden_ingest.rs`

Pin the per-sub-tree roots from CBOR fixtures and the canonical "v0.9.0 hybrid bundle" root tuple (5 sub-trees from CBOR + 2 from existing JSON fixtures).

Use the fail-once-then-pin workflow: insert `"deadbeef"` placeholders, run, capture actual hex from failure messages, replace placeholders.

- [ ] **Step 1: Create `golden_ingest.rs` with placeholders**

`/home/hoskinson/omega-commitment/crates/omega-commitment-ingest/tests/golden_ingest.rs`:

```rust
//! Ingestion-layer golden vectors: per-sub-tree roots from CBOR
//! fixtures, plus the canonical v0.9.0 hybrid bundle root tuple
//! (5 CBOR-derived sub-trees + 2 existing JSON fixtures).
//!
//! These are the canonical ingestion regression net. If any of these
//! drift, encoding or aggregation logic changed — investigate before
//! re-pinning.

use omega_commitment_bundle::bundle::assemble;
use omega_commitment_core::tree::MerkleTree;
use omega_commitment_ingest::{
    governance::ingest_governance, script::ingest_scripts, stake::ingest_stake,
    token_policy::ingest_token_policies, utxo::ingest_utxos,
};
use std::{fs, path::PathBuf};

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn extended_cbor() -> Vec<u8> {
    fs::read(manifest().join("tests/fixtures/ledger_state_extended.cbor")).unwrap()
}

fn stake_cbor() -> Vec<u8> {
    fs::read(manifest().join("tests/fixtures/stake_snapshot.cbor")).unwrap()
}

fn governance_cbor() -> Vec<u8> {
    fs::read(manifest().join("tests/fixtures/governance_snapshot.cbor")).unwrap()
}

#[test]
fn golden_utxo_root_from_extended_cbor() {
    let out = ingest_utxos(&extended_cbor()).unwrap();
    let leaves: Vec<_> = out.utxos.iter().map(|u| u.leaf_hash().unwrap()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "<INSERT_UTXO_ROOT_HEX_HERE>",
        "ingestion-layer UTXO root drifted"
    );
}

#[test]
fn golden_token_policy_root_from_extended_cbor() {
    let out = ingest_token_policies(&extended_cbor()).unwrap();
    let leaves: Vec<_> = out.policies.iter().map(|p| p.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "<INSERT_TOKEN_POLICY_ROOT_HEX_HERE>",
        "ingestion-layer token-policy root drifted"
    );
}

#[test]
fn golden_script_root_from_extended_cbor() {
    let out = ingest_scripts(&extended_cbor()).unwrap();
    let leaves: Vec<_> = out.scripts.iter().map(|s| s.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "<INSERT_SCRIPT_ROOT_HEX_HERE>",
        "ingestion-layer script root drifted"
    );
}

#[test]
fn golden_stake_root_from_cbor() {
    let out = ingest_stake(&stake_cbor()).unwrap();
    let leaves: Vec<_> = out.stake_entries.iter().map(|s| s.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "<INSERT_STAKE_ROOT_HEX_HERE>",
        "ingestion-layer stake root drifted"
    );
}

#[test]
fn golden_governance_root_from_cbor() {
    let out = ingest_governance(&governance_cbor()).unwrap();
    let leaves: Vec<_> = out.facts.iter().map(|f| f.leaf_hash()).collect();
    let root = MerkleTree::build(leaves).root();
    assert_eq!(
        hex::encode(root),
        "<INSERT_GOVERNANCE_ROOT_HEX_HERE>",
        "ingestion-layer governance root drifted"
    );
}

/// Run all 5 ingestion paths, write per-sub-tree JSON to a tempdir,
/// copy the 2 existing JSON fixtures (header, tx_index) over, then
/// run `omega-commitment-bundle::assemble` and pin the resulting
/// dual-track bundle root tuple.
#[test]
fn golden_hybrid_bundle_roots() {
    let dir = tempfile::tempdir().unwrap();

    // 5 sub-trees from CBOR ingestion.
    let utxo = ingest_utxos(&extended_cbor()).unwrap();
    fs::write(dir.path().join("utxo.json"), serde_json::to_string(&utxo).unwrap()).unwrap();
    let tp = ingest_token_policies(&extended_cbor()).unwrap();
    fs::write(dir.path().join("token_policy.json"), serde_json::to_string(&tp).unwrap()).unwrap();
    let s = ingest_scripts(&extended_cbor()).unwrap();
    fs::write(dir.path().join("script.json"), serde_json::to_string(&s).unwrap()).unwrap();
    let st = ingest_stake(&stake_cbor()).unwrap();
    fs::write(dir.path().join("stake.json"), serde_json::to_string(&st).unwrap()).unwrap();
    let g = ingest_governance(&governance_cbor()).unwrap();
    fs::write(dir.path().join("governance.json"), serde_json::to_string(&g).unwrap()).unwrap();

    // 2 sub-trees from existing JSON fixtures (header + tx-index).
    let core_fixtures = manifest().parent().unwrap()
        .join("omega-commitment-core/tests/fixtures");
    fs::copy(
        core_fixtures.join("header_chain_small.json"),
        dir.path().join("header.json"),
    ).unwrap();
    fs::copy(
        core_fixtures.join("tx_index_small.json"),
        dir.path().join("tx_index.json"),
    ).unwrap();

    let bundle = assemble(dir.path()).unwrap();
    assert_eq!(
        hex::encode(bundle.blake2b_bundle_root),
        "<INSERT_HYBRID_BLAKE2B_ROOT_HEX_HERE>",
        "v0.9.0 hybrid blake2b_bundle_root drifted"
    );
    assert_eq!(
        hex::encode(bundle.sha3_bundle_root),
        "<INSERT_HYBRID_SHA3_ROOT_HEX_HERE>",
        "v0.9.0 hybrid sha3_bundle_root drifted"
    );
}
```

Note: `omega-commitment-bundle` needs to be a dev-dep of `omega-commitment-ingest`. Add it to `crates/omega-commitment-ingest/Cargo.toml` `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3"
omega-commitment-bundle = { path = "../omega-commitment-bundle" }
```

- [ ] **Step 2: Run with placeholders, capture actual hex from failures, paste back, re-run**

```bash
cargo test -p omega-commitment-ingest --test golden_ingest 2>&1 | tail -40
```

Each test will fail with `assertion 'left == right' failed`. The "left" side is the actual hex; "right" is `"<INSERT_..._HERE>"`. From the failure messages, capture each actual hex string and replace each placeholder.

After replacing all 7 placeholders (5 sub-tree roots + 2 hybrid bundle roots), re-run:

```bash
cargo test -p omega-commitment-ingest --test golden_ingest 2>&1 | tail -10
```

Expected: 6 tests pass.

- [ ] **Step 3: Verify the full workspace**

```bash
cargo test --workspace 2>&1 | tail -5    # 226 (220 + 6 golden_ingest)
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-ingest/Cargo.toml \
        crates/omega-commitment-ingest/tests/golden_ingest.rs
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "test(qa): pin v0.9.0 ingestion-layer golden roots + hybrid bundle root tuple"
```

---

## Task 10: Bump workspace to v0.9.0 + extend README

**Files:**
- Modify: `crates/omega-commitment-core/Cargo.toml`
- Modify: `crates/omega-commitment-cli/Cargo.toml`
- Modify: `crates/omega-commitment-bundle/Cargo.toml`
- Modify: `crates/omega-commitment-ingest/Cargo.toml`
- Modify: `README.md`

- [ ] **Step 1: Bump versions**

In each of the four crate `Cargo.toml` files: change `version = "0.8.0"` to `version = "0.9.0"`.

- [ ] **Step 2: Verify**

```bash
cargo build --workspace 2>&1 | grep -E "warning|error"   # empty
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
cargo test --workspace 2>&1 | tail -5    # 226
```

- [ ] **Step 3: Append to README**

Append to `/home/hoskinson/omega-commitment/README.md`:

```markdown
## v0.9.0 — Four scaffolded ingestion paths implemented

The `unimplemented!()` markers from v0.8.0's token-policy, script, stake, and governance ingestion modules are now real implementations against richer hand-crafted CBOR fixtures. All five LedgerState-derivable sub-trees have working CBOR-fixture ingestion paths end-to-end, and ingestion-layer golden vectors are pinned for regression detection.

### What changed

- **Extended UTXO CBOR fixture** (`tests/fixtures/ledger_state_extended.cbor`) — 6-element format adds multi-asset bundles and optional script credentials per UTXO.
- **UTXO ingestion** is now backwards-compatible: it accepts both the v0.8.0 minimal 4-element format and the v0.9.0 extended 6-element format. Output JSON shape is unchanged (the extension fields are consumed by token-policy and script ingestion, not surfaced in the UTXO sub-tree).
- **Token-policy ingestion** walks the extended UTXO fixture's multi-asset bundles, sums quantities per `policy_id` across all UTXOs, and emits a deduplicated, sorted policy list. `first_issuance_slot` is pinned to 0 (synthetic-fixture limitation).
- **Script ingestion** walks the extended UTXO fixture's `script_credential` fields, deduplicates by `script_hash`, and emits the script registry. `deployment_slot` is pinned to 0 (same limitation).
- **Stake ingestion** parses a new dedicated CBOR fixture `tests/fixtures/stake_snapshot.cbor` (4 entries covering undelegated / pool-only / pool+DRep / pool-operator).
- **Governance ingestion** parses a new dedicated CBOR fixture `tests/fixtures/governance_snapshot.cbor` (4 facts, one per `kind`: treasury / CC seat / ratified action / in-flight action).
- **End-to-end pipeline test** runs all 5 sub-trees through ingestion → leaf hashes → root, asserts each root is non-zero and distinct.
- **Ingestion-layer golden vectors** pinned in `crates/omega-commitment-ingest/tests/golden_ingest.rs`: 5 per-sub-tree roots + the canonical "hybrid" bundle root tuple (5 from CBOR + header & tx-index from existing JSON fixtures).

### What v0.9.0 still does NOT do

Real Conway-era LedgerState parsing remains future work. v0.9.0's CBOR fixtures are hand-crafted simplified formats — sufficient to exercise every ingestion code path deterministically in CI, but not the same shape as a Mithril snapshot or a `cardano-node` LedgerState dump. The `scripts/download_snapshot.sh` script can fetch real Mithril preview-testnet data for human inspection, but `omega-ingest` cannot yet parse it.

The v1.0 plan in this track will:
- Either depend on `pallas-traverse`'s evolving Conway-LedgerState parsers, or
- Use a third-party indexer (Koios / Blockfrost) to extract sub-tree data via REST.

This decision is deferred until pallas-traverse's Conway support stabilizes or until empirical experience shows a REST indexer is the more reliable path.

### Sub-trees status

| # | Sub-tree | Commitment layer | Ingestion (CBOR fixture) | Real-snapshot |
|---|---|---|---|---|
| 1 | UTXO | ✅ Shipped (v0.1.0) | ✅ Shipped (v0.8.0+) | 🟡 v1.0 |
| 2 | Block header chain | ✅ Shipped (v0.2.0) | ❌ Chain-follower req'd | ❌ v1.0+ |
| 3 | Transaction index | ✅ Shipped (v0.3.0) | ❌ Chain-follower req'd | ❌ v1.0+ |
| 4 | Native token policies | ✅ Shipped (v0.4.0) | ✅ **Shipped (v0.9.0)** | 🟡 v1.0 |
| 5 | Script registry | ✅ Shipped (v0.5.0) | ✅ **Shipped (v0.9.0)** | 🟡 v1.0 |
| 6 | Stake state | ✅ Shipped (v0.6.0) | ✅ **Shipped (v0.9.0)** | 🟡 v1.0 |
| 7 | Governance state | ✅ Shipped (v0.6.0) | ✅ **Shipped (v0.9.0)** | 🟡 v1.0 |
```

- [ ] **Step 4: Commit**

```bash
git add crates/omega-commitment-core/Cargo.toml \
        crates/omega-commitment-cli/Cargo.toml \
        crates/omega-commitment-bundle/Cargo.toml \
        crates/omega-commitment-ingest/Cargo.toml \
        README.md
git -c user.email="charles.hoskinson@gmail.com" -c user.name="charles hoskinson" commit -m "chore: bump workspace to 0.9.0; document ingestion completion of LedgerState-derivable sub-trees"
```

- [ ] **Step 5: Final verification**

```bash
git log --oneline | head -12
cargo test --workspace 2>&1 | tail -5
cargo lint 2>&1 | tail -3
cargo fmt-check 2>&1 | tail -3
```

Expected: HEAD is the version-bump commit, 226 tests pass, lint+fmt clean.

---

## Self-review

**Coverage of the user's ask:**
- ✅ Implement four scaffolded ingestion paths (token-policy, script, stake, governance) — Tasks 4, 5, 6, 7.
- ✅ Lock ingestion-layer golden vectors — Task 9.
- ✅ End-to-end pipeline test for all 5 LedgerState-derivable sub-trees — Task 8.
- 🟡 Real Mithril snapshot parsing — explicitly deferred to v1.0 with documented rationale.

**Decision honoring:**
- ✅ PQ-only crypto — unchanged.
- ✅ Plonky3-friendly tree — unchanged.
- ✅ Selective dual-track at bundle layer — golden bundle test exercises both blake2b + sha3 bundle roots.
- ✅ Lazy/pull migration — unchanged.

**Placeholder scan:** All code blocks runnable. The 7 hex placeholders in Task 9 are explicitly part of the documented "fail-once-then-pin" workflow. ✅

**Type consistency:**
- `TokenPolicyOutput` / `ScriptOutput` / `StakeOutput` / `GovernanceOutput` consistent with v0.8.0 scaffolds.
- Function signatures (`ingest_token_policies`, `ingest_scripts`, `ingest_stake`, `ingest_governance`) match the v0.8.0 stubs.
- The CLI binary needs no changes — it already dispatches to these functions; the v0.8.0 scaffold versions returned `unimplemented!()` and now return real values, but the call sites are identical.
- `omega-commitment-bundle` becomes a dev-dep of `omega-commitment-ingest` for the golden_ingest test.
- ✅ No drift.

**Bite-sized tasks:** 10 tasks, each with 3–5 numbered steps; each step is a single action.

**Net delta:** ~37 new tests (+1 cbor batch, +3 utxo, +5 token_policy, +5 script, +5 stake, +5 governance, +8 per-sub-tree integration, +1 qa_pipeline extension, +6 golden_ingest). 189 → ~226. 10 commits.

---

## What's NOT in this plan (and why)

- **Real Conway-era LedgerState CBOR parsing.** Defers to v1.0 once pallas-traverse Conway support stabilizes (or until we decide to use a REST indexer instead).
- **Real Mithril snapshot ingestion.** Same reason; the download script is in place but parsing is deferred.
- **Cross-validation between sub-trees** (e.g., "every CC seat in governance corresponds to a known credential in stake"). Future plan.
- **Header + tx-index ingestion.** Requires a chain-follower; out of scope for both v0.9.0 and v1.0's first cut.
- **`first_issuance_slot` and `deployment_slot` accuracy.** Pinned to 0 in v0.9.0 because the simplified fixtures don't carry this data. Real-data ingestion in v1.0 will populate them from chain history.

---

## How to execute this plan

Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans`. Ten tasks, each independently committable. Total runway: 5–7 days.

Expected post-execution state:
- 10 commits added (74 → 84)
- ~37 new tests (189 → 226), zero `#[ignore]`d
- All four crates at v0.9.0
- 5 of 7 sub-trees have working CBOR-fixture ingestion + golden vectors
- Hybrid bundle root tuple pinned as the v0.9.0 ingestion-layer canonical Ω-Commitment

Next plan in this track: `omega-commitment-ingest-v1` — real Mithril/LedgerState parsing.
