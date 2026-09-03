//! `VOICE_INV_005` — Deterministic Core.
//!
//! ## Read this first: what these tests can and cannot prove
//!
//! The invariant asks for bit-identical PCM for identical input. What is checkable today is the
//! deterministic half of the pipeline -- tokenisation, utterance planning, loudness, WAV framing -- and
//! `MockEngine`, which emits digital silence. So passing here proves that the code *this repository
//! owns* is a pure function of its input. It does **not** prove that ONNX Runtime is deterministic,
//! which is the part the invariant ultimately cares about: that needs real inference (Step 2) and, per
//! the invariant's own test approach, a run across the NANO/LITE/STUDIO tiers, which do not exist yet as
//! selectable models. When inference lands, add the tier loop here rather than declaring determinism
//! proven by the tests in this file.
//!
//! Two ways this could have been written as theatre, both avoided on purpose:
//!
//! - Re-rendering twice inside one process proves little on its own, so the checks are split: the
//!   *pure* stages are compared structurally (tokens, units, style row), and the engine is compared
//!   byte-for-byte, including across two separate engine instances -- a lazy per-instance cache or a
//!   seed captured at `initialize()` would survive the first and fail the second.
//! - A test that "checks for randomness" by re-running synthesis cannot see nondeterminism that only
//!   appears across processes or machines. So the structural guard is a source scan: if a clock, an
//!   RNG, or a pid enters the shipped path, that is the non-reproducible operation the invariant forbids,
//!   and it is caught in the file where it was introduced instead of in a flaky diff three CI runs later.

use vocal_core::engine::mock::MockEngine;
use vocal_core::engine::VoiceEngine;
use vocal_core::phoneme_tokens::encode;
use vocal_core::synthesis::SynthesisRequest;
use vocal_core::utterance_plan::{plan_pieces, Piece, PlanPolicy};

const SENTENCE: &str = "Welcome to Chiti Vocal Runtime. Every persona here is generated on the device.";
const VOICE: &str = "tara-mock";

async fn render(engine: &mut MockEngine) -> (Vec<u8>, Vec<u64>) {
    let request = SynthesisRequest::new(VOICE, SENTENCE);
    let response = engine.synthesize(&request).await.unwrap();
    let metadata = vec![
        response.metadata.duration_ms as u64,
        response.metadata.sample_rate as u64,
        response.metadata.channels as u64,
        response.metadata.bit_depth as u64,
        response.audio.len() as u64,
    ];
    (response.audio, metadata)
}

#[tokio::test]
async fn ten_renders_in_one_engine_are_bit_identical() {
    let mut engine = MockEngine::new();
    engine.initialize().await.unwrap();

    let (first_bytes, first_meta) = render(&mut engine).await;
    for i in 1..10 {
        let (bytes, meta) = render(&mut engine).await;
        assert_eq!(bytes, first_bytes, "render {i} differs from render 0 in audio bytes");
        assert_eq!(meta, first_meta, "render {i} differs from render 0 in reported metadata");
    }
    assert!(!first_bytes.is_empty(), "an empty render compares equal to everything");
}

#[tokio::test]
async fn a_second_engine_instance_renders_the_same_bytes() {
    // Same input, independent instance: catches state captured at initialize() (a seed, a cached
    // buffer, a counter folded into the style row) that a single-instance loop cannot see.
    let mut first = MockEngine::new();
    first.initialize().await.unwrap();
    let mut second = MockEngine::new();
    second.initialize().await.unwrap();

    let (a, a_meta) = render(&mut first).await;
    let (b, b_meta) = render(&mut second).await;
    assert_eq!(a, b, "two engines of the same kind disagree on identical input");
    assert_eq!(a_meta, b_meta);
}

#[test]
fn the_pure_stages_are_functions_of_their_input() {
    let phonemes = "ˈwɛləm tə ˈtʃɪti ˈvəʊkəl ˈrʌntaɪm";
    assert_eq!(encode(phonemes), encode(phonemes), "encode is not a pure function");

    let pieces: Vec<Piece> = phonemes.split_whitespace().map(Piece::phonemes).collect();
    let policy = PlanPolicy::default();
    let a = plan_pieces(&pieces, &policy).unwrap();
    let b = plan_pieces(&pieces, &policy).unwrap();

    assert!(!a.utterances.is_empty(), "a plan with no utterances proves nothing");
    assert_eq!(a.utterances.len(), b.utterances.len(), "planning split differently");
    for (x, y) in a.utterances.iter().zip(b.utterances.iter()) {
        assert_eq!(x.phonemes, y.phonemes);
        assert_eq!(x.units, y.units);
        assert_eq!(x.style_row, y.style_row, "the style row is chosen by token count, so a \
                 difference here means the index moved -- that is a silent voice change, not a flake");
    }
}

#[test]
fn the_deterministic_modules_reference_no_clock_or_rng() {
    // VOICE_INV_005's enforcement mechanism is "no rand crate usage anywhere in the synthesis
    // pipeline", which a dependency can satisfy while an `Instant::now()` in a hot path defeats it.
    // So the scan is textual and it covers the shipped code only: `#[cfg(test)]` modules are cut off,
    // because a test naming a temp directory after the pid is not a nondeterministic render.
    const FORBIDDEN: [&str; 5] = [
        "Instant::now",
        "SystemTime",
        "rand::",
        "thread_rng",
        "getrandom",
    ];
    let sources = [
        include_str!("../src/phoneme_tokens.rs"),
        include_str!("../src/utterance_plan.rs"),
        include_str!("../src/style_matrix.rs"),
        include_str!("../src/audio_levels.rs"),
        include_str!("../src/wav.rs"),
        include_str!("../src/persona.rs"),
    ];
    for src in sources {
        let shipped = src.split("#[cfg(test)]").next().unwrap_or(src);
        for needle in FORBIDDEN {
            assert!(
                !shipped.contains(needle),
                "the deterministic path now references {needle}; a clock or an RNG in here makes \
                 certified audio unreproducible, which is exactly what this test exists to catch"
            );
        }
    }
}
