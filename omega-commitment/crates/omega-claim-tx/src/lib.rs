//! Claim transaction wire types for the Omega proof experiment harness.

use minicbor::{Decoder, Encoder};
use omega_commitment_core::hash::{blake3_256, Hash};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use thiserror::Error;

const WIRE_VERSION: u64 = 1;
const VARIANT_UTXO: u64 = 0;
const VARIANT_COLLECTION: u64 = 1;
const CHECKSUM_LEN: usize = 32;

type CborEncodeError = minicbor::encode::Error<Infallible>;

/// A claim transaction accepted by the proof experiment harness.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClaimTx {
    /// A single-leaf UTxO claim.
    Utxo(ClaimUtxo),
    /// A batched claim carrying one folded proof for all public inputs.
    Collection(ClaimCollection),
}

/// A single-leaf claim.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimUtxo {
    /// Public inputs visible to the verifier and ledger.
    pub public: ClaimPublicInputs,
    /// Private witness material used by the prover.
    pub witness: ClaimWitness,
    /// Plonky3 STARK proof bytes.
    pub proof: ProofBytes,
}

/// Public claim inputs committed into consensus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimPublicInputs {
    /// Ω-Commitment sub-tree identifier.
    pub sub_tree_id: u8,
    /// Canonical leaf index in the sub-tree.
    pub leaf_index: u64,
    /// Blake3 bundle root from the published Ω-Commitment.
    #[serde(with = "hex::serde")]
    pub bundle_root_blake3: Hash,
    /// Replay-prevention nullifier.
    #[serde(with = "hex::serde")]
    pub nullifier: Hash,
    /// Starstream recipient address.
    #[serde(with = "hex::serde")]
    pub recipient_starstream_addr: Hash,
}

/// Private witness data carried by the v0.1 harness.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimWitness {
    /// Canonical leaf payload bytes.
    #[serde(with = "hex::serde")]
    pub leaf_payload: Vec<u8>,
    /// Sibling hashes from leaf to root.
    #[serde(with = "omega_commitment_core::serde_helpers::hex_vec_hash")]
    pub merkle_path: Vec<Hash>,
    /// Prototype signing-key proof bytes.
    #[serde(with = "hex::serde")]
    pub signing_key_proof: Vec<u8>,
}

/// A batched claim. v0.1 carries one proof for the whole collection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimCollection {
    /// Public inputs for each claimed leaf.
    pub public: Vec<ClaimPublicInputs>,
    /// Witnesses for each claimed leaf.
    pub witness: Vec<ClaimWitness>,
    /// Folded proof bytes for the collection.
    pub proof: ProofBytes,
}

/// Opaque Plonky3 proof bytes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct ProofBytes(#[serde(with = "hex::serde")] pub Vec<u8>);

/// Errors returned by the claim transaction CBOR codec.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum CborError {
    /// CBOR encoding failed.
    #[error("claim tx cbor encode failed: {0}")]
    Encode(String),
    /// CBOR decoding failed.
    #[error("claim tx cbor decode failed: {0}")]
    Decode(String),
    /// The envelope version is not supported by this crate.
    #[error("unsupported claim tx wire version {0}")]
    UnsupportedVersion(u64),
    /// A checksum inside the wire envelope did not match the payload.
    #[error("claim tx cbor checksum mismatch")]
    ChecksumMismatch,
    /// The decoded payload re-encoded to different bytes.
    #[error("claim tx cbor payload is not canonical")]
    NonCanonicalPayload,
    /// The decoded value left bytes unread.
    #[error("claim tx cbor trailing bytes: position {position} of {total}")]
    TrailingBytes { position: usize, total: usize },
    /// A fixed-size byte string had the wrong length.
    #[error("claim tx cbor field {field} expected {expected} bytes, got {actual}")]
    InvalidByteLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    /// A fixed-size array had the wrong length.
    #[error("claim tx cbor array {field} expected {expected} entries, got {actual}")]
    InvalidArrayLength {
        field: &'static str,
        expected: u64,
        actual: u64,
    },
    /// The codec rejected an indefinite-length array.
    #[error("claim tx cbor array {field} must use definite length")]
    IndefiniteArray { field: &'static str },
    /// The claim transaction variant tag is not known.
    #[error("unknown claim tx variant {0}")]
    UnknownVariant(u64),
    /// Collection public inputs and witnesses differ in length.
    #[error("collection arity mismatch: public={public}, witness={witness}")]
    CollectionArityMismatch { public: usize, witness: usize },
}

impl From<CborEncodeError> for CborError {
    fn from(error: CborEncodeError) -> Self {
        Self::Encode(error.to_string())
    }
}

