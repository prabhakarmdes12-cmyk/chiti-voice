//! Character-level tokenisation for the Kokoro-family ONNX contract.
//!
//! This is the half of Step 1 that can be **verified without ONNX Runtime, without a model and
//! without a network**: it reproduces `input_ids` from the measured reference fixture
//! (`tests/fixtures/kokoro/reference.json`, produced by `scripts/spike-kokoro-offline.py`), and
//! `tests/kokoro_tokens.rs` asserts that equality. When `ort` arrives it sits on top of these
//! functions instead of re-deriving rules from prose.
//!
//! Four measured oddities are encoded here, one per section:
//!
//! 1. **The id space is sparse.** 115 symbols occupy ids `0..=177`. A `Vec` sized by
//!    `vocab.len()` panics on the first high id; the table is sized by the largest id, which is
//!    why it is 178 entries long and not 115.
//! 2. **Normalisation is a vocabulary filter, not a regex.** Upstream ships a
//!    `[^...]` whitelist regex; checked exhaustively against `tokenizer.json`, that whitelist and
//!    the vocab keys are the *same 115 characters*, so "drop what is not in the vocab" is
//!    equivalent — and needs no regex engine on an embedded target. The test re-derives both sets
//!    from the fixture so the equivalence cannot rot silently.
//! 3. **The wrap happens before truncation.** `[pad] ++ seq ++ [pad]` is then cut to
//!    `MAX_TOKENS`, so an over-long utterance loses its *trailing* `$` and keeps the leading one.
//!    That is what the reference implementation does; matching it exactly is the whole point.
//! 4. **The style row is chosen by token count** — `n_tokens * STYLE_DIM`, the line in the
//!    reference code that looks arbitrary. It means prosody follows *utterance length*, so how a
//!    caller chunks text changes the voice. See [`style_offset`].
//!
//! G2P is deliberately **not** here: that is where the licensing decision lands (espeak-ng is
//! GPL-3.0-or-later), so the engine takes phonemes. See `docs/ROADMAP_EMBEDDED.md` §3.

use crate::error::{VoiceError, VoiceErrorCode, VoiceResult};

/// `model_max_length`: the window the graph is exported for.
pub const MAX_TOKENS: usize = 512;
/// Rows in a voice vector: one style row per possible phoneme count.
pub const MAX_PHONEME_UNITS: usize = 510;
/// Dims of one style row (`float32[1, 256]` in the graph).
pub const STYLE_DIM: usize = 256;
/// The padding / wrap symbol `$`, id 0.
pub const PAD: u16 = 0;
/// The largest id in use (177) — deliberately not `vocab.len() - 1`.
pub const MAX_ID: u16 = 177;
/// `float32` samples the reference `waveform` is scaled by: 32767. The reference floors after
/// scaling, and `audio_levels::scale_to_i16` now does exactly that; `wav.rs` used to round, which
/// was a permanent <= 1 LSB difference against every WAV in this repo, so the rule lives in one
/// place and `tests/dsp_parity.rs` grades it against real graph output instead of an example.
pub const PCM_SCALE: f32 = 32767.0;

/// `SYMBOLS[id]` is the character for that id, or `'\0'` for an id the vocab does not use.
/// Generated from the measured `tokenizer.json`; the tests fail if table and fixture diverge.
pub const SYMBOLS: &[char; 178] = &[
    '$', ';', ':', ',', '.', '!', '?', '\0',
    '\0', '\u{2014}', '\u{2026}', '"', '(', ')', '\u{201c}', '\u{201d}',
    ' ', '\u{303}', '\u{2a3}', '\u{2a5}', '\u{2a6}', '\u{2a8}', '\u{1d5d}', '\u{ab67}',
    'A', 'I', '\0', '\0', '\0', '\0', '\0', 'O',
    '\0', 'Q', '\0', 'S', 'T', '\0', '\0', 'W',
    '\0', 'Y', '\u{1d4a}', 'a', 'b', 'c', 'd', 'e',
    'f', '\0', 'h', 'i', 'j', 'k', 'l', 'm',
    'n', 'o', 'p', 'q', 'r', 's', 't', 'u',
    'v', 'w', 'x', 'y', 'z', '\u{251}', '\u{250}', '\u{252}',
    '\u{e6}', '\0', '\0', '\u{3b2}', '\u{254}', '\u{255}', '\u{e7}', '\0',
    '\u{256}', '\u{f0}', '\u{2a4}', '\u{259}', '\0', '\u{25a}', '\u{25b}', '\u{25c}',
    '\0', '\0', '\u{25f}', '\0', '\u{261}', '\0', '\0', '\0',
    '\0', '\0', '\0', '\u{265}', '\0', '\u{268}', '\u{26a}', '\u{29d}',
    '\0', '\0', '\0', '\0', '\0', '\0', '\u{26f}', '\u{270}',
    '\u{14b}', '\u{273}', '\u{272}', '\u{274}', '\u{f8}', '\0', '\u{278}', '\u{3b8}',
    '\u{153}', '\0', '\0', '\u{279}', '\0', '\u{27e}', '\u{27b}', '\0',
    '\u{281}', '\u{27d}', '\u{282}', '\u{283}', '\u{288}', '\u{2a7}', '\0', '\u{28a}',
    '\u{28b}', '\0', '\u{28c}', '\u{263}', '\u{264}', '\0', '\u{3c7}', '\u{28e}',
    '\0', '\0', '\0', '\u{292}', '\u{294}', '\0', '\0', '\0',
    '\0', '\0', '\0', '\0', '\u{2c8}', '\u{2cc}', '\u{2d0}', '\0',
    '\0', '\0', '\u{2b0}', '\0', '\u{2b2}', '\0', '\0', '\0',
    '\0', '\u{2193}', '\0', '\u{2192}', '\u{2197}', '\u{2198}', '\0', '\0',
    '\0', '\u{1d7b}',
];

