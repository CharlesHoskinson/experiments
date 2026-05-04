//! Wire types for v0.1 Ω-Commitment claim transactions.
//!
//! # Overview
//!
//! `omega-claim-tx` defines the CBOR-serialised transaction shapes shipped
//! between the prover, the mock ledger, and the verifier in the proof
//! experiment harness. The two top-level shapes are [`ClaimUtxo`], a
//! single-leaf membership claim, and [`ClaimCollection`], a batched claim
//! that carries one folded Plonky3 proof for many leaves; both are wrapped
//! in the [`ClaimTx`] enum and round-tripped through [`ClaimTx::to_cbor`]
//! and [`ClaimTx::from_cbor`].
//!
//! # Design context
//!
//! - OpenSpec change: [`add-proof-experiment-harness`][change].
//! - Wire-format scenarios: [`proof-harness/spec.md`][spec].
//!
//! [change]: ../../../openspec/changes/add-proof-experiment-harness/
//! [spec]: ../../../openspec/changes/add-proof-experiment-harness/specs/proof-harness/spec.md
//!
//! # Tier of trust
//!
//! Soundness-adjacent. The types here do not perform cryptographic checks
//! themselves, but they pin the bytes that the verifier sees: a divergence
//! between two implementations of this crate is a cross-implementation
//! compatibility break. Public types that bind cryptographic content
//! ([`ClaimPublicInputs`] in particular) carry `# Soundness` blocks.
//!
//! # v0.1 limitations
//!
//! - Wire format v2 (see [`CLAIM_TX_WIRE_VERSION`]). v1 envelopes are not
//!   accepted: [`ClaimTx::from_cbor`] rejects them at parse time with
//!   [`CborError::UnsupportedVersion`].
//! - [`ClaimPublicInputs`] carries `tree_depth` and `per_sub_tree_root` as
//!   public inputs that bind the verifier to a specific sub-tree at a
//!   specific depth. v1 omitted these fields and relied on the verifier
//!   recovering depth out-of-band, which the v2 bump closes.
//! - [`ClaimCollection`] carries one folded proof for the whole batch in
//!   v0.1; per-leaf proofs are out of scope.
//! - [`ClaimWitness`] is the prover-side private witness and is never
//!   broadcast; envelopes destined for the wire MUST redact the witness
//!   fields before serialisation.
//!
//! # Conventions
//!
//! - On-the-wire encoding is canonical CBOR: definite-length arrays,
//!   fixed field order, no duplicate keys, no trailing bytes. The
//!   envelope is `[version, payload, blake3(payload)]`.
//! - All `Hash` fields are 32-byte Blake3 digests serialised as raw byte
//!   strings on the wire and as hex in `serde_json` output.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![warn(rustdoc::missing_crate_level_docs)]

use minicbor::{Decoder, Encoder};
use omega_commitment_core::hash::{blake3_256, Hash};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use thiserror::Error;

/// Wire-format version for [`ClaimTx`] CBOR envelopes.
///
/// The envelope header carries this number as its first element; a decoder
/// observing a different version returns [`CborError::UnsupportedVersion`]
/// and refuses to descend into the payload. Bumping this constant is the
/// only sanctioned way to change the on-wire shape.
///
/// # Version history
///
/// - **v1** (historical, unsupported): [`ClaimPublicInputs`] held only
///   `sub_tree_id`, `leaf_index`, `bundle_root_blake3`, `nullifier`, and
///   `recipient_starstream_addr`. The verifier had to recover the Merkle
///   `tree_depth` and `per_sub_tree_root` out-of-band from the published
///   Ω-Commitment.
/// - **v2** (current): adds `tree_depth` and `per_sub_tree_root` to
///   [`ClaimPublicInputs`] so the verifier sees them as proof public
///   inputs. v1 envelopes parse to [`CborError::UnsupportedVersion`]; the
///   crate offers no migration path.
pub const CLAIM_TX_WIRE_VERSION: u64 = 2;

const VARIANT_UTXO: u64 = 0;
const VARIANT_COLLECTION: u64 = 1;
const CHECKSUM_LEN: usize = 32;

type CborEncodeError = minicbor::encode::Error<Infallible>;

