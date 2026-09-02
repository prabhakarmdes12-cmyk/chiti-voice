//! Guard on the **measured reference** for real offline synthesis (docs/ROADMAP_EMBEDDED.md
//! Step 1): `tests/fixtures/kokoro/`, produced by `scripts/spike-kokoro-offline.py`.
//!
//! ## What this test proves, in CI, today
//!
//! * the fixture is *internally* consistent -- the numbers in `reference.json` are the
//!   numbers you get by measuring the committed `reference_af_heart.wav`, so the two cannot
//!   drift apart or be edited into a story;
//! * the graph contract this repo will implement against is spelled once and pinned: three
//!   input tensors with exact names, `waveform` out, 24 kHz mono, 256-dim style row chosen
//!   by phoneme count, `$`-wrapped char-level ids bounded by the 512-token window;
//! * the reference audio is **not silence** and not a placeholder -- there is an explicit RMS
//!   floor, which is the exact failure mode the whole truth pass was about (30-byte model
//!   files and a silent mock, all of it documented as working).
//!
//! ## What it deliberately does NOT prove
//!
//! That a Rust engine reproduces the reference. That comparison needs ONNX Runtime and the
//! 88 MB model (`CHITI_KOKORO_MODEL=… cargo test -p vocal-core --features piper kokoro`), and
//! it belongs to the engine work, not here. Nothing in this file asserts that audible
//! synthesis exists in `crates/`: `vocal_core::REAL_SYNTHESIS_AVAILABLE` still says it does
//! not, and that flag is asserted against engine behaviour in `offline_synthesis.rs`.

use serde_json::Value;
use std::{fs, path::PathBuf};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/kokoro")
        .join(name)
}

fn reference() -> Value {
    let path = fixture("reference.json");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e} -- regenerate with scripts/spike-kokoro-offline.py", path.display()));
    serde_json::from_str(&text).expect("reference.json must parse")
}

/// Integer fields of the fixture. The JSON holds numbers as `f64`, and the two casts that
/// follow are the only place this file turns them back into integers.
fn int(v: &Value, key: &str) -> u64 {
    num(v, key) as u64
}

fn num(v: &Value, key: &str) -> f64 {
    v.get(key)
        .and_then(Value::as_f64)
        .unwrap_or_else(|| panic!("reference.json: expected numeric `{key}`"))
}

fn ustr(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("reference.json: expected string `{key}`"))
        .to_string()
}

/// The three tensor names are the whole interface. If a re-export renames them, the Rust
/// engine fails at run time deep inside ONNX Runtime; better to fail here.
#[test]
fn graph_contract_is_pinned() {
    let r = reference();
    let engine = &r["engine"];
    let inputs = engine["inputs"].as_object().expect("`engine.inputs` must be an object");
    let mut names: Vec<&str> = inputs.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["input_ids", "speed", "style"], "the ONNX I/O contract changed");
    assert_eq!(int(engine, "sample_rate"), 24000);
    assert_eq!(int(engine, "style_dim"), 256);
    assert_eq!(int(engine, "max_phoneme_units"), 510);
    assert!(ustr(engine, "output").contains("waveform"), "model must return waveform, i.e. PCM already: this export has no separate vocoder graph to feed");
}

/// 510 rows x 256 f32 = 522_240 bytes is what `expo-kokoro`'s `voice_data.slice(offset,
/// offset + 256)` assumes. A pack whose voice file is a different size is a different layout,
/// not the same engine with a longer tail.
#[test]
fn voice_vector_is_exactly_rows_of_style_dim() {
    let r = reference();
    let bytes = int(&r["voice"], "bytes");
    let style_dim = int(&r["engine"], "style_dim");
    let rows = int(&r["engine"], "max_phoneme_units");
    assert_eq!(bytes, rows * style_dim * 4, "a Kokoro voice vector is {rows} rows of {style_dim} f32");
}

