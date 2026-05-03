//! Reusable serde adapters for hash-typed fields.
//!
//! - `hex_vec_hash`: serialize/deserialize `Vec<Hash>` as JSON array of
//!   lowercase hex strings.
//! - `opt_hex`: serialize/deserialize `Option<[u8; 32]>` as a hex string
//!   or `null`.

use crate::hash::Hash;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Serde adapter for `Vec<Hash>` -> JSON array of hex strings.
///
/// Use as `#[serde(with = "crate::serde_helpers::hex_vec_hash")]`.
pub mod hex_vec_hash {
    use super::*;

    pub fn serialize<S: Serializer>(v: &[Hash], s: S) -> Result<S::Ok, S::Error> {
        let strs: Vec<String> = v.iter().map(hex::encode).collect();
        strs.serialize(s)
    }

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

/// Serde adapter for `Option<[u8; 32]>` -> hex string or null.
///
/// Use as `#[serde(with = "crate::serde_helpers::opt_hex", default)]`.
pub mod opt_hex {
    use super::*;

    pub fn serialize<S: Serializer>(v: &Option<[u8; 32]>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            None => s.serialize_none(),
            Some(bytes) => s.serialize_some(&hex::encode(bytes)),
        }
    }

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
