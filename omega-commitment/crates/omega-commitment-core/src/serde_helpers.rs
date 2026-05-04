//! Internal serde adapters for hash-typed fields. Not part of the
//! public API; exposed `pub` only because Rust's visibility model
//! cannot express "pub within crate, private outside" for `#[serde(with
//! = ...)]` paths.
#![doc(hidden)]

use crate::hash::Hash;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Serde adapter: `Vec<Hash>` ↔ JSON array of lowercase hex strings.
///
/// Use as `#[serde(with = "crate::serde_helpers::hex_vec_hash")]`.
#[doc(hidden)]
pub mod hex_vec_hash {
    use super::*;

    #[doc(hidden)]
    pub fn serialize<S: Serializer>(v: &[Hash], s: S) -> Result<S::Ok, S::Error> {
        let strs: Vec<String> = v.iter().map(hex::encode).collect();
        strs.serialize(s)
    }

    #[doc(hidden)]
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Hash>, D::Error> {
        let strs: Vec<String> = Vec::deserialize(d)?;
        strs.iter()
            .map(|s| {
                let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
                if bytes.len() != 32 {
                    return Err(serde::de::Error::custom("hash must be 32 bytes"));
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                Ok(arr)
            })
            .collect()
    }
}

/// Serde adapter for `u128` that survives buffered/internally-tagged
/// deserialization in `serde_json`.
///
/// Default `serde_json` cannot dispatch `deserialize_u128` from the
/// internally-buffered `Content` path that `#[serde(tag = "...")]` enums
/// rely on. Routing the field through this adapter encodes u128 as a
/// JSON string and accepts either JSON string or JSON number on
/// deserialize, sidestepping the buffered-path limitation.
///
/// Use as `#[serde(with = "crate::serde_helpers::u128_dec")]`.
#[doc(hidden)]
pub mod u128_dec {
    use serde::{de, Deserializer, Serializer};
    use std::fmt;

    #[doc(hidden)]
    pub fn serialize<S: Serializer>(v: &u128, s: S) -> Result<S::Ok, S::Error> {
        // Encode as a decimal string so the wire form fits in any JSON
        // consumer regardless of u128 support. (Cardano u128 values can
        // exceed 2^64 — see the governance ratified-action packed value.)
        s.serialize_str(&v.to_string())
    }

    #[doc(hidden)]
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u128, D::Error> {
        struct V;
        impl<'de> de::Visitor<'de> for V {
            type Value = u128;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a u128 as a decimal string or JSON number")
            }
            fn visit_str<E: de::Error>(self, s: &str) -> Result<u128, E> {
                s.parse::<u128>().map_err(de::Error::custom)
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<u128, E> {
                Ok(v as u128)
            }
            fn visit_u128<E: de::Error>(self, v: u128) -> Result<u128, E> {
                Ok(v)
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<u128, E> {
                u128::try_from(v).map_err(de::Error::custom)
            }
            fn visit_i128<E: de::Error>(self, v: i128) -> Result<u128, E> {
                u128::try_from(v).map_err(de::Error::custom)
            }
            fn visit_string<E: de::Error>(self, s: String) -> Result<u128, E> {
                s.parse::<u128>().map_err(de::Error::custom)
            }
        }
        // `deserialize_any` lets us accept both JSON number and JSON string
        // forms; the Content-buffered path used by tagged enums will dispatch
        // to whichever one survived buffering.
        d.deserialize_any(V)
    }
}

/// Serde adapter: `Option<[u8; 32]>` ↔ hex string or `null`.
///
/// Use as `#[serde(with = "crate::serde_helpers::opt_hex", default)]`.
#[doc(hidden)]
pub mod opt_hex {
    use super::*;

    #[doc(hidden)]
    pub fn serialize<S: Serializer>(v: &Option<[u8; 32]>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            None => s.serialize_none(),
            Some(bytes) => s.serialize_some(&hex::encode(bytes)),
        }
    }

    #[doc(hidden)]
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<[u8; 32]>, D::Error> {
        let opt: Option<String> = Option::deserialize(d)?;
        match opt {
            None => Ok(None),
            Some(s) => {
                let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
                if bytes.len() != 32 {
                    return Err(serde::de::Error::custom("hash must be 32 bytes"));
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                Ok(Some(arr))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct VecWrapper(#[serde(with = "hex_vec_hash")] Vec<Hash>);

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct OptWrapper {
        #[serde(with = "opt_hex", default)]
        h: Option<[u8; 32]>,
    }

    #[test]
    fn hex_vec_hash_roundtrip() {
        let original = VecWrapper(vec![[0x11u8; 32], [0x22u8; 32]]);
        let s = serde_json::to_string(&original).unwrap();
        assert!(s.contains("\"1111111111"));
        let parsed: VecWrapper = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn opt_hex_some_roundtrip() {
        let original = OptWrapper {
            h: Some([0xAAu8; 32]),
        };
        let s = serde_json::to_string(&original).unwrap();
        assert!(s.contains("\"aaaaaaaa"));
        let parsed: OptWrapper = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn opt_hex_none_roundtrip() {
        let original = OptWrapper { h: None };
        let s = serde_json::to_string(&original).unwrap();
        assert!(s.contains("null"));
        let parsed: OptWrapper = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn opt_hex_default_when_field_missing() {
        let parsed: OptWrapper = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed, OptWrapper { h: None });
    }

    #[test]
    fn hex_vec_hash_rejects_wrong_length() {
        let bad = "[\"abcd\"]";
        let r: Result<VecWrapper, _> = serde_json::from_str(bad);
        assert!(r.is_err(), "should reject 2-byte hex as Hash");
    }

    #[test]
    fn opt_hex_rejects_wrong_length() {
        let bad = "{\"h\":\"abcd\"}";
        let r: Result<OptWrapper, _> = serde_json::from_str(bad);
        assert!(r.is_err(), "should reject 2-byte hex as Hash");
    }
}
