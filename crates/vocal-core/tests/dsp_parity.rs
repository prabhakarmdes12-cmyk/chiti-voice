//! Grade vocal-core's DSP rules against **the reference path's own numbers**.
//!
//! `scripts/make-dsp-parity-fixtures.py` ran the real ONNX graph, kept the raw float32 waveform it
//! returned, and recorded what the reference implementation's arithmetic produces from it. Those
//! inputs and expected outputs are `tests/fixtures/kokoro/dsp_parity.json`, and this file asserts
//! that Rust reproduces them — equality, not tolerance, because the two rules under test *are*
//! rounding decisions and a tolerance is where they go to die:
//!
//! * `clamp(floor(x * 32767), -32768, 32767)` — the case named `graph_output_head` is literally the
//!   first 512 samples of the run that produced `assets/offline-spike/af_heart-en_us.wav`, so passing
//!   it means the Rust encoder reproduces committed reference audio, sample for sample.
//! * `gain = min(target_linear / rms, ceiling / peak)` with the peak ceiling allowed to win and
//!   amplification capped at `max_gain_db` — the `target_0dbfs_ceiling_0.95` case is exactly the
//!   ceiling-binding situation that made 8 of 54 stock voices dangerous (`PERSONA_STYLE_VECTORS.md`).
//!
//! ## What this does NOT prove
//!
//! It does not prove that inference exists in `crates/`, and it is not a stand-in for that: no ONNX
//! session runs here, `vocal_core::REAL_SYNTHESIS_AVAILABLE` is still `false`, and the fixture's float
//! data is committed precisely so CI needs no model and no network. What gets de-risked is the part
//! that used to be argued about — byte layout of the style matrix, the direction of the row-index
//! rule, and whether `floor` or `round` is the house rule.

use serde_json::Value;
use std::{fs, path::PathBuf};
use vocal_core::audio_levels::{self, LoudnessSpec};
use vocal_core::style_matrix::{self, StyleMatrix};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/kokoro")
        .join(name)
}

fn parity() -> Value {
    let path = fixture("dsp_parity.json");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} must exist (scripts/make-dsp-parity-fixtures.py): {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()))
}

fn bits_to_samples(value: &Value) -> Vec<f32> {
    value
        .as_array()
        .expect("input_bits must be an array of u32 bit patterns")
        .iter()
        .map(|v| {
            let bits = v
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .expect("each bit pattern must be a u32");
            f32::from_bits(bits)
        })
        .collect()
}

fn expected_pcm(value: &Value) -> Vec<i16> {
    value
        .as_array()
        .expect("expected_i16 must be an array of int16 values")
        .iter()
        .map(|v| {
            v.as_i64()
                .and_then(|i| i16::try_from(i).ok())
                .expect("each expected sample must be an i16")
        })
        .collect()
}

fn cases<'a>(doc: &'a Value, key: &str) -> &'a [Value] {
    doc[key]
        .as_array()
        .unwrap_or_else(|| panic!("fixture must carry a `{key}` array"))
        .as_slice()
}

fn name_of(case: &Value) -> String {
    case["name"].as_str().unwrap_or("<unnamed case>").to_string()
}

#[test]
fn fixture_is_tied_to_a_real_graph_run() {
    let doc = parity();
    let prov = &doc["provenance"];
    let bytes = prov["model"]["bytes"].as_u64().expect("model.bytes");
    // A placeholder model is the failure mode this repo was rebuilt around, so the fixture itself
    // has to prove it came from the 88 MB graph and not from a stub.
    assert!(
        bytes > 80_000_000,
        "the parity fixture claims a model of {bytes} bytes; real Kokoro-82M int8 is ~92 MB"
    );
    let sha = prov["model"]["sha256"].as_str().expect("model.sha256");
    assert_eq!(sha.len(), 64, "sha256 as hex");
    assert_eq!(
        prov["voice"]["bytes"].as_u64().expect("voice.bytes") as usize,
        style_matrix::BYTES,
        "the voice used to build these fixtures must be a full 510 x 256 style matrix"
    );
    assert_eq!(
        prov["content_tokens"].as_u64().expect("content_tokens") as usize,
        prov["style_row_used"].as_u64().expect("style_row_used") as usize,
        "the row index *is* the content token count; if the generator disagrees with the contract, stop"
    );
    assert_eq!(prov["speed"].as_f64(), Some(1.0), "fixtures are generated at unity speed");
}

