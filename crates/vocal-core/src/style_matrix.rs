//! Reading a Kokoro voice vector: 510 rows × 256 little-endian float32, row-major.
//!
//! A persona in this engine family is *not* a model — it is one of these files, 522,240 bytes
//! (`docs/research/KOKORO_OFFLINE_SPIKE.md`). Two consequences this module owns:
//!
//! * **Which row is used is part of the sound.** The reference reads row `n_tokens`, i.e. the row is
//!   indexed by how many phonemes the utterance has, so the same voice changes prosody when a caller
//!   chunks text differently. [`StyleMatrix::row_for_phonemes`] keeps that rule in one place instead
//!   of letting each caller re-derive the index — which is also where an off-by-one would hide.
//! * **A short file must be refused, not tolerated.** `check_voice_bytes` is exact, and
//!   [`StyleMatrix::new`] uses it: a 30-byte placeholder that "loads" is the failure mode this whole
//!   repo was rebuilt around, and the 88 MB graph would happily consume garbage style rows and
//!   produce audible nonsense.

use crate::error::{VoiceError, VoiceErrorCode, VoiceResult};
use crate::phoneme_tokens::{check_voice_bytes, style_row, STYLE_DIM};

/// Rows in a voice vector: one per utterance length the tokenizer can produce.
pub const ROWS: usize = 510;
/// One row, in bytes (`STYLE_DIM` float32s).
pub const ROW_BYTES: usize = STYLE_DIM * 4;
/// A whole voice vector.
pub const BYTES: usize = ROWS * ROW_BYTES;

// The row index comes from `phoneme_tokens::n_tokens`, which clamps to `MAX_PHONEME_UNITS - 1`.
// If those two constants ever drift, this is a compile error rather than a runtime surprise — which
// is the point: the relationship is the contract, not an implementation detail.
const _: () = assert!(ROWS == crate::phoneme_tokens::MAX_PHONEME_UNITS);

/// Borrowed view over a voice vector's bytes. Decoding is per row on demand: the full matrix is only
/// needed when *creating* a persona (blending every row, so a voice behaves the same at any
/// utterance length — see `scripts/derive-persona-style.py`).
#[derive(Debug, Clone, Copy)]
pub struct StyleMatrix<'a> {
    bytes: &'a [u8],
}

impl<'a> StyleMatrix<'a> {
    /// Wrap a voice vector, rejecting anything that is not exactly [`BYTES`].
    pub fn new(bytes: &'a [u8]) -> VoiceResult<Self> {
        check_voice_bytes(bytes.len())?;
        Ok(Self { bytes })
    }