/// A claim transaction submitted to the proof experiment harness.
///
/// The two variants pick out the two shapes the harness accepts:
/// single-leaf claims (one proof, one set of public inputs) and batched
/// collections (one folded proof for many sets of public inputs). The
/// wire form is selected by a u64 variant tag inside the CBOR payload, so
/// adding a third shape is a wire-format bump.
///
/// # Examples
///
/// ```
/// use omega_claim_tx::{ClaimPublicInputs, ClaimTx, ClaimUtxo, ClaimWitness, ProofBytes};
///
/// let claim = ClaimTx::Utxo(ClaimUtxo {
///     public: ClaimPublicInputs {
///         sub_tree_id: 0,
///         leaf_index: 0,
///         tree_depth: 8,
///         per_sub_tree_root: [0; 32],
///         bundle_root_blake3: [0; 32],
///         nullifier: [0; 32],
///         recipient_starstream_addr: [0; 32],
///     },
///     witness: ClaimWitness {
///         leaf_payload: vec![0; 32],
///         merkle_path: vec![[0; 32]; 8],
///         signing_key_proof: vec![],
///     },
///     proof: ProofBytes(vec![]),
/// });
/// let bytes = claim.to_cbor().unwrap();
/// assert_eq!(ClaimTx::from_cbor(&bytes).unwrap(), claim);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClaimTx {
    /// A single-leaf claim. Used when the prover wants to attest to one
    /// leaf and pays the per-claim proof cost individually.
    Utxo(ClaimUtxo),
    /// A batched claim with one folded proof covering every entry in
    /// `public`. Used when the prover amortises a single proving run
    /// across many leaves; the verifier checks the folded proof once.
    Collection(ClaimCollection),
}

/// A single-leaf claim with its public inputs, private witness, and
/// Plonky3 proof bytes.
///
/// On the wire the fields are encoded in fixed order as a 3-element CBOR
/// array. The `witness` field is prover-side private material and MUST be
/// redacted before the envelope is broadcast; see [`ClaimWitness`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimUtxo {
    /// Public inputs visible to the verifier and the ledger; these are
    /// what the proof binds to.
    pub public: ClaimPublicInputs,
    /// Private witness used by the prover. Never broadcast; redacted
    /// before envelope serialisation.
    pub witness: ClaimWitness,
    /// Plonky3 STARK proof bytes attesting to the public inputs above.
    pub proof: ProofBytes,
}

/// A batched claim carrying one folded Plonky3 proof over many leaves.
///
/// The `public` and `witness` vectors MUST have the same length; encoding
/// or decoding a [`ClaimCollection`] with mismatched arities returns
/// [`CborError::CollectionArityMismatch`]. The folded `proof` is one
/// proof for the entire batch in v0.1; per-leaf proofs are not supported.
///
/// # Examples
///
/// ```
/// use omega_claim_tx::{ClaimCollection, ClaimPublicInputs, ClaimTx, ClaimWitness, ProofBytes};
///
/// let pub_in = ClaimPublicInputs {
///     sub_tree_id: 0, leaf_index: 1, tree_depth: 8,
///     per_sub_tree_root: [0; 32], bundle_root_blake3: [0; 32],
///     nullifier: [1; 32], recipient_starstream_addr: [0; 32],
/// };
/// let wit = ClaimWitness {
///     leaf_payload: vec![1; 32],
///     merkle_path: vec![[0; 32]; 8],
///     signing_key_proof: vec![],
/// };
/// let claim = ClaimTx::Collection(ClaimCollection {
///     public: vec![pub_in],
///     witness: vec![wit],
///     proof: ProofBytes(vec![]),
/// });
/// assert!(claim.to_cbor().is_ok());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimCollection {
    /// Public inputs for each claimed leaf, in the order the folded proof
    /// commits to them.
    pub public: Vec<ClaimPublicInputs>,
    /// Private witness for each claimed leaf, paired index-for-index
    /// with `public`. Same redaction rule as [`ClaimUtxo::witness`].
    pub witness: Vec<ClaimWitness>,
    /// Folded Plonky3 proof bytes covering the entire batch. v0.1 emits
    /// exactly one proof for the whole collection.
    pub proof: ProofBytes,
}

