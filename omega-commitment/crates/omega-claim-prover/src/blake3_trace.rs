use omega_commitment_core::{
    hash::Hash,
    tree::{DOMAIN_LEAF, DOMAIN_NODE},
};
use p3_air::utils::u32_to_bits_le;
use p3_blake3_air::NUM_BLAKE3_COLS;
use p3_field::PrimeCharacteristicRing;
use p3_matrix::dense::RowMajorMatrix;

use crate::{Val, MAX_V01_LEAF_PAYLOAD_LEN};

/// Global LogUp lookup name used to glue [`crate::OmegaMembershipAir`]'s
/// compression-input rows to [`crate::OmegaBlake3Air`]'s compression-output
/// rows.
///
/// # Soundness
///
/// The constant ties together the two AIRs. The membership AIR registers
/// `Receive` interactions under this name (one for the leaf compression, two
/// for each two-block node hash); the Blake3 AIR registers a `Send` under the
/// same name on every active compression row. If the names diverge the LogUp
/// running sums on each side close out against different transcripts; the
/// imbalance polynomial is non-zero and `verify_batch` rejects.
pub(crate) const BLAKE3_LOOKUP_NAME: &str = "omega_blake3_compression_v1";
/// Blake3 `CHUNK_START` flag bit.
pub(crate) const CHUNK_START: u32 = 1;
/// Blake3 `CHUNK_END` flag bit.
pub(crate) const CHUNK_END: u32 = 2;
/// Blake3 `ROOT` flag bit.
pub(crate) const ROOT: u32 = 8;
/// Flag word for a leaf compression: `CHUNK_START | CHUNK_END | ROOT`. The
/// preimage fits in one block, so the compression is the chunk's only and
/// final block, and the chunk is the tree root.
pub(crate) const LEAF_FLAGS: u32 = CHUNK_START | CHUNK_END | ROOT;
/// Flag word for the first block of a two-block node compression:
/// `CHUNK_START` only — the chunk continues into a second block.
pub(crate) const NODE_FIRST_FLAGS: u32 = CHUNK_START;
/// Flag word for the second block of a node compression:
/// `CHUNK_END | ROOT` — closes the chunk and is the tree root.
pub(crate) const NODE_SECOND_FLAGS: u32 = CHUNK_END | ROOT;
/// Flag word for a padding row in the Blake3 trace. With both `CHUNK_START`
/// and `CHUNK_END` clear, the LogUp multiplicity (computed as their arithmetic
/// OR) is zero, so the row sends no lookup tuple.
pub(crate) const DUMMY_FLAGS: u32 = 0;

/// Blake3 initial chaining value.
pub(crate) const IV: [u32; 8] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];

const MSG_PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];
const BITS_PER_WORD: usize = 32;
const BYTES_PER_WORD: usize = 4;
const BLOCK_BYTES: usize = 64;
const CV_BYTES: usize = 32;
/// Width in bytes of the Blake3 counter field (`u64`).
pub(crate) const COUNTER_BYTES: usize = 8;
/// Width in bytes of a `u32` (block length, flags).
pub(crate) const U32_BYTES: usize = 4;
/// Total width of one LogUp lookup tuple between [`crate::OmegaMembershipAir`]
/// and [`crate::OmegaBlake3Air`]: block bytes + chaining value + counter +
/// block length + flags + output bytes.
pub(crate) const COMPRESSION_LOOKUP_WIDTH: usize =
    BLOCK_BYTES + CV_BYTES + COUNTER_BYTES + U32_BYTES + U32_BYTES + CV_BYTES;