/// Look up a character's id. `None` for anything outside the vocab; [`encode`] turns that
/// `None` into a [`PAD`] token, exactly as upstream's `vocab.get(c, pad)` does, so an unmapped
/// character costs a slot without shifting anything.
///
/// The upstream pipeline maps an unmapped character to `pad` instead (`vocab.get(c, pad)`): the
/// character becomes a [`PAD`] *token*, keeping the sequence's length. This crate's [`encode`] does
/// not — it filters such characters out first, so the sequence is shorter.
///
/// That is not cosmetic, because the style row is selected by token count: one dropped character shifts
/// the row as well as removing a sound. `tests/kokoro_tokens.rs` holds the consequence in check for the
/// reference fixtures (it asserts they need no stripping at all, so `encode` cannot have diverged from
/// the run that produced them), and nothing here makes the general case faithful. Either `encode` learns
/// the `pad` behaviour, or this divergence gets a name and a reason. What it must not keep is a doc
/// comment claiming the two agree.
pub fn id_for(ch: char) -> Option<u16> {
    // Linear scan: ids are sparse and the table is short (178), so a full 512-token utterance
    // costs ~90k comparisons — noise against an inference budget measured in seconds.
    SYMBOLS.iter().position(|&c| c == ch).map(|i| i as u16)
}

/// Drop every character the vocabulary does not contain.
///
/// This is NOT what [`encode`] does with an unmapped character, and the two must not be confused:
/// filtering changes the sequence length, and the style row is the sequence length, so a filter in the
/// synthesis path moves prosody as well as removing a sound. It survives as a *reporting* helper (a
/// caller deciding whether a string is spellable at all) and as the fence in
/// `tests/kokoro_tokens.rs`, which asserts the reference fixtures need no filtering so that the ids
/// recorded there are comparable with the table as it stands.
pub fn strip_to_vocab(phonemes: &str) -> String {
    phonemes.chars().filter(|c| id_for(*c).is_some()).collect()
}

/// The characters this string uses that the table does not have, deduplicated, in order of first use.
///
/// Empty for anything a Kokoro-class graph can spell. Callers that *author* phoneme strings -- a pack's
/// `pronunciation_overrides`, a hand-written test fixture -- should treat non-empty as an error, because
/// the synthesiser will not reject those characters, it will pad them: see [`encode`].
#[must_use]
pub fn unmapped_symbols(phonemes: &str) -> Vec<char> {
    let mut seen: Vec<char> = Vec::new();
    for c in phonemes.chars() {
        if id_for(c).is_none() && !seen.contains(&c) {
            seen.push(c);
        }
    }
    seen
}

/// `$ seq $` — pad-wrapped, truncated to [`MAX_TOKENS`]. Input is *phonemes*, not text.
///
/// The result is always two ids longer than the input's character count -- `chars + 2` -- until it hits
/// the cap, so a caller allocating `input_ids` sizes it from the encoded length, never from the text.
/// The crate's own `phoneme_framing` test pins that, and `apps/sample-reader` prints `framed=` next
/// to `units=` so an integrator can see they are two different numbers rather than one of them.
///
/// A character outside the table becomes [`PAD`], which is what the reference does (`vocab.get(c, pad)`)
/// and therefore what the recorded fixtures were produced by. It used to be filtered out here, which is
/// a quiet way to change a voice: dropping a character shortens the sequence, and the style row is
/// selected by sequence length, so every row from the dropped character onward shifted by one. Padded
/// instead, the row is the caller's length and the divergence disappears; where a string must be
/// *spellable* rather than merely representable, ask [`unmapped_symbols`] and refuse it at load --
/// `Persona::check_overrides_encodable` is that check for the one input a pack asserts by name.
pub fn encode(phonemes: &str) -> Vec<u16> {
    let mut ids = Vec::with_capacity(phonemes.chars().count() + 2);
    ids.push(PAD);
    ids.extend(phonemes.chars().map(|c| id_for(c).unwrap_or(PAD)));
    ids.push(PAD);
    ids.truncate(MAX_TOKENS);
    ids
}