/// Public inputs that the verifier sees and that the proof commits to.
///
/// Every field here is part of the transcript the verifier hashes when it
/// rebuilds the proof's binding digest; changing any field on the wire
/// while leaving the proof bytes intact causes the verifier to reject the
/// proof as invalid.
///
/// # Soundness
///
/// **Preserved:** the tuple `(sub_tree_id, leaf_index, tree_depth,
/// per_sub_tree_root, bundle_root_blake3, nullifier,
/// recipient_starstream_addr)` is bound bit-for-bit into the proof's
/// public values. A verifier that accepts the proof has been told,
/// non-malleably, the exact sub-tree, depth, and root the prover claims
/// to have opened against.
///
/// **Closed attack:** rewrite-the-envelope. A man-in-the-middle that
/// changes any public input (for example, swapping `nullifier` to point
/// at a different UTxO, or lowering `tree_depth` to attack a sub-tree
/// other than the one the proof was generated for) cannot do so without
/// invalidating the proof's binding digest. The combination of
/// `tree_depth` and `per_sub_tree_root` (added in wire format v2) closes
/// the v1 gap where a verifier had to recover those values out-of-band
/// from the published Ω-Commitment.
///
/// **Not preserved:** correctness of the values themselves. A malicious
/// prover can still publish a [`ClaimPublicInputs`] for the wrong
/// sub-tree or the wrong depth; the verifier catches that by comparing
/// `per_sub_tree_root` and `tree_depth` against the bound
/// Ω-Commitment, not by trusting the fields here.
///
/// # Limitations
///
/// `tree_depth` is a `u8`; sub-trees deeper than 255 are unrepresentable
/// and would require a wire-format bump. v0.1 sub-trees are at most 32
/// deep so this is comfortable headroom.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimPublicInputs {
    /// Ω-Commitment sub-tree identifier; selects which sub-tree's root
    /// the proof must match.
    pub sub_tree_id: u8,
    /// Canonical leaf index inside the selected sub-tree.
    pub leaf_index: u64,
    /// Merkle path depth for the selected sub-tree. Bound into the proof
    /// as a public input in wire format v2.
    pub tree_depth: u8,
    /// Per-sub-tree root pinned by the published Ω-Commitment for
    /// `sub_tree_id`. Bound into the proof as a public input in wire
    /// format v2.
    #[serde(with = "hex::serde")]
    pub per_sub_tree_root: Hash,
    /// Blake3 bundle root from the published Ω-Commitment.
    #[serde(with = "hex::serde")]
    pub bundle_root_blake3: Hash,
    /// Replay-prevention nullifier. The ledger rejects a second claim
    /// presenting the same nullifier.
    #[serde(with = "hex::serde")]
    pub nullifier: Hash,
    /// Starstream recipient address that receives the claimed value.
    #[serde(with = "hex::serde")]
    pub recipient_starstream_addr: Hash,
}

/// Prover-side private witness for a single-leaf claim.
///
/// This struct is NEVER broadcast as part of a wire envelope. It exists
/// for the prover-to-prover handoff inside the harness and for tests.
/// Code that builds an envelope for the network MUST replace the witness
/// with an empty placeholder before calling [`ClaimTx::to_cbor`]; leaking
/// `leaf_payload` or `signing_key_proof` defeats the privacy contract of
/// the proof system.
///
/// # Limitations
///
/// `signing_key_proof` is a placeholder for the v0.2 PQ signing-key
/// gadget; v0.1 ships an opaque byte string that the verifier does not
/// parse.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimWitness {
    /// Canonical leaf payload bytes. Hashed under domain tag
    /// `omega:v2:leaf` to recover the leaf hash.
    #[serde(with = "hex::serde")]
    pub leaf_payload: Vec<u8>,
    /// Sibling hashes from leaf to root, length equal to
    /// [`ClaimPublicInputs::tree_depth`].
    #[serde(with = "omega_commitment_core::serde_helpers::hex_vec_hash")]
    pub merkle_path: Vec<Hash>,
    /// Prototype signing-key proof bytes; opaque to the v0.1 verifier.
    #[serde(with = "hex::serde")]
    pub signing_key_proof: Vec<u8>,
}

/// Opaque Plonky3 proof bytes carried inside a [`ClaimTx`].
///
/// The verifier parses these bytes via `omega-claim-verifier`; this crate
/// treats them as a raw byte string and only commits them to the
/// envelope checksum. Two claims with byte-identical [`ProofBytes`]
/// encode identically.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct ProofBytes(#[serde(with = "hex::serde")] pub Vec<u8>);