/// Bit offset, within a Blake3 row, of the compression input bits (16 words ×
/// 32 bits).
pub(crate) const B3_INPUTS_OFFSET: usize = 0;
/// Bit offset of the chaining-value bits (8 words × 32 bits).
pub(crate) const B3_CHAINING_VALUES_OFFSET: usize = B3_INPUTS_OFFSET + 16 * BITS_PER_WORD;
/// Bit offset of the counter low word.
pub(crate) const B3_COUNTER_LOW_OFFSET: usize = B3_CHAINING_VALUES_OFFSET + 8 * BITS_PER_WORD;
/// Bit offset of the counter high word.
pub(crate) const B3_COUNTER_HI_OFFSET: usize = B3_COUNTER_LOW_OFFSET + BITS_PER_WORD;
/// Bit offset of the block-length word.
pub(crate) const B3_BLOCK_LEN_OFFSET: usize = B3_COUNTER_HI_OFFSET + BITS_PER_WORD;
/// Bit offset of the flags word.
pub(crate) const B3_FLAGS_OFFSET: usize = B3_BLOCK_LEN_OFFSET + BITS_PER_WORD;
const B3_INITIAL_ROW0_OFFSET: usize = B3_FLAGS_OFFSET + BITS_PER_WORD;
const B3_INITIAL_ROW2_OFFSET: usize = B3_INITIAL_ROW0_OFFSET + 8;
const B3_FULL_ROUNDS_OFFSET: usize = B3_INITIAL_ROW2_OFFSET + 8;
const B3_STATE_WIDTH: usize = 8 + 128 + 8 + 128;
const B3_FULL_ROUND_WIDTH: usize = B3_STATE_WIDTH * 4;
const B3_FINAL_HELPERS_OFFSET: usize = B3_FULL_ROUNDS_OFFSET + B3_FULL_ROUND_WIDTH * 7;
/// Bit offset of the compression output bits (16 words × 32 bits).
pub(crate) const B3_OUTPUTS_OFFSET: usize = B3_FINAL_HELPERS_OFFSET + 4 * BITS_PER_WORD;
const EXPECTED_BLAKE3_COLS: usize = B3_OUTPUTS_OFFSET + 16 * BITS_PER_WORD;
const _: [(); NUM_BLAKE3_COLS] = [(); EXPECTED_BLAKE3_COLS];

/// One Blake3 compression invocation: inputs (`block_words`, `cv_words`,
/// `counter`, `block_len`, `flags`) plus the resulting first eight output
/// words (the chaining value or, for a `ROOT` block, the digest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Compression {
    /// 64 bytes of input message, packed as 16 little-endian `u32` words.
    pub block_words: [u32; 16],
    /// Eight-word chaining value (or `IV` for the first block).
    pub cv_words: [u32; 8],
    /// Blake3 chunk counter (always 0 for v0.1's leaf and node hashes).
    pub counter: u64,
    /// Number of meaningful bytes in the block, ≤ 64.
    pub block_len: u32,
    /// Blake3 flags word for this compression (e.g. `LEAF_FLAGS`).
    pub flags: u32,
    /// First eight output words of the compression.
    pub output_words: [u32; 8],
}

/// Builds the single Blake3 compression that produces the v2 leaf hash for
/// `(sub_tree_id, leaf_index, payload)`.
///
/// The block contains `DOMAIN_LEAF || sub_tree_id || leaf_index_be ||
/// payload_len_be || payload`, zero-padded to 64 bytes; `block_len` is the
/// effective preimage length and `flags = LEAF_FLAGS`.
///
/// Caller must ensure `payload.len() <= MAX_V01_LEAF_PAYLOAD_LEN`; this is
/// debug-asserted.
///
/// # Examples
///
/// ```ignore
/// // pub(crate); not callable from outside the crate.
/// let c = leaf_compression(1, 42, b"alice");
/// assert_eq!(hash_from_words(c.output_words).len(), 32);
/// ```
pub(crate) fn leaf_compression(sub_tree_id: u8, leaf_index: u64, payload: &[u8]) -> Compression {
    debug_assert!(payload.len() <= MAX_V01_LEAF_PAYLOAD_LEN);
    let mut block = [0u8; BLOCK_BYTES];
    let mut offset = 0usize;
    block[offset..offset + DOMAIN_LEAF.len()].copy_from_slice(DOMAIN_LEAF);
    offset += DOMAIN_LEAF.len();
    block[offset] = sub_tree_id;
    offset += 1;
    block[offset..offset + 8].copy_from_slice(&leaf_index.to_be_bytes());
    offset += 8;
    block[offset..offset + 8].copy_from_slice(&(payload.len() as u64).to_be_bytes());
    offset += 8;
    block[offset..offset + payload.len()].copy_from_slice(payload);

    let block_len = (DOMAIN_LEAF.len() + 1 + 8 + 8 + payload.len()) as u32;
    compression(block_words(&block), IV, 0, block_len, LEAF_FLAGS)
}