#[test]
fn pcm16_rule_reproduces_the_reference_on_real_graph_output() {
    let doc = parity();
    for case in cases(&doc, "pcm16_cases") {
        let name = name_of(case);
        let samples = bits_to_samples(&case["input_bits"]);
        let want = expected_pcm(&case["expected_i16"]);
        assert_eq!(samples.len(), want.len(), "{name}: fixture is self-inconsistent");
        let got = audio_levels::to_pcm16(&samples);
        assert_eq!(got, want, "{name}: the float -> int16 rule diverged from the reference");
        // The same rule must be reachable through the strict path for identical data.
        let strict = audio_levels::encode_strict(&samples, 1.0).expect("finite samples encode");
        assert_eq!(strict, want, "{name}: strict and lenient disagreed on finite input");
    }
}

#[test]
fn loudness_normalisation_matches_the_python_path_case_for_case() {
    let doc = parity();
    for case in cases(&doc, "loudness_cases") {
        let name = name_of(case);
        let target = case["target_dbfs"].as_f64().expect("target_dbfs") as f32;
        let ceiling = case["peak_ceiling"].as_f64().expect("peak_ceiling") as f32;
        let samples = bits_to_samples(&case["input_bits"]);
        let want = expected_pcm(&case["expected_i16"]);

        let spec = LoudnessSpec {
            target_dbfs: target,
            peak_ceiling: ceiling,
            max_gain_db: audio_levels::DEFAULT_MAX_GAIN_DB,
        };
        let (got, applied) = audio_levels::normalise(&samples, &spec);
        assert_eq!(got.len(), want.len(), "{name}: sample count changed");
        assert_eq!(got, want, "{name}: normalised PCM diverged from the reference");

        assert_eq!(
            applied.ceiling_limited,
            case["expected_limited"].as_bool().expect("expected_limited"),
            "{name}: the wrong limit was reported as binding"
        );
        assert!(!applied.gain_limited, "{name}: generator must not emit cases the cap would bind on");

        // The ceiling must hold on the encoded output, up to the one-LSB asymmetry that flooring a
        // negative sample creates (matched with the reference on purpose).
        let bound = i64::from(audio_levels::ceiling_in_samples(ceiling)) + 1;
        let worst = got.iter().fold(0i64, |acc, s| acc.max(i64::from(*s).abs()));
        assert!(worst <= bound, "{name}: peak {worst} escaped the ceiling bound {bound}");

        if let Some(margin) = case.get("boundary_margin").and_then(Value::as_f64) {
            assert!(
                margin > 1e-3,
                "{name}: a scaled sample sits {margin:.2e} from a floor boundary, so this fixture \
                 cannot pin a rounding rule; regenerate with a different window"
            );
        }
    }
}

#[test]
fn the_row_index_rule_and_the_asset_size_agree_with_the_contract() {
    // These are the two numbers a Rust engine has to get right about style vectors, and both were
    // measured rather than inferred (522,240 B; row = content token count, clamped to the rows that
    // exist). If `phoneme_tokens` and `style_matrix` ever disagree, this fails at once.
    assert_eq!(style_matrix::BYTES, 510 * 256 * 4);
    assert_eq!(style_matrix::ROWS, 510);
    assert_eq!(
        vocal_core::phoneme_tokens::n_tokens(&[0u16; 2]),
        0,
        "a bare '$ $' wrap has no content tokens, so row 0"
    );

    let long: Vec<u16> = std::iter::once(0)
        .chain((0..600).map(|i| 1 + (i % 100) as u16))
        .chain(std::iter::once(0))
        .collect();
    assert_eq!(
        vocal_core::phoneme_tokens::n_tokens(&long),
        style_matrix::ROWS - 1,
        "an over-long utterance must clamp to the last row a voice actually has"
    );
    let bytes = vec![7u8; style_matrix::BYTES];
    let matrix = StyleMatrix::new(&bytes).expect("a 522,240-byte buffer is exactly one voice vector");
    let row = matrix
        .row_for_phonemes(&long)
        .expect("the clamped index must stay addressable");
    assert_eq!(row.len(), 256);
    assert!(
        row.iter().all(f32::is_finite),
        "0x07 repeated must decode to tiny numbers, never to NaN"
    );
    // And the *next* row index past the matrix is an error, not a panic: `n_tokens` clamps, but a
    // caller that builds an index by hand must get a rejection.
    assert!(matrix.row(style_matrix::ROWS).is_err());
}