    /// Number of rows this buffer can serve. Always [`ROWS`] for a validated matrix.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.bytes.len() / ROW_BYTES
    }

    /// The underlying asset, for hashing or provenance recording.
    #[must_use]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Row `index` as `STYLE_DIM` float32 samples, in graph order.
    ///
    /// Little-endian, decoded through `as_chunks::<4>()` so a mis-set stride or a swapped byte order
    /// cannot pass silently: both would move the bytes that land in a given slot and the parity tests
    /// read specific positions.
    pub fn row(&self, index: usize) -> VoiceResult<Vec<f32>> {
        let rows = self.row_count();
        if index >= rows {
            return Err(VoiceError::new(
                VoiceErrorCode::SynthesisFailed,
                format!(
                    "style row {index} is out of range: this voice has {rows} rows (0..{})",
                    rows.saturating_sub(1)
                ),
            ));
        }
        let start = index * ROW_BYTES;
        let end = start + ROW_BYTES;
        let (chunks, _tail) = self.bytes[start..end].as_chunks::<4>();
        Ok(chunks
            .iter()
            .map(|chunk| f32::from_le_bytes(*chunk))
            .collect())
    }

    /// The row this phoneme sequence will use, applying `phoneme_tokens::style_row` so the caller
    /// cannot re-derive the index differently.
    pub fn row_for_phonemes(&self, encoded: &[u16]) -> VoiceResult<Vec<f32>> {
        let index = style_row(encoded);
        self.row(index)
    }

    /// As [`row`] but for a raw float32 slice, for callers that already hold decoded samples (a
    /// derived persona in memory, before it is ever written to disk). No `#[must_use]`: the return
    /// is already a `Result`, and clippy calls the duplicate noise — rightly, a bare `must_use`
    /// without a message says nothing the type does not.
    pub fn row_from_flat(flat: &[f32], index: usize) -> VoiceResult<Vec<f32>> {
        let width = STYLE_DIM;
        if flat.len() != ROWS * width {
            return Err(VoiceError::new(
                VoiceErrorCode::PackInvalidFormat,
                format!(
                    "style matrix has {} floats, expected {} ({} rows x {width})",
                    flat.len(),
                    ROWS * width,
                    ROWS
                ),
            ));
        }
        if index >= ROWS {
            return Err(VoiceError::new(
                VoiceErrorCode::SynthesisFailed,
                format!("style row {index} is out of range for {ROWS} rows"),
            ));
        }
        Ok(flat[index * width..(index + 1) * width].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phoneme_tokens::{MAX_PHONEME_UNITS, SYMBOLS};

    /// A deterministic filler where every byte differs from its neighbour, so a wrong stride or a
    /// wrong byte order moves values instead of accidentally reproducing them.
    fn filler() -> Vec<u8> {
        (0..BYTES).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn constants_agree_with_the_asset() {
        assert_eq!(BYTES, 522_240, "the size pinned in reference.json and by fetch-offline-model");
        assert_eq!(ROWS, MAX_PHONEME_UNITS);
        assert_eq!(ROW_BYTES, 1024);
        assert_eq!(SYMBOLS.len(), 178);
    }

    #[test]
    fn rejects_anything_that_is_not_a_full_voice() {
        let bytes = filler();
        assert!(StyleMatrix::new(&bytes[..BYTES - 1]).is_err());
        assert!(StyleMatrix::new(&bytes[..BYTES / 2]).is_err());
        assert!(StyleMatrix::new(&[]).is_err());
        let mut short = bytes.clone();
        short.push(0);
        assert!(
            StyleMatrix::new(&short).is_err(),
            "a voice with one extra byte must not load as a voice with padding"
        );
        assert!(StyleMatrix::new(&bytes).is_ok());
    }

    #[test]
    fn row_decodes_little_endian_at_the_right_stride() {
        let bytes = filler();
        let matrix = StyleMatrix::new(&bytes).unwrap();
        assert_eq!(matrix.row_count(), ROWS);

        // Independent check of offset + byte order, built the same way but not through row().
        let expect_at = |row: usize, col: usize| {
            let start = row * ROW_BYTES + col * 4;
            let mut le = [0u8; 4];
            le.copy_from_slice(&bytes[start..start + 4]);
            f32::from_le_bytes(le)
        };
        let row7 = matrix.row(7).unwrap();
        assert_eq!(row7.len(), STYLE_DIM);
        for col in [0usize, 1, 3, 17, 128, 255] {
            assert_eq!(
                row7[col].to_bits(),
                expect_at(7, col).to_bits(),
                "row 7 col {col} decoded from the wrong offset or byte order"
            );
        }
        // Row 0 must not alias row 1: a stride bug shows up exactly here.
        assert_ne!(matrix.row(0).unwrap()[0].to_bits(), row7[0].to_bits());

        // The last row is addressable, and the one past it is an error rather than a panic.
        assert!(matrix.row(ROWS - 1).is_ok());
        let err = matrix.row(ROWS).unwrap_err();
        assert_eq!(err.code(), VoiceErrorCode::SynthesisFailed);
        let too_far = matrix.row(BYTES / ROW_BYTES).unwrap_err();
        assert!(too_far.to_string().contains("out of range"));
    }

    #[test]
    fn row_for_phonemes_follows_the_token_count_rule() {
        let bytes = filler();
        let matrix = StyleMatrix::new(&bytes).unwrap();
        // `encode` wraps with `$`; content tokens are the length minus the two pads, and the style
        // row is that count. Two sequences of different lengths must therefore read different rows.
        let short = crate::phoneme_tokens::encode("hələʊ");
        let long = crate::phoneme_tokens::encode("hələʊ ˈwɜːld əv ˈaʊdʒ");
        assert_ne!(short.len(), long.len(), "test needs two different token counts");
        let a = matrix.row_for_phonemes(&short).unwrap();
        let b = matrix.row_for_phonemes(&long).unwrap();
        assert_eq!(a, matrix.row(style_row(&short)).unwrap());
        assert_eq!(b, matrix.row(style_row(&long)).unwrap());
        assert_ne!(a[0].to_bits(), b[0].to_bits());
        assert_eq!(a.len(), STYLE_DIM);
    }

    #[test]
    fn flat_matrix_row_has_the_same_shape_and_its_own_bounds_check() {
        let flat: Vec<f32> = (0..ROWS * STYLE_DIM).map(|i| i as f32 * 0.25).collect();
        let row = StyleMatrix::row_from_flat(&flat, 3).unwrap();
        assert_eq!(row.len(), STYLE_DIM);
        assert_eq!(row[0], 192.0, "row 3 starts at float 768, and 768 * 0.25 is exact");
        assert!(StyleMatrix::row_from_flat(&flat, ROWS).is_err());
        assert!(StyleMatrix::row_from_flat(&flat[..STYLE_DIM], 0).is_err());
    }
}