/// Builds the two Blake3 compressions that produce the v2 node hash for
/// `node_hash_v2(left, right)`.
///
/// The full preimage is `DOMAIN_NODE || left || right` (4 + 32 + 32 = 68
/// bytes), spanning two 64-byte blocks: the first carries `flags =
/// NODE_FIRST_FLAGS` (chunk start), the second carries `flags =
/// NODE_SECOND_FLAGS` (chunk end and root) and consumes the first block's
/// `output_words` as its chaining value.
///
/// # Examples
///
/// ```ignore
/// // pub(crate); not callable from outside the crate.
/// let [first, second] = node_compressions(&[0u8; 32], &[0u8; 32]);
/// assert_ne!(first.output_words, second.output_words);
/// ```
pub(crate) fn node_compressions(left: &Hash, right: &Hash) -> [Compression; 2] {
    let mut preimage = [0u8; DOMAIN_NODE.len() + 64];
    let mut offset = 0usize;
    preimage[offset..offset + DOMAIN_NODE.len()].copy_from_slice(DOMAIN_NODE);
    offset += DOMAIN_NODE.len();
    preimage[offset..offset + 32].copy_from_slice(left);
    offset += 32;
    preimage[offset..offset + 32].copy_from_slice(right);

    let mut first_block = [0u8; BLOCK_BYTES];
    first_block.copy_from_slice(&preimage[..BLOCK_BYTES]);
    let first = compression(
        block_words(&first_block),
        IV,
        0,
        BLOCK_BYTES as u32,
        NODE_FIRST_FLAGS,
    );

    let mut second_block = [0u8; BLOCK_BYTES];
    second_block[..preimage.len() - BLOCK_BYTES].copy_from_slice(&preimage[BLOCK_BYTES..]);
    let second = compression(
        block_words(&second_block),
        first.output_words,
        0,
        (preimage.len() - BLOCK_BYTES) as u32,
        NODE_SECOND_FLAGS,
    );

    [first, second]
}

/// Repacks eight little-endian `u32` words into a 32-byte hash.
///
/// # Examples
///
/// ```ignore
/// // pub(crate); not callable from outside the crate.
/// let bytes = hash_from_words([0; 8]);
/// assert_eq!(bytes, [0u8; 32]);
/// ```
pub(crate) fn hash_from_words(words: [u32; 8]) -> Hash {
    let mut out = [0u8; 32];
    for (chunk, word) in out.chunks_exact_mut(4).zip(words) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    out
}

#[cfg(test)]
pub(crate) fn compression_lookup_values(
    compression: &Compression,
) -> [Val; COMPRESSION_LOOKUP_WIDTH] {
    let mut values = [Val::ZERO; COMPRESSION_LOOKUP_WIDTH];
    let mut offset = 0usize;
    write_word_bytes(&mut values, &mut offset, &compression.block_words);
    write_word_bytes(&mut values, &mut offset, &compression.cv_words);
    write_u64_bytes(&mut values, &mut offset, compression.counter);
    write_u32_bytes(&mut values, &mut offset, compression.block_len);
    write_u32_bytes(&mut values, &mut offset, compression.flags);
    write_word_bytes(&mut values, &mut offset, &compression.output_words);
    debug_assert_eq!(offset, COMPRESSION_LOOKUP_WIDTH);
    values
}

