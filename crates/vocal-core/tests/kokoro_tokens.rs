//! `phoneme_tokens` against the **measured reference**, with no ONNX Runtime and no model.
//!
//! This is the interesting part of "verified": the Rust tokeniser's output is compared to the
//! exact id array that produced real audio in `docs/research/KOKORO_OFFLINE_SPIKE.md`. A Rust
//! engine that calls ONNX Runtime and gets these bytes right is on the path to speech; one that
//! doesn't will produce fluent, mispronounced audio — the failure mode no RMS check catches.
//!
//! It deliberately does not test synthesis. Nothing here implies `REAL_SYNTHESIS_AVAILABLE`.

use serde_json::Value;
use std::{fs, path::PathBuf};
use vocal_core::phoneme_tokens::{
    check_voice_bytes, encode, n_tokens, strip_to_vocab, style_offset, style_row, MAX_ID,
    MAX_PHONEME_UNITS, MAX_TOKENS, STYLE_DIM,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/kokoro")
        .join(name)
}

fn reference() -> Value {
    let path = fixture("reference.json");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e} (regenerate with scripts/spike-kokoro-offline.py)", path.display()));
    serde_json::from_str(&text).expect("reference.json parses")
}

fn ids_of(v: &Value) -> Vec<u16> {
    v.as_array()
        .expect("input_ids must be an array")
        .iter()
        .map(|x| x.as_u64().expect("ids are unsigned") as u16)
        .collect()
}

/// The claim this whole file exists for.
#[test]
fn reproduces_the_measured_input_ids_exactly() {
    let r = reference();
    let phonemes = r["request"]["phonemes"].as_str().expect("request.phonemes");
    let want = ids_of(&r["request"]["input_ids"]);
    let got = encode(phonemes);
    assert_eq!(
        got, want,
        "tokenisation diverged from the run that produced the reference audio \
         ({} rust ids vs {} fixture ids)",
        got.len(),
        want.len()
    );
}

#[test]
fn agrees_on_style_row_selection() {
    let r = reference();
    let phonemes = r["request"]["phonemes"].as_str().unwrap();
    let ids = encode(phonemes);
    assert_eq!(n_tokens(&ids), r["request"]["n_tokens"].as_u64().unwrap() as usize);
    assert_eq!(style_row(&ids), r["request"]["style_row"].as_u64().unwrap() as usize);
    assert_eq!(
        style_offset(&ids),
        r["request"]["style_row"].as_u64().unwrap() as usize * STYLE_DIM
    );
}

#[test]
fn constants_match_the_measured_graph() {
    let r = reference();
    let e = &r["engine"];
    assert_eq!(STYLE_DIM, e["style_dim"].as_u64().unwrap() as usize);
    assert_eq!(MAX_PHONEME_UNITS, e["max_phoneme_units"].as_u64().unwrap() as usize);
    assert_eq!(
        MAX_PHONEME_UNITS * STYLE_DIM * 4,
        r["voice"]["bytes"].as_u64().unwrap() as usize,
        "the voice asset the reference used must be exactly the size this crate assumes"
    );
    assert!(check_voice_bytes(r["voice"]["bytes"].as_u64().unwrap() as usize).is_ok());
    let tok: Value = serde_json::from_str(&fs::read_to_string(fixture("tokenizer.json")).unwrap()).unwrap();
    assert_eq!(
        tok["config"]["model_max_length"].as_u64().unwrap() as usize,
        MAX_TOKENS
    );
    // Compared against the *fixture's* symbol count rather than a literal, so this is a real
    // check on the data (and not an assertion about two constants, which clippy rejects as
    // something that cannot fail).
    let symbols: Value = serde_json::from_str(&fs::read_to_string(fixture("tokenizer.json")).unwrap()).unwrap();
    let vocab_len = symbols["model"]["vocab"].as_object().map(|m| m.len()).unwrap();
    assert!(MAX_ID as usize > vocab_len,
        "ids must stay sparse (largest id {} vs {} symbols) or the table comment in phoneme_tokens.rs is stale",
        MAX_ID, vocab_len);
}

/// The whitelist-vs-vocab equivalence, checked on the observable side: the reference phoneme
/// string must survive the filter unchanged, and each character must map to a real id.
/// (The full set equality — 115 whitelist chars == 115 vocab keys — was verified against
/// `tokenizer.json` when this table was generated; re-run that check if the vocab ever changes.)
#[test]
fn reference_phonemes_are_already_canonical() {
    let r = reference();
    let phonemes = r["request"]["phonemes"].as_str().unwrap();
    assert_eq!(strip_to_vocab(phonemes), phonemes, "the reference string must need no stripping, else encode() diverged from the run that produced it");
    let ids = encode(phonemes);
    assert!(ids.iter().all(|&id| id <= MAX_ID));
    // Every interior id came from a real symbol, so none of them is the pad id: an interior pad
    // would mean an unmapped character quietly became padding.
    assert!(
        ids[1..ids.len() - 1].iter().all(|&id| id != 0),
        "interior padding means a character was dropped silently"
    );
}