impl From<minicbor::decode::Error> for CborError {
    fn from(error: minicbor::decode::Error) -> Self {
        Self::Decode(error.to_string())
    }
}

impl ClaimTx {
    /// Encode this claim transaction as canonical CBOR.
    ///
    /// The public wire form is a versioned CBOR envelope:
    /// `[version, payload, blake3(payload)]`, where `payload` is the
    /// canonical fixed-order CBOR encoding of the claim.
    ///
    /// # Errors
    ///
    /// Returns an error if CBOR encoding fails.
    pub fn to_cbor(&self) -> Result<Vec<u8>, CborError> {
        self.validate()?;
        let payload = encode_payload(self)?;
        let checksum = blake3_256(&payload);

        let mut encoder = Encoder::new(Vec::new());
        encoder
            .array(3)?
            .u64(WIRE_VERSION)?
            .bytes(&payload)?
            .bytes(&checksum)?;
        Ok(encoder.into_writer())
    }

    /// Decode a claim transaction from canonical CBOR.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed CBOR, unsupported versions,
    /// checksum mismatch, non-canonical payloads, and shape violations.
    pub fn from_cbor(bytes: &[u8]) -> Result<Self, CborError> {
        let mut decoder = Decoder::new(bytes);
        expect_array(&mut decoder, "envelope", 3)?;
        let version = decoder.u64()?;
        if version != WIRE_VERSION {
            return Err(CborError::UnsupportedVersion(version));
        }
        let payload = decoder.bytes()?.to_vec();
        let checksum = read_hash(&mut decoder, "checksum")?;
        expect_end(&decoder, bytes.len())?;

        if blake3_256(&payload) != checksum {
            return Err(CborError::ChecksumMismatch);
        }

        let decoded = decode_payload(&payload)?;
        if encode_payload(&decoded)? != payload {
            return Err(CborError::NonCanonicalPayload);
        }

        Ok(decoded)
    }

    fn validate(&self) -> Result<(), CborError> {
        if let ClaimTx::Collection(claim) = self {
            if claim.public.len() != claim.witness.len() {
                return Err(CborError::CollectionArityMismatch {
                    public: claim.public.len(),
                    witness: claim.witness.len(),
                });
            }
        }

        Ok(())
    }
}

fn encode_payload(tx: &ClaimTx) -> Result<Vec<u8>, CborError> {
    let mut encoder = Encoder::new(Vec::new());
    encode_claim_tx(tx, &mut encoder)?;
    Ok(encoder.into_writer())
}

fn encode_claim_tx(tx: &ClaimTx, encoder: &mut Encoder<Vec<u8>>) -> Result<(), CborEncodeError> {
    encoder.array(2)?;
    match tx {
        ClaimTx::Utxo(claim) => {
            encoder.u64(VARIANT_UTXO)?;
            encode_utxo(claim, encoder)?;
        }
        ClaimTx::Collection(claim) => {
            encoder.u64(VARIANT_COLLECTION)?;
            encode_collection(claim, encoder)?;
        }
    }
    Ok(())
}

fn encode_utxo(claim: &ClaimUtxo, encoder: &mut Encoder<Vec<u8>>) -> Result<(), CborEncodeError> {
    encoder.array(3)?;
    encode_public_inputs(&claim.public, encoder)?;
    encode_witness(&claim.witness, encoder)?;
    encoder.bytes(&claim.proof.0)?;
    Ok(())
}

fn encode_collection(
    claim: &ClaimCollection,
    encoder: &mut Encoder<Vec<u8>>,
) -> Result<(), CborEncodeError> {
    encoder.array(3)?;
    encoder.array(claim.public.len() as u64)?;
    for public in &claim.public {
        encode_public_inputs(public, encoder)?;
    }
    encoder.array(claim.witness.len() as u64)?;
    for witness in &claim.witness {
        encode_witness(witness, encoder)?;
    }
    encoder.bytes(&claim.proof.0)?;
    Ok(())
}

fn encode_public_inputs(
    public: &ClaimPublicInputs,
    encoder: &mut Encoder<Vec<u8>>,
) -> Result<(), CborEncodeError> {
    encoder
        .array(5)?
        .u8(public.sub_tree_id)?
        .u64(public.leaf_index)?
        .bytes(&public.bundle_root_blake3)?
        .bytes(&public.nullifier)?
        .bytes(&public.recipient_starstream_addr)?;
    Ok(())
}

fn encode_witness(
    witness: &ClaimWitness,
    encoder: &mut Encoder<Vec<u8>>,
) -> Result<(), CborEncodeError> {
    encoder
        .array(3)?
        .bytes(&witness.leaf_payload)?
        .array(witness.merkle_path.len() as u64)?;
    for sibling in &witness.merkle_path {
        encoder.bytes(sibling)?;
    }
    encoder.bytes(&witness.signing_key_proof)?;
    Ok(())
}