/// Builds the [`crate::OmegaBlake3Air`] trace from a slice of compressions.
///
/// One row per real compression, then padding rows with `flags = DUMMY_FLAGS`
/// out to the next power of two. Padding rows produce zero LogUp multiplicity
/// (the `flag0 + flag1 - flag0 * flag1` arithmetic OR is zero when both flag
/// bits are clear) and so do not contribute any spurious lookup tuples.
pub(crate) fn build_blake3_trace(compressions: &[Compression]) -> RowMajorMatrix<Val> {
    let real_rows = compressions.len().max(1);
    let height = real_rows.next_power_of_two();
    let mut values = Val::zero_vec(height * NUM_BLAKE3_COLS);

    for (row_index, compression) in compressions.iter().copied().enumerate() {
        let row = &mut values[row_index * NUM_BLAKE3_COLS..(row_index + 1) * NUM_BLAKE3_COLS];
        fill_trace_row(row, compression);
    }

    let dummy = compression([0u32; 16], IV, 0, 0, DUMMY_FLAGS);
    for row_index in real_rows..height {
        let row = &mut values[row_index * NUM_BLAKE3_COLS..(row_index + 1) * NUM_BLAKE3_COLS];
        fill_trace_row(row, dummy);
    }

    RowMajorMatrix::new(values, NUM_BLAKE3_COLS)
}