/// Errors returned by the [`ClaimTx`] CBOR codec.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum CborError {
    /// Fired when the underlying `minicbor` encoder fails. In practice
    /// this only happens if the `Vec<u8>` writer cannot grow, which the
    /// `std` allocator surfaces as an abort, so this variant is mostly
    /// reserved for future non-`Vec` writers.
    #[error("claim tx cbor encode failed: {0}")]
    Encode(String),
    /// Fired when `minicbor` cannot decode the input as CBOR (truncated
    /// bytes, wrong major type, malformed length headers).
    #[error("claim tx cbor decode failed: {0}")]
    Decode(String),
    /// Fired when the envelope's version header does not equal
    /// [`CLAIM_TX_WIRE_VERSION`]. v1 envelopes hit this path.
    #[error("unsupported claim tx wire version {0}")]
    UnsupportedVersion(u64),
    /// Fired when the envelope's `blake3(payload)` checksum does not
    /// match a freshly computed digest of the decoded payload bytes.
    #[error("claim tx cbor checksum mismatch")]
    ChecksumMismatch,
    /// Fired when the decoded payload re-encodes to bytes that differ
    /// from the input. Guards canonical CBOR (definite-length arrays,
    /// fixed field order, no duplicate keys).
    #[error("claim tx cbor payload is not canonical")]
    NonCanonicalPayload,
    /// Fired when the decoder reaches a successful end-of-value while
    /// `position < total`, i.e. the input has trailing bytes after a
    /// well-formed envelope.
    #[error("claim tx cbor trailing bytes: position {position} of {total}")]
    TrailingBytes {
        /// Byte position the decoder stopped at.
        position: usize,
        /// Total length of the input.
        total: usize,
    },
    /// Fired when a fixed-size byte string field (a `Hash`, the
    /// envelope checksum) does not have the expected length.
    #[error("claim tx cbor field {field} expected {expected} bytes, got {actual}")]
    InvalidByteLength {
        /// Logical name of the offending field.
        field: &'static str,
        /// Expected byte length.
        expected: usize,
        /// Observed byte length.
        actual: usize,
    },
    /// Fired when a fixed-arity CBOR array (the envelope, a public-inputs
    /// row, a witness row) does not have the expected element count.
    #[error("claim tx cbor array {field} expected {expected} entries, got {actual}")]
    InvalidArrayLength {
        /// Logical name of the offending array.
        field: &'static str,
        /// Expected element count.
        expected: u64,
        /// Observed element count.
        actual: u64,
    },
    /// Fired when the input uses a CBOR indefinite-length array. The
    /// canonical form requires definite-length headers everywhere.
    #[error("claim tx cbor array {field} must use definite length")]
    IndefiniteArray {
        /// Logical name of the offending array.
        field: &'static str,
    },
    /// Fired when the [`ClaimTx`] variant tag inside the payload is
    /// neither `0` (Utxo) nor `1` (Collection).
    #[error("unknown claim tx variant {0}")]
    UnknownVariant(u64),
    /// Fired when [`ClaimCollection::public`] and
    /// [`ClaimCollection::witness`] have different lengths, on either
    /// encode or decode.
    #[error("collection arity mismatch: public={public}, witness={witness}")]
    CollectionArityMismatch {
        /// Length of the `public` vector.
        public: usize,
        /// Length of the `witness` vector.
        witness: usize,
    },
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
    /// Encodes this claim transaction as a canonical CBOR envelope.
    ///
    /// The wire form is the 3-element array
    /// `[CLAIM_TX_WIRE_VERSION, payload, blake3(payload)]`, where
    /// `payload` is the fixed-order CBOR encoding of the claim. The
    /// encoder uses definite-length arrays, no duplicate keys, and emits
    /// no trailing bytes; an honest re-decode followed by a re-encode
    /// reproduces the original bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use omega_claim_tx::{ClaimPublicInputs, ClaimTx, ClaimUtxo, ClaimWitness, ProofBytes};
    /// let claim = ClaimTx::Utxo(ClaimUtxo {
    ///     public: ClaimPublicInputs {
    ///         sub_tree_id: 0, leaf_index: 0, tree_depth: 0,
    ///         per_sub_tree_root: [0; 32], bundle_root_blake3: [0; 32],
    ///         nullifier: [0; 32], recipient_starstream_addr: [0; 32],
    ///     },
    ///     witness: ClaimWitness { leaf_payload: vec![], merkle_path: vec![], signing_key_proof: vec![] },
    ///     proof: ProofBytes(vec![]),
    /// });
    /// let bytes = claim.to_cbor().unwrap();
    /// assert!(!bytes.is_empty());
    /// ```
    ///
    /// # Errors
    ///
    /// - [`CborError::CollectionArityMismatch`] when `self` is a
    ///   [`ClaimTx::Collection`] whose `public` and `witness` lengths
    ///   disagree.
    /// - [`CborError::Encode`] when the underlying CBOR writer fails.
    pub fn to_cbor(&self) -> Result<Vec<u8>, CborError> {
        self.validate()?;
        let payload = encode_payload(self)?;
        let checksum = blake3_256(&payload);

        let mut encoder = Encoder::new(Vec::new());
        encoder
            .array(3)?
            .u64(CLAIM_TX_WIRE_VERSION)?
            .bytes(&payload)?
            .bytes(&checksum)?;
        Ok(encoder.into_writer())
    }

    /// Decodes a claim transaction from its canonical CBOR envelope.
    ///
    /// Verifies the envelope structure, the version, the
    /// `blake3(payload)` checksum, the canonicality of the payload bytes
    /// (decoded then re-encoded must equal the input), and the absence
    /// of trailing bytes before returning the parsed [`ClaimTx`].
    ///
    /// # Examples
    ///
    /// ```
    /// use omega_claim_tx::{ClaimPublicInputs, ClaimTx, ClaimUtxo, ClaimWitness, ProofBytes};
    /// let claim = ClaimTx::Utxo(ClaimUtxo {
    ///     public: ClaimPublicInputs {
    ///         sub_tree_id: 0, leaf_index: 0, tree_depth: 0,
    ///         per_sub_tree_root: [0; 32], bundle_root_blake3: [0; 32],
    ///         nullifier: [0; 32], recipient_starstream_addr: [0; 32],
    ///     },
    ///     witness: ClaimWitness { leaf_payload: vec![], merkle_path: vec![], signing_key_proof: vec![] },
    ///     proof: ProofBytes(vec![]),
    /// });
    /// let bytes = claim.to_cbor().unwrap();
    /// assert_eq!(ClaimTx::from_cbor(&bytes).unwrap(), claim);
    /// ```
    ///
    /// # Errors
    ///
    /// - [`CborError::Decode`] when the input is not well-formed CBOR.
    /// - [`CborError::UnsupportedVersion`] when the envelope version is
    ///   not [`CLAIM_TX_WIRE_VERSION`] (in particular, every v1
    ///   envelope).
    /// - [`CborError::InvalidArrayLength`] when an inner CBOR array has
    ///   the wrong element count.
    /// - [`CborError::IndefiniteArray`] when an inner CBOR array uses an
    ///   indefinite-length header.
    /// - [`CborError::InvalidByteLength`] when a fixed-size byte string
    ///   (a `Hash`, the envelope checksum) has the wrong length.
    /// - [`CborError::ChecksumMismatch`] when the envelope's checksum
    ///   does not match `blake3(payload)`.
    /// - [`CborError::NonCanonicalPayload`] when the decoded payload
    ///   re-encodes to a different byte string.
    /// - [`CborError::TrailingBytes`] when bytes follow a well-formed
    ///   envelope.
    /// - [`CborError::UnknownVariant`] when the [`ClaimTx`] variant tag
    ///   is neither `0` nor `1`.
    /// - [`CborError::CollectionArityMismatch`] when a decoded
    ///   [`ClaimTx::Collection`] has mismatched `public` and `witness`
    ///   lengths.
    pub fn from_cbor(bytes: &[u8]) -> Result<Self, CborError> {
        let mut decoder = Decoder::new(bytes);
        expect_array(&mut decoder, "envelope", 3)?;
        let version = decoder.u64()?;
        if version != CLAIM_TX_WIRE_VERSION {
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
        .array(7)?
        .u8(public.sub_tree_id)?
        .u64(public.leaf_index)?
        .u8(public.tree_depth)?
        .bytes(&public.per_sub_tree_root)?
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
    expect_array(decoder, "public_inputs", 7)?;
    Ok(ClaimPublicInputs {
        sub_tree_id: decoder.u8()?,
        leaf_index: decoder.u64()?,
        tree_depth: decoder.u8()?,
        per_sub_tree_root: read_hash(decoder, "per_sub_tree_root")?,
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
