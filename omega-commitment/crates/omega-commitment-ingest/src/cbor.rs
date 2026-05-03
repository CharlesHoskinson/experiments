//! Minimal CBOR navigation helpers.
//!
//! For v0.8.0 we parse a hand-crafted simplified CBOR fixture
//! (see `tests/fixtures/ledger_state_minimal.cbor.md`). When real
//! Mithril/LedgerState snapshot ingestion lands in a later release,
//! this module will be expanded with `pallas-traverse`-based readers
//! for the full Conway-era LedgerState shape.

use anyhow::{anyhow, Result};
use pallas_codec::minicbor::Decoder;

/// Read a 32-byte fixed-length byte string from a `Decoder` cursor.
/// Returns Err if the next item is not exactly 32 bytes.
pub fn read_32_bytes<'b>(d: &mut Decoder<'b>) -> Result<[u8; 32]> {
    let bytes = d
        .bytes()
        .map_err(|e| anyhow!("cbor: expected bytes ({e})"))?;
    if bytes.len() != 32 {
        return Err(anyhow!(
            "cbor: expected 32-byte string, got {}",
            bytes.len()
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(bytes);
    Ok(out)
}

/// Read a u64 from a `Decoder` cursor.
pub fn read_u64<'b>(d: &mut Decoder<'b>) -> Result<u64> {
    d.u64().map_err(|e| anyhow!("cbor: expected u64 ({e})"))
}

/// Read a u32 (encoded as u64 in CBOR) from a `Decoder` cursor.
pub fn read_u32<'b>(d: &mut Decoder<'b>) -> Result<u32> {
    let v = read_u64(d)?;
    u32::try_from(v).map_err(|_| anyhow!("cbor: u64 value {v} too large for u32"))
}

/// Expect an array of length `expected` next on the cursor.
pub fn expect_array<'b>(d: &mut Decoder<'b>, expected: usize) -> Result<()> {
    let actual = d
        .array()
        .map_err(|e| anyhow!("cbor: expected array ({e})"))?
        .ok_or_else(|| anyhow!("cbor: expected definite-length array"))?;
    if actual as usize != expected {
        return Err(anyhow!("cbor: expected array of {expected}, got {actual}"));
    }
    Ok(())
}

/// Read an array header and return the length (definite-length only).
pub fn read_array_len<'b>(d: &mut Decoder<'b>) -> Result<usize> {
    let len = d
        .array()
        .map_err(|e| anyhow!("cbor: expected array ({e})"))?
        .ok_or_else(|| anyhow!("cbor: expected definite-length array"))?;
    Ok(len as usize)
}

/// Read a 28-byte fixed-length byte string (Cardano Blake3-224 hash).
pub fn read_28_bytes<'b>(d: &mut Decoder<'b>) -> Result<[u8; 28]> {
    let bytes = d
        .bytes()
        .map_err(|e| anyhow!("cbor: expected bytes ({e})"))?;
    if bytes.len() != 28 {
        return Err(anyhow!(
            "cbor: expected 28-byte string, got {}",
            bytes.len()
        ));
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
        return Err(anyhow!(
            "cbor: expected 16-byte u128 string, got {}",
            bytes.len()
        ));
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
    let ty = d
        .datatype()
        .map_err(|e| anyhow!("cbor: peek failed ({e})"))?;
    if ty == Type::Null {
        d.null().map_err(|e| anyhow!("cbor: consume null ({e})"))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Assert the decoder cursor is at end-of-input. Returns Err if there
/// are trailing bytes beyond the parsed structure.
pub fn expect_end<'b>(d: &Decoder<'b>, total_len: usize) -> Result<()> {
    let pos = d.position();
    if pos != total_len {
        return Err(anyhow!(
            "cbor: trailing bytes after parse: position {} of {} (extra {} bytes)",
            pos,
            total_len,
            total_len - pos
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_32_bytes_succeeds_on_correct_length() {
        // CBOR for 32-byte string of 0x11s: 0x58 0x20 [32 × 0x11]
        let mut buf = vec![0x58, 0x20];
        buf.extend_from_slice(&[0x11; 32]);
        let mut d = Decoder::new(&buf);
        let out = read_32_bytes(&mut d).unwrap();
        assert_eq!(out, [0x11; 32]);
    }

    #[test]
    fn read_32_bytes_fails_on_wrong_length() {
        // 4-byte string instead of 32.
        let buf = vec![0x44, 0xDE, 0xAD, 0xBE, 0xEF];
        let mut d = Decoder::new(&buf);
        assert!(read_32_bytes(&mut d).is_err());
    }

    #[test]
    fn read_u64_handles_small_int() {
        let buf = vec![0x05]; // CBOR uint 5
        let mut d = Decoder::new(&buf);
        assert_eq!(read_u64(&mut d).unwrap(), 5);
    }

    #[test]
    fn read_array_len_reads_array_header() {
        let buf = vec![0x83]; // CBOR array of 3
        let mut d = Decoder::new(&buf);
        assert_eq!(read_array_len(&mut d).unwrap(), 3);
    }

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
        assert_eq!(
            read_var_bytes(&mut d).unwrap(),
            vec![0xDE, 0xAD, 0xBE, 0xEF]
        );
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

    #[test]
    fn expect_end_succeeds_on_exact_consume() {
        let buf = vec![0x05]; // a single u64
        let mut d = Decoder::new(&buf);
        let _ = read_u64(&mut d).unwrap();
        expect_end(&d, buf.len()).unwrap();
    }

    #[test]
    fn expect_end_fails_on_trailing_garbage() {
        let buf = vec![0x05, 0xFF, 0xAB]; // u64 + 2 trailing bytes
        let mut d = Decoder::new(&buf);
        let _ = read_u64(&mut d).unwrap();
        assert!(expect_end(&d, buf.len()).is_err());
    }
}