fn compression(
    block_words: [u32; 16],
    cv_words: [u32; 8],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> Compression {
    let output = compress(block_words, cv_words, counter, block_len, flags);
    Compression {
        block_words,
        cv_words,
        counter,
        block_len,
        flags,
        output_words: output[..8].try_into().expect("first eight output words"),
    }
}

fn block_words(block: &[u8; BLOCK_BYTES]) -> [u32; 16] {
    core::array::from_fn(|i| {
        let offset = i * BYTES_PER_WORD;
        u32::from_le_bytes([
            block[offset],
            block[offset + 1],
            block[offset + 2],
            block[offset + 3],
        ])
    })
}

fn compress(
    block_words: [u32; 16],
    cv_words: [u32; 8],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> [u32; 16] {
    let mut state = [
        cv_words[0],
        cv_words[1],
        cv_words[2],
        cv_words[3],
        cv_words[4],
        cv_words[5],
        cv_words[6],
        cv_words[7],
        IV[0],
        IV[1],
        IV[2],
        IV[3],
        counter as u32,
        (counter >> 32) as u32,
        block_len,
        flags,
    ];
    let mut m_vec = block_words;
    for _ in 0..7 {
        round(&mut state, &m_vec);
        permute(&mut m_vec);
    }
    for i in 0..8 {
        state[i] ^= state[i + 8];
        state[i + 8] ^= cv_words[i];
    }
    state
}

fn round(state: &mut [u32; 16], m: &[u32; 16]) {
    g(state, 0, 4, 8, 12, m[0], m[1]);
    g(state, 1, 5, 9, 13, m[2], m[3]);
    g(state, 2, 6, 10, 14, m[4], m[5]);
    g(state, 3, 7, 11, 15, m[6], m[7]);
    g(state, 0, 5, 10, 15, m[8], m[9]);
    g(state, 1, 6, 11, 12, m[10], m[11]);
    g(state, 2, 7, 8, 13, m[12], m[13]);
    g(state, 3, 4, 9, 14, m[14], m[15]);
}

fn g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, mx: u32, my: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(mx);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(my);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

fn permute<T: Copy>(m: &mut [T; 16]) {
    *m = core::array::from_fn(|i| m[MSG_PERMUTATION[i]]);
}

fn fill_trace_row(row: &mut [Val], compression: Compression) {
    for (word_index, word) in compression.block_words.iter().copied().enumerate() {
        write_bits(row, B3_INPUTS_OFFSET + word_index * BITS_PER_WORD, word);
    }
    for (word_index, word) in compression.cv_words.iter().copied().enumerate() {
        write_bits(
            row,
            B3_CHAINING_VALUES_OFFSET + word_index * BITS_PER_WORD,
            word,
        );
    }
    write_bits(row, B3_COUNTER_LOW_OFFSET, compression.counter as u32);
    write_bits(
        row,
        B3_COUNTER_HI_OFFSET,
        (compression.counter >> 32) as u32,
    );
    write_bits(row, B3_BLOCK_LEN_OFFSET, compression.block_len);
    write_bits(row, B3_FLAGS_OFFSET, compression.flags);

    for (i, (cv_word, iv_word)) in compression
        .cv_words
        .iter()
        .copied()
        .zip(IV.iter().copied())
        .take(4)
        .enumerate()
    {
        write_limbs(row, B3_INITIAL_ROW0_OFFSET + i * 2, cv_word);
        write_limbs(row, B3_INITIAL_ROW2_OFFSET + i * 2, iv_word);
    }

    let mut state = [
        [
            compression.cv_words[0],
            compression.cv_words[1],
            compression.cv_words[2],
            compression.cv_words[3],
        ],
        [
            compression.cv_words[4],
            compression.cv_words[5],
            compression.cv_words[6],
            compression.cv_words[7],
        ],
        [IV[0], IV[1], IV[2], IV[3]],
        [
            compression.counter as u32,
            (compression.counter >> 32) as u32,
            compression.block_len,
            compression.flags,
        ],
    ];
    let mut m_vec = compression.block_words;
    for round_index in 0..7 {
        fill_round(
            row,
            B3_FULL_ROUNDS_OFFSET + round_index * B3_FULL_ROUND_WIDTH,
            &mut state,
            &m_vec,
        );
        permute(&mut m_vec);
    }

    for (i, value) in state[2].iter().copied().enumerate() {
        write_bits(row, B3_FINAL_HELPERS_OFFSET + i * BITS_PER_WORD, value);
    }

    for (i, (((s0, s1), (s2, s3)), (cv0, cv1))) in state[0]
        .iter()
        .copied()
        .zip(state[1].iter().copied())
        .zip(state[2].iter().copied().zip(state[3].iter().copied()))
        .zip(
            compression.cv_words[..4]
                .iter()
                .copied()
                .zip(compression.cv_words[4..].iter().copied()),
        )
        .enumerate()
    {
        write_bits(row, B3_OUTPUTS_OFFSET + i * BITS_PER_WORD, s0 ^ s2);
        write_bits(row, B3_OUTPUTS_OFFSET + (4 + i) * BITS_PER_WORD, s1 ^ s3);
        write_bits(row, B3_OUTPUTS_OFFSET + (8 + i) * BITS_PER_WORD, s2 ^ cv0);
        write_bits(row, B3_OUTPUTS_OFFSET + (12 + i) * BITS_PER_WORD, s3 ^ cv1);
    }
}

fn fill_round(row: &mut [Val], round_offset: usize, state: &mut [[u32; 4]; 4], m_vec: &[u32; 16]) {
    for i in 0..4 {
        (state[0][i], state[1][i], state[2][i], state[3][i]) = verifiable_half_round(
            state[0][i],
            state[1][i],
            state[2][i],
            state[3][i],
            m_vec[2 * i],
            false,
        );
    }
    save_state(row, round_offset, state);

    for i in 0..4 {
        (state[0][i], state[1][i], state[2][i], state[3][i]) = verifiable_half_round(
            state[0][i],
            state[1][i],
            state[2][i],
            state[3][i],
            m_vec[2 * i + 1],
            true,
        );
    }
    save_state(row, round_offset + B3_STATE_WIDTH, state);

    for i in 0..4 {
        (
            state[0][i],
            state[1][(i + 1) % 4],
            state[2][(i + 2) % 4],
            state[3][(i + 3) % 4],
        ) = verifiable_half_round(
            state[0][i],
            state[1][(i + 1) % 4],
            state[2][(i + 2) % 4],
            state[3][(i + 3) % 4],
            m_vec[8 + 2 * i],
            false,
        );
    }
    save_state(row, round_offset + B3_STATE_WIDTH * 2, state);

    for i in 0..4 {
        (
            state[0][i],
            state[1][(i + 1) % 4],
            state[2][(i + 2) % 4],
            state[3][(i + 3) % 4],
        ) = verifiable_half_round(
            state[0][i],
            state[1][(i + 1) % 4],
            state[2][(i + 2) % 4],
            state[3][(i + 3) % 4],
            m_vec[9 + 2 * i],
            true,
        );
    }
    save_state(row, round_offset + B3_STATE_WIDTH * 3, state);
}

const fn verifiable_half_round(
    mut a: u32,
    mut b: u32,
    mut c: u32,
    mut d: u32,
    m: u32,
    flag: bool,
) -> (u32, u32, u32, u32) {
    let (rot_1, rot_2) = if flag { (8, 7) } else { (16, 12) };
    a = a.wrapping_add(b);
    a = a.wrapping_add(m);
    d = (d ^ a).rotate_right(rot_1);
    c = c.wrapping_add(d);
    b = (b ^ c).rotate_right(rot_2);
    (a, b, c, d)
}

fn save_state(row: &mut [Val], offset: usize, state: &[[u32; 4]; 4]) {
    for (i, (((s0, s1), s2), s3)) in state[0]
        .iter()
        .copied()
        .zip(state[1].iter().copied())
        .zip(state[2].iter().copied())
        .zip(state[3].iter().copied())
        .enumerate()
    {
        write_limbs(row, offset + i * 2, s0);
        write_bits(row, offset + 8 + i * BITS_PER_WORD, s1);
        write_limbs(row, offset + 8 + 128 + i * 2, s2);
        write_bits(row, offset + 8 + 128 + 8 + i * BITS_PER_WORD, s3);
    }
}

fn write_bits(row: &mut [Val], offset: usize, value: u32) {
    let bits = u32_to_bits_le::<Val>(value);
    row[offset..offset + BITS_PER_WORD].copy_from_slice(&bits);
}

fn write_limbs(row: &mut [Val], offset: usize, value: u32) {
    row[offset] = Val::from_u16(value as u16);
    row[offset + 1] = Val::from_u16((value >> 16) as u16);
}

#[cfg(test)]
fn write_word_bytes(values: &mut [Val], offset: &mut usize, words: &[u32]) {
    for word in words {
        for byte in word.to_le_bytes() {
            values[*offset] = Val::from_u8(byte);
            *offset += 1;
        }
    }
}

#[cfg(test)]
fn write_u64_bytes(values: &mut [Val], offset: &mut usize, value: u64) {
    for byte in value.to_le_bytes() {
        values[*offset] = Val::from_u8(byte);
        *offset += 1;
    }
}

#[cfg(test)]
fn write_u32_bytes(values: &mut [Val], offset: &mut usize, value: u32) {
    for byte in value.to_le_bytes() {
        values[*offset] = Val::from_u8(byte);
        *offset += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_commitment_core::hash::blake3_256;
    use omega_commitment_core::tree::{leaf_hash_v2, node_hash_v2};
    use p3_matrix::Matrix;

    #[test]
    fn leaf_compression_matches_core_hash_for_single_block_payload() {
        let payload = b"utxo payload";
        let compression = leaf_compression(1, 42, payload);

        assert_eq!(
            hash_from_words(compression.output_words),
            leaf_hash_v2(1, 42, payload)
        );
    }

    #[test]
    fn node_compressions_match_core_hash_for_two_block_node_preimage() {
        let left = blake3_256(b"left");
        let right = blake3_256(b"right");
        let [_, second] = node_compressions(&left, &right);

        assert_eq!(
            hash_from_words(second.output_words),
            node_hash_v2(&left, &right)
        );
    }

    #[test]
    fn generated_trace_uses_pinned_blake3_width() {
        let compression = leaf_compression(1, 0, b"");
        let trace = build_blake3_trace(&[compression]);

        assert_eq!(trace.width(), NUM_BLAKE3_COLS);
    }
}