/// Char-level ids, `$`-wrapped by the post-processor, bounded by the tokenizer's vocab and the
/// model's window. Reconstructing this wrongly is the single easiest way to get audio that is
/// *fluent and wrong*, which no RMS check would catch.
#[test]
fn phoneme_ids_are_wrapped_truncated_and_in_vocab() {
    let r = reference();
    let req = &r["request"];
    let ids: Vec<u64> = req["input_ids"].as_array().expect("`request.input_ids` must be an array")
        .iter().map(|v| v.as_u64().expect("ids must be integers")).collect();
    let n_tokens = int(req, "n_tokens") as usize;

    assert_eq!(ids.len(), n_tokens + 2, "expected `$ seq $`");
    assert_eq!(ids.first().copied(), Some(0));
    assert_eq!(ids.last().copied(), Some(0));
    assert!(n_tokens <= 509, "style rows are indexed 0..510; n_tokens={n_tokens} is out of range");

    let tok: Value = serde_json::from_str(&fs::read_to_string(fixture("tokenizer.json")).expect("tokenizer.json"))
        .expect("tokenizer.json must parse");
    let vocab: Vec<u64> = tok["model"]["vocab"].as_object().map(|m| m.values().filter_map(Value::as_u64).collect())
        .expect("vocab");
    assert_eq!(vocab.len(), 115, "Kokoro's char vocab is 115 entries incl. `$`");
    // The ids are NOT dense: 115 symbols occupy 0..=177. A `Vec` sized by vocab.len() would
    // panic on `q`/`ɡ`-class ids, so a lookup table must be sized by max id + 1 (178) or be a
    // map. This is the trap this assertion exists to make impossible to fall into silently.
    let upper = *vocab.iter().max().expect("non-empty vocab");
    assert!(ids.iter().all(|id| vocab.contains(id)),
        "an id outside the vocab's value set means the unknown-token path fired");
    assert!(upper < 256, "ids fit a u8 index table ({upper} is the largest) — if that changes, so does the pack format");
    assert_eq!(int(&tok["config"], "model_max_length"), 512);
}

/// Read the WAV the way `crates/vocal-core/src/wav.rs` writes it, and check the header against
/// the fixture. This is the anti-drift test: someone may replace reference_af_heart.wav with a
/// placeholder (this repo has form), and that must fail here rather than in a device demo.
#[test]
fn committed_audio_and_committed_numbers_describe_each_other() {
    let r = reference();
    let bytes = fs::read(fixture("reference_af_heart.wav")).expect("reference_af_heart.wav");
    assert!(bytes.len() > 44, "a 30-byte model file was this project's original sin; a 44-byte WAV is the same trick");
    // Byte comparisons are spelled slice-vs-slice (`&b"…"[..]`): the slice/array PartialEq
    // impls differ by reference-ness, and this test must not hinge on a trait bound that
    // nobody can compile here.
    assert_eq!(&bytes[0..4], &b"RIFF"[..]);
    assert_eq!(&bytes[8..12], &b"WAVE"[..]);
    assert_eq!(&bytes[36..40], &b"data"[..], "canonical header offsets: `data` chunk at 36");
    let rate = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    let channels = u16::from_le_bytes(bytes[22..24].try_into().unwrap());
    let bits = u16::from_le_bytes(bytes[34..36].try_into().unwrap());
    let data_len = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    assert_eq!((rate, channels, bits), (24000, 1, 16), "reference must be 24 kHz mono PCM16");

    let payload = &bytes[44..44 + data_len];
    let samples: Vec<i16> = payload
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(int(&r["expected"], "samples") as usize, samples.len());
    // Whole-number-of-samples stated as an equality rather than `payload.len() % 2 == 0`:
    // clippy's `manual_is_multiple_of` rejects the modulo, and this repo lints with
    // `-D warnings`, so the idiomatic form is the one that builds.
    assert_eq!(payload.len(), samples.len() * 2, "no trailing partial sample in the reference");

    let sum_sq: f64 = samples.iter().map(|&s| (f64::from(s) / 32768.0).powi(2)).sum();
    let rms = (sum_sq / samples.len() as f64).sqrt();
    let want = num(&r["expected"], "rms");
    assert!((rms - want).abs() / want < 0.01, "fixture says rms={want}, the audio measures {rms}");

    let peak = samples.iter().map(|&s| (f64::from(s) / 32768.0).abs()).fold(0.0f64, f64::max);
    assert!((peak - num(&r["expected"], "peak")).abs() < 0.01);
    assert!(peak > 0.05 && peak <= 1.0, "peak={peak}: either inaudible or clipping");
}

/// The named test. Everything else here is bookkeeping; this one is the claim.
#[test]
fn reference_is_audible_not_a_placeholder() {
    let r = reference();
    let rms = num(&r["expected"], "rms");
    assert!(rms > 0.01, "reference rms={rms} — that is silence, and silence must not be a valid reference");
    let dur = num(&r["expected"], "duration_s");
    let samples = num(&r["expected"], "samples");
    assert!((dur * 24000.0 - samples).abs() < 1.0, "duration and sample count disagree: {dur}s vs {samples} samples");
    assert!(dur > 1.0, "one second is too short to tell speech from a click");
}

/// Tolerances exist to absorb float accumulation order across ONNX Runtime builds, not to
/// absorb "did I even call the model". A 1.0 relative tolerance would accept silence.
#[test]
fn engine_tolerances_stay_tight() {
    let r = reference();
    let tol = &r["tolerances_for_the_rust_engine"];
    assert!(num(tol, "samples_relative") < 0.05, "sample-count tolerance loose enough to hide truncation");
    assert!(num(tol, "rms_relative") < 0.5, "rms tolerance loose enough to accept near-silence");
    assert!(ustr(tol, "why_not_bit_exact").len() > 40, "a tolerance without a stated reason is how drift becomes policy");
}