fn decode_payload(payload: &[u8]) -> Result<ClaimTx, CborError> {
    let mut decoder = Decoder::new(payload);
    let tx = decode_claim_tx(&mut decoder)?;
    expect_end(&decoder, payload.len())?;
    Ok(tx)
}

fn decode_claim_tx(decoder: &mut Decoder<'_>) -> Result<ClaimTx, CborError> {
    expect_array(decoder, "claim_tx", 2)?;
    match decoder.u64()? {
        VARIANT_UTXO => Ok(ClaimTx::Utxo(decode_utxo(decoder)?)),
        VARIANT_COLLECTION => Ok(ClaimTx::Collection(decode_collection(decoder)?)),
        variant => Err(CborError::UnknownVariant(variant)),
    }
}

fn decode_utxo(decoder: &mut Decoder<'_>) -> Result<ClaimUtxo, CborError> {
    expect_array(decoder, "utxo", 3)?;
    Ok(ClaimUtxo {
        public: decode_public_inputs(decoder)?,
        witness: decode_witness(decoder)?,
        proof: ProofBytes(decoder.bytes()?.to_vec()),
    })
}

fn decode_collection(decoder: &mut Decoder<'_>) -> Result<ClaimCollection, CborError> {
    expect_array(decoder, "collection", 3)?;

    let public_len = array_len(decoder, "collection.public")?;
    let mut public = Vec::with_capacity(public_len as usize);
    for _ in 0..public_len {
        public.push(decode_public_inputs(decoder)?);
    }

    let witness_len = array_len(decoder, "collection.witness")?;
    let mut witness = Vec::with_capacity(witness_len as usize);
    for _ in 0..witness_len {
        witness.push(decode_witness(decoder)?);
    }

    if public.len() != witness.len() {
        return Err(CborError::CollectionArityMismatch {
            public: public.len(),
            witness: witness.len(),
        });
    }

    Ok(ClaimCollection {
        public,
        witness,
        proof: ProofBytes(decoder.bytes()?.to_vec()),
    })
}

fn decode_public_inputs(decoder: &mut Decoder<'_>) -> Result<ClaimPublicInputs, CborError> {
    expect_array(decoder, "public_inputs", 5)?;
    Ok(ClaimPublicInputs {
        sub_tree_id: decoder.u8()?,
        leaf_index: decoder.u64()?,
        bundle_root_blake3: read_hash(decoder, "bundle_root_blake3")?,
        nullifier: read_hash(decoder, "nullifier")?,
        recipient_starstream_addr: read_hash(decoder, "recipient_starstream_addr")?,
    })
}

fn decode_witness(decoder: &mut Decoder<'_>) -> Result<ClaimWitness, CborError> {
    expect_array(decoder, "witness", 3)?;
    let leaf_payload = decoder.bytes()?.to_vec();
    let merkle_path_len = array_len(decoder, "witness.merkle_path")?;
    let mut merkle_path = Vec::with_capacity(merkle_path_len as usize);
    for _ in 0..merkle_path_len {
        merkle_path.push(read_hash(decoder, "witness.merkle_path.sibling")?);
    }
    let signing_key_proof = decoder.bytes()?.to_vec();

    Ok(ClaimWitness {
        leaf_payload,
        merkle_path,
        signing_key_proof,
    })
}

fn read_hash(decoder: &mut Decoder<'_>, field: &'static str) -> Result<Hash, CborError> {
    let bytes = decoder.bytes()?;
    if bytes.len() != CHECKSUM_LEN {
        return Err(CborError::InvalidByteLength {
            field,
            expected: CHECKSUM_LEN,
            actual: bytes.len(),
        });
    }
    let mut out = [0u8; CHECKSUM_LEN];
    out.copy_from_slice(bytes);
    Ok(out)
}

fn expect_array(
    decoder: &mut Decoder<'_>,
    field: &'static str,
    expected: u64,
) -> Result<(), CborError> {
    let actual = array_len(decoder, field)?;
    if actual != expected {
        return Err(CborError::InvalidArrayLength {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

fn array_len(decoder: &mut Decoder<'_>, field: &'static str) -> Result<u64, CborError> {
    decoder.array()?.ok_or(CborError::IndefiniteArray { field })
}

fn expect_end(decoder: &Decoder<'_>, total: usize) -> Result<(), CborError> {
    let position = decoder.position();
    if position != total {
        return Err(CborError::TrailingBytes { position, total });
    }
    Ok(())
}