/// Content tokens the graph sees: wraps removed, clamped to the rows a voice vector has.
pub fn n_tokens(encoded: &[u16]) -> usize {
    encoded.len().saturating_sub(2).min(MAX_PHONEME_UNITS - 1)
}

/// Which style row this sequence uses.
pub fn style_row(encoded: &[u16]) -> usize {
    n_tokens(encoded)
}

/// Float offset into a voice vector for this sequence (multiply by 4 for bytes).
pub fn style_offset(encoded: &[u16]) -> usize {
    style_row(encoded) * STYLE_DIM
}

/// A voice vector is exactly `510 * 256` `f32`s. Exact, not a minimum: a truncated asset would
/// otherwise read as "a valid voice with fewer rows", and short-model placeholders are the
/// failure mode this project was rebuilt around.
pub fn check_voice_bytes(len: usize) -> VoiceResult<()> {
    const EXPECTED: usize = MAX_PHONEME_UNITS * STYLE_DIM * 4;
    if len == EXPECTED {
        Ok(())
    } else {
        Err(VoiceError::new(
            VoiceErrorCode::PackInvalidFormat,
            format!(
                "Kokoro voice vector is {len} bytes, expected {EXPECTED} \
                 ({MAX_PHONEME_UNITS} rows x {STYLE_DIM} x f32)"
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unmapped_character_becomes_a_pad_token_and_the_row_is_unaffected() {
        // "d\u{0261}u g": script-g is in the table, ASCII 'g' is not -- which is the trap, because
        // espeak-style IPA writes /g/ as the ASCII letter. `encode` now behaves like the reference
        // (`vocab.get(c, pad)`), so the character costs a token instead of vanishing: the sequence keeps
        // the caller's length, and so does the style row derived from it. Asserted with numbers because
        // "one character" is exactly what a future refactor of the vocabulary filter would erase, and
        // because the alternative -- the old behaviour -- shifted every row by one silently.
        let input = "d\u{0261}u g";
        let ids = encode(input);
        assert_eq!(
            ids.len(),
            input.chars().count() + 2,
            "an unmapped character must occupy a slot, not shorten the sequence"
        );
        assert_eq!(ids[input.chars().count()], PAD, "the unmapped 'g' became PAD, in place");
        assert_eq!(n_tokens(&ids), input.chars().count(), "and the row index is the caller's length");
        assert_eq!(unmapped_symbols(input), vec!['g'], "which is reportable, and must be reported");
        assert_eq!(strip_to_vocab(input).chars().count(), 4, "the filter is the other behaviour, kept for reporting");
    }

    #[test]
    fn table_is_sparse_exactly_as_documented() {
        let used = SYMBOLS.iter().filter(|c| **c != '\0').count();
        assert_eq!(used, 115, "the measured vocab is 115 symbols");
        let (len, max_id) = (SYMBOLS.len(), MAX_ID as usize);
        assert_eq!(len, max_id + 1, "sized by largest id, not by count");
        assert_eq!(SYMBOLS[PAD as usize], '$');
        assert!(used < SYMBOLS.len(), "if the ids ever become dense, the comment above is a lie");
        assert!(id_for('\u{26a1}').is_none(), "a symbol outside the vocab must not map");

        // The table carries U+0261 (script-g), not ASCII 'g' -- which is what a phonemiser emitting
        // plain IPA produces for /g/. Budgeting and encoding both rest on that asymmetry, so it is
        // asserted rather than assumed: the alternative is silently losing every 'g' in a language.
        assert!(
            id_for('g').is_none() && id_for('\u{0261}').is_some(),
            "the vocab has script-g only; an espeak-style IPA 'g' is an unmapped character"
        );
    }

    #[test]
    fn wrap_then_truncate_keeps_the_leading_pad() {
        let long = "a".repeat(MAX_TOKENS + 40);
        let ids = encode(&long);
        assert_eq!(ids.len(), MAX_TOKENS);
        assert_eq!(ids[0], PAD);
        assert_ne!(ids[MAX_TOKENS - 1], PAD, "truncation eats the trailing wrap, as upstream does");
    }

    #[test]
    fn style_row_tracks_token_count_and_clamps() {
        assert_eq!(n_tokens(&[PAD]), 0, "an empty sequence must not index the last row");
        assert_eq!(n_tokens(&[PAD, 1, 2, 3, PAD]), 3);
        assert_eq!(style_offset(&[PAD, 1, 2, 3, PAD]), 3 * STYLE_DIM);
        let full = vec![PAD; MAX_TOKENS + 1];
        assert_eq!(n_tokens(&full), MAX_PHONEME_UNITS - 1, "clamped to the last available row");
    }

    #[test]
    fn voice_length_check_is_exact_not_minimum() {
        assert!(check_voice_bytes(MAX_PHONEME_UNITS * STYLE_DIM * 4).is_ok());
        for len in [0usize, 4, 522_236, 522_244] {
            let err = check_voice_bytes(len).unwrap_err();
            assert_eq!(err.code(), VoiceErrorCode::PackInvalidFormat);
            assert!(err.message().contains("expected"), "the message must state the expected size");
        }
    }
}
