//! Pipeline + invariant tests for vocal-core.
//!
//! ## Read this first: what these tests can and cannot prove
//!
//! They exercise `MockEngine`, which emits digital silence. Passing them proves the
//! request/response plumbing, error typing, and dependency hygiene are intact. It does
//! **not** prove that speech synthesis works, that audio is audible, or that the runtime
//! is offline.
//!
//! Offline-ness in particular cannot be proven by an in-process assertion, because
//! `MockEngine` contains no network code to begin with — a test that "checks for network
//! calls" while running a silent mock is vacuous. The enforceable version of
//! `VOICE_INV_001` here is (a) the dependency/source scan in this file, which fails if a
//! network client ever appears in the crate, and (b) the CI job that runs the suite inside
//! a network-isolated namespace (`unshare -rn`). Don't add a test here that merely re-runs
//! synthesis and calls it an offline proof.

use vocal_core::engine::mock::MockEngine;
use vocal_core::engine::piper::PiperEngine;
use vocal_core::engine::VoiceEngine;
use vocal_core::error::VoiceErrorCode;
use vocal_core::synthesis::{SynthesisFormat, SynthesisRequest};

#[tokio::test]
async fn mock_engine_synthesizes_silence() {
    let mut engine = MockEngine::new();
    engine.initialize().await.unwrap();
    assert!(matches!(
        engine.health().await.unwrap(),
        vocal_core::engine::EngineHealth::Healthy
    ));

    let request = SynthesisRequest::new("tara-mock", "Welcome to Chiti Vocal Runtime.")
        .with_format(SynthesisFormat::PcmF32);
    let response = engine.synthesize(&request).await.unwrap();

    assert!(!response.audio.is_empty());
    assert_eq!(response.metadata.sample_rate, 22050);
    assert_eq!(response.metadata.channels, 1);
    assert_eq!(response.metadata.bit_depth, 32);

    // The claim that matters for silence: EVERY byte is zero.
    assert!(
        response.audio.iter().all(|b| *b == 0),
        "MockEngine is documented to emit silence; non-zero bytes means it changed"
    );

    // Byte length must agree with the reported duration/sample rate — previously this
    // test computed `expected_bytes` and then asserted `len() > 0`, which is vacuous.
    let expected_samples =
        response.metadata.duration_ms as usize * response.metadata.sample_rate as usize / 1000;
    let expected_bytes = expected_samples * response.metadata.channels as usize * 4;
    assert!(
        response.audio.len() >= expected_bytes.saturating_sub(4),
        "audio {} bytes is not consistent with {} ms @ {} Hz",
        response.audio.len(),
        response.metadata.duration_ms,
        response.metadata.sample_rate
    );
    assert_eq!(response.format, SynthesisFormat::PcmF32);
}

#[tokio::test]
async fn voices_have_distinct_durations_for_distinct_lengths() {
    let mut engine = MockEngine::new();
    engine.initialize().await.unwrap();

    let tara = engine
        .synthesize(&SynthesisRequest::new("tara-mock", "Hello from Tara"))
        .await
        .unwrap();
    let kashi = engine
        .synthesize(&SynthesisRequest::new(
            "kashi-mock",
            "Namaste from Kashi, longer text",
        ))
        .await
        .unwrap();

    assert_ne!(tara.audio.len(), kashi.audio.len());
    assert!(kashi.metadata.duration_ms > tara.metadata.duration_ms);
}

#[tokio::test]
async fn unknown_voice_is_voice_not_found() {
    let mut engine = MockEngine::new();
    engine.initialize().await.unwrap();
    let err = engine
        .synthesize(&SynthesisRequest::new("nobody", "hi"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), VoiceErrorCode::VoiceNotFound);
}

/// The load-bearing honesty test: there is NO real backend.
///
/// If someone implements ONNX inference, this must fail, and the fix is to implement the
/// real-audio test it asks for and flip `vocal_core::REAL_SYNTHESIS_AVAILABLE` in the same
/// PR — not to `#[ignore]` it.
#[tokio::test]
async fn piper_engine_cannot_synthesize_and_says_so() {
    let mut engine = PiperEngine::new();
    engine.initialize().await.unwrap();

    assert!(
        !matches!(
            engine.health().await.unwrap(),
            vocal_core::engine::EngineHealth::Healthy
        ),
        "PiperEngine must not report Healthy while it cannot produce audio"
    );

    let err = engine
        .synthesize(&SynthesisRequest::new("tara", "Hello"))
        .await
        .expect_err("PiperEngine::synthesize must fail until ONNX inference exists");
    assert_eq!(err.code(), VoiceErrorCode::EngineNotAvailable);

    // Tied to the observed refusal rather than asserted on its own: `assert!(!CONST)`
    // can never fail for the right reason, and the whole value of the flag is that it
    // moves in lockstep with an engine that actually synthesizes.
    assert_eq!(
        vocal_core::REAL_SYNTHESIS_AVAILABLE,
        !matches!(err.code(), VoiceErrorCode::EngineNotAvailable),
        "REAL_SYNTHESIS_AVAILABLE and PiperEngine::synthesize disagree; they must change in the same commit"
    );
}

#[tokio::test]
async fn streaming_api_is_exposed_even_though_piper_cannot_stream() {
    let mut engine = MockEngine::new();
    engine.initialize().await.unwrap();
    let chunk = engine
        .stream(&SynthesisRequest::new("tara-mock", "stream me"))
        .await
        .unwrap()
        .await
        .unwrap();
    assert!(!chunk.is_empty());

    let piper = PiperEngine::new();
    assert!(piper
        .stream(&SynthesisRequest::new("tara", "stream me"))
        .await
        .is_err());
}

#[tokio::test]
async fn critical_evaluation_sentences_run_through_the_pipeline() {
    // NOTE: these sentences assert that the pipeline accepts them, NOT that they sound
    // right. Text normalization is a pass-through stub, so "Rs 12,50,000" would NOT be
    // expanded today — the PRD's Indian-English normalization requirements are unmet.
    let mut engine = MockEngine::new();
    engine.initialize().await.unwrap();

    let cases = [
        (
            "tara-mock",
            "Your appointment is confirmed for Thursday at three PM.",
        ),
        (
            "tara-mock",
            "We'll be with you shortly — thank you for your patience.",
        ),
        (
            "tara-mock",
            "The total amount due is twelve thousand five hundred rupees.",
        ),
        ("kashi-mock", "Your question is important."),
        ("kashi-mock", "Peace and patience are the true strength."),
    ];

    for (voice, text) in cases {
        let response = engine.synthesize(&SynthesisRequest::new(voice, text)).await;
        assert!(response.is_ok(), "failed for {voice}: {text}");
    }
}

#[test]
fn error_codes_are_defined_and_documented() {
    let codes = [
        VoiceErrorCode::VoiceNotFound,
        VoiceErrorCode::PackNotFound,
        VoiceErrorCode::PackInvalidFormat,
        VoiceErrorCode::PackSchemaMismatch,
        VoiceErrorCode::PackChecksumFailed,
        VoiceErrorCode::PackPathTraversal,
        VoiceErrorCode::PackSizeExceeded,
        VoiceErrorCode::PackExecutableContent,
        VoiceErrorCode::PackProvenanceIncomplete,
        VoiceErrorCode::EngineNotAvailable,
        VoiceErrorCode::EngineVersionMismatch,
        VoiceErrorCode::SynthesisFailed,
        VoiceErrorCode::SynthesisCancelled,
        VoiceErrorCode::NormalizationFailed,
        VoiceErrorCode::LocaleNotSupported,
        VoiceErrorCode::DaemonNotRunning,
        VoiceErrorCode::DaemonAuthFailed,
        VoiceErrorCode::AudioDeviceError,
    ];
    assert_eq!(codes.len(), 18, "PRD section 15 defines 18 codes");

    for code in codes {
        let s = code.as_str();
        assert!(!s.is_empty(), "{code:?} has an empty stable string");
        assert!(!code.user_message().is_empty(), "{s} has no user message");
        assert!(
            s.chars().all(|c| c.is_ascii_uppercase() || c == '_'),
            "error code {s} must be SCREAMING_SNAKE_CASE so it can be matched on by clients"
        );
    }
}

#[test]
fn pack_loader_errors_map_to_stable_codes() {
    let err: vocal_core::VoiceError =
        voice_pack::LoadError::LimitExceeded("too big".to_string()).into();
    assert_eq!(err.code(), VoiceErrorCode::PackSizeExceeded);

    let err: vocal_core::VoiceError =
        voice_pack::LoadError::ValidationFailed("Checksum mismatch for model.onnx".to_string())
            .into();
    assert_eq!(err.code(), VoiceErrorCode::PackChecksumFailed);
}

// ──────────────────────────────────────────────────────────────────────────────
// Enforceable hygiene gates (VOICE_INV_001 / INV_002 / INV_003)
// ──────────────────────────────────────────────────────────────────────────────

fn read(path: &str) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {path}: {e} (tests must run from the workspace)"))
}

/// `vocal-core` must never depend on an HTTP/TLS client or a cloud/LLM SDK.
///
/// This is the real, CI-cheap version of the "zero cloud dependencies" claim: it is
/// machine-checkable and fails the build, unlike the previous `test_no_network_access`
/// placeholder that only re-ran a silent mock.
#[test]
fn no_network_or_llm_clients_in_vocal_core() {
    let manifest_path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    // Ignore comments: this crate's manifest deliberately *mentions* forbidden crates
    // while explaining why they are absent. Only declarations count.
    let manifest: String = read(manifest_path)
        .lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");

    const FORBIDDEN: &[&str] = &[
        "reqwest",
        "hyper",
        "ureq",
        "awc",
        "surf",
        "isahc",
        "curl",
        "openssl",
        "rustls",
        "native-tls",
        "openai",
        "anthropic",
        "elevenlabs",
        "eleven-labs",
        "azure",
        "aws-sdk",
        "google-cloud",
        "polly",
        "huggingface",
        "hf-hub",
        "mockito",
        "wiremock",
        "httpc",
        "tungstenite",
    ];

    for needle in FORBIDDEN {
        assert!(
            !manifest.contains(needle),
            "vocal-core/Cargo.toml must not depend on {needle:?} — the core runtime has no \
             network path by design (VOICE_INV_001/002/003)"
        );
    }

    // Source-level backstop: no HTTP client use in src/, even transitively via a renamed dep.
    for entry in walk(concat!(env!("CARGO_MANIFEST_DIR"), "/src")) {
        let text = read(&entry);
        for needle in [
            "reqwest::",
            "hyper::",
            "TcpStream::connect",
            "UnixStream::connect",
        ] {
            assert!(
                !text.contains(needle),
                "{entry} uses {needle:?}; the synthesis core must not open sockets"
            );
        }
    }
}

/// ONNX model fetching must stay off: `ort`'s `fetch-models` feature would let the
/// runtime pull weights over the network, defeating the offline invariant.
#[test]
fn ort_fetch_models_feature_is_disabled() {
    let root: String = read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"))
        .lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let line = root
        .lines()
        .find(|l| l.trim_start().starts_with("ort "))
        .expect("workspace root must declare `ort`");
    assert!(
        !line.contains("fetch-models"),
        "`ort` must not enable fetch-models, found: {line}"
    );
}

#[test]
fn pipeline_and_normalizer_are_honest_about_being_stubs() {
    // Locks in current behaviour so a future implementation is a deliberate change
    // rather than a silent one. If normalization starts transforming text, update the
    // PRD claims and this test together.
    let normalized = vocal_core::text_normalization::TextNormalizer::new()
        .normalize("Pay Rs 1,25,000 by 14/08/1947")
        .unwrap();
    assert_eq!(
        normalized, "Pay Rs 1,25,000 by 14/08/1947",
        "text normalization is documented as an unimplemented pass-through"
    );

    let processed =
        futures_block_on(vocal_core::pipeline::SynthesisPipeline::new().process("hi", None));
    assert_eq!(processed.unwrap(), "hi");
}

fn futures_block_on<F: std::future::Future>(fut: F) -> F::Output {
    let mut fut = std::pin::pin!(fut);
    // Waker::noop() is const-stable since 1.85; the hand-rolled Arc<dyn Wake> it
    // replaces was written when the MSRV predates it.
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

fn walk(dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![std::path::PathBuf::from(dir)];
    while let Some(path) = stack.pop() {
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                for e in entries.filter_map(|e| e.ok()) {
                    stack.push(e.path());
                }
            }
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path.to_string_lossy().to_string());
        }
    }
    out
}

#[test]
fn ci_probe_persona_rules() {
    // (label, persona json, files json, placeholder?, expected substring of the error / "" == expect Ok)
    let nine: Vec<String> = (0..9)
        .map(|i| {
            format!(
                r#"{{"voice":"v{i}","weight":{w:.5}}}"#,
                w = 1.0_f32 / 9.0
            )
        })
        .collect();
    let bloated_style = format!(r#""style":{{"blend":[{}]}}"#, nine.join(","));

    let cases: Vec<(&str, String, &str, bool, &str)> = vec![
        ("01 placeholder, no style", without_style(r#""pitch_baked_into_style":false"#), FILES_ONE, true, ""),
        ("02 release, no style", without_style(""), FILES_ONE, false, "persona.style"),
        ("03 source_voice", with_style(""), FILES_ONE, false, ""),
        (
            "04 tara blend",
            with_style(r#""style":{"blend":[{"voice":"af_bella","weight":0.40},{"voice":"af_heart","weight":0.35},{"voice":"af_aoede","weight":0.25}]}"#),
            FILES_ONE,
            false,
            "",
        ),
        (
            "05 blend sum drift",
            with_style(r#""style":{"blend":[{"voice":"af_bella","weight":0.40},{"voice":"af_heart","weight":0.40}]}"#),
            FILES_ONE,
            false,
            "sum to",
        ),
        (
            "06 blend duplicate",
            with_style(r#""style":{"blend":[{"voice":"af_heart","weight":0.5},{"voice":"af_heart","weight":0.5}]}"#),
            FILES_ONE,
            false,
            "twice",
        ),
        (
            "07 blend weight range",
            with_style(r#""style":{"blend":[{"voice":"af_heart","weight":1.5},{"voice":"af_bella","weight":-0.5}]}"#),
            FILES_ONE,
            false,
            "0.0..=1.0",
        ),
        ("08 blend too many terms", with_style(&bloated_style), FILES_ONE, false, "terms"),
        (
            "09 two style sources",
            with_style(r#""style":{"source_voice":"af_heart","embedded_file":"persona.bin"}"#),
            FILES_TWO,
            false,
            "exactly one",
        ),
        ("10 embedded ok", with_style(r#""embedded_file":"persona.bin""#), FILES_TWO, false, ""),
        ("11 embedded absent", with_style(r#""embedded_file":"absent.bin""#), FILES_TWO, false, "not declared"),
        ("12 embedded wrong type", with_style(r#""embedded_file":"model.onnx""#), FILES_WRONGTYPE, false, "not style_vector"),
        ("13 embedded truncated", with_style(r#""embedded_file":"persona.bin""#), FILES_TRUNC, false, "522240"),
        ("14 pitch, no owner", with_style(r#""default_pitch":-0.10"#), FILES_ONE, false, "no pitch input"),
        ("15 pitch baked", with_style(r#""default_pitch":-0.10,"pitch_baked_into_style":true"#), FILES_ONE, false, ""),
        ("16 pitch multiplier 1.0", with_style(r#""default_pitch":1.0"#), FILES_ONE, false, "unit mix-up"),
        (
            "17 intent pitch",
            with_style(r#""intent_profiles":{"GREETING":{"rate":1.05,"pitch":1.05,"energy":0.65,"pause_factor":1.0}}"#),
            FILES_ONE,
            false,
            "per-intent pitch",
        ),
        (
            "18 intent energy",
            with_style(r#""intent_profiles":{"GREETING":{"rate":1.05,"pitch":0.0,"energy":1.1,"pause_factor":1.0}}"#),
            FILES_ONE,
            false,
            "0.0..=1.0",
        ),
        (
            "19 intent rate",
            with_style(r#""intent_profiles":{"GREETING":{"rate":2.0,"pitch":0.0,"energy":0.65,"pause_factor":1.0}}"#),
            FILES_ONE,
            false,
            "0.5..=1.6",
        ),
        (
            "20 intent pause",
            with_style(r#""intent_profiles":{"GREETING":{"rate":1.05,"pitch":0.0,"energy":0.65,"pause_factor":9.0}}"#),
            FILES_ONE,
            false,
            "pause_factor",
        ),
        (
            "21 tara intent table",
            with_style(r#""intent_profiles":{"GREETING":{"rate":1.05,"pitch":0.0,"energy":0.65,"pause_factor":1.0},"WARNING":{"rate":0.90,"pitch":0.0,"energy":0.45,"pause_factor":1.15},"CELEBRATION":{"rate":1.10,"pitch":0.0,"energy":0.75,"pause_factor":1.0}}"#),
            FILES_ONE,
            false,
            "",
        ),
        ("22 loud 0 dBFS", with_style(r#""loudness":{"target_dbfs":0.0,"peak_ceiling":0.98,"max_gain_db":12.0}"#), FILES_ONE, false, "full-scale distortion"),
        ("23 loud -60 dBFS", with_style(r#""loudness":{"target_dbfs":-60.0,"peak_ceiling":0.98,"max_gain_db":12.0}"#), FILES_ONE, false, "target_dbfs"),
        ("24 ceiling 1.0", with_style(r#""loudness":{"target_dbfs":-20.0,"peak_ceiling":1.0,"max_gain_db":12.0}"#), FILES_ONE, false, "clipping"),
        ("25 gain 60 dB", with_style(r#""loudness":{"target_dbfs":-20.0,"peak_ceiling":0.98,"max_gain_db":60.0}"#), FILES_ONE, false, "max_gain_db"),
        ("26 tara loudness", with_style(r#""loudness":{"target_dbfs":-21.0,"peak_ceiling":0.98,"max_gain_db":12.0}"#), FILES_ONE, false, ""),
        ("27 override ok", with_style(r#""pronunciation_overrides":{"chiti":"X"}"#), FILES_ONE, false, ""),
        ("28 override phrase", with_style(r#""pronunciation_overrides":{"new delhi":"nju delhi"}"#), FILES_ONE, false, "single non-empty word"),
        ("29 override blank", with_style(r#""pronunciation_overrides":{"chiti":"   "}"#), FILES_ONE, false, "malformed"),
    ];

    let mut fails = 0usize;
    for (i, (label, persona, files, placeholder, expect)) in cases.into_iter().enumerate() {
        let json = manifest(&persona, files, placeholder);
        let parsed: Result<voice_pack::PackManifest, _> = serde_json::from_str(&json);
        let outcome = match parsed {
            Err(e) => {
                fails += 1;
                format!("::error::PROBE {i:02} {label} PARSE-FAIL {e}")
            }
            Ok(m) => {
                let got = m.validate_persona();
                match (expect.is_empty(), &got) {
                    (true, Ok(())) => format!("::notice::PROBE {i:02} {label} ok"),
                    (true, Err(e)) => {
                        fails += 1;
                        format!("::error::PROBE {i:02} {label} UNEXPECTED-REJECT {e}")
                    }
                    (false, Ok(())) => {
                        fails += 1;
                        format!("::error::PROBE {i:02} {label} SHOULD-HAVE-FAILED (wanted {expect:?})")
                    }
                    (false, Err(e)) if e.contains(expect) => {
                        format!("::notice::PROBE {i:02} {label} rejected-as-intended")
                    }
                    (false, Err(e)) => {
                        fails += 1;
                        format!("::error::PROBE {i:02} {label} WRONG-MESSAGE want={expect:?} got={e}")
                    }
                }
            }
        };
        println!("{outcome}");
    }

    // The validator gate itself, plus the runtime's prosody maths — the other two places a red
    // `Unit Tests` could be coming from, with the same fixture values the real tests use.
    let m: voice_pack::PackManifest =
        serde_json::from_str(&manifest(&with_style(r#""default_pitch":0.30"#), FILES_ONE, false)).unwrap();
    let v = voice_pack::PackValidator::with_limits(voice_pack::PackLimits::embedded());
    let gate = v.validate_manifest(&m);
    println!(
        "::error::PROBE 30 validate_manifest prefix = {:?}",
        gate.err().map(|e| e.chars().take(9).collect::<String>())
    );

    let cfg: voice_pack::PersonaConfig =
        serde_json::from_str(&with_style(r#""loudness":{"target_dbfs":-21.0,"peak_ceiling":0.98,"max_gain_db":12.0},"intent_profiles":{"GREETING":{"rate":1.05,"pitch":0.0,"energy":0.65,"pause_factor":1.0}}"#)).unwrap();
    let mut rt = vocal_core::PersonaRuntime::new();
    rt.register_persona(vocal_core::Persona::from_pack(&cfg));
    let base = rt.prosody("tara", None);
    let greet = rt.prosody("tara", Some("GREETING"));
    println!(
        "::error::PROBE 31 prosody no-intent={:?} greeting={:?} expected_no=-21.0 expected_greet={}",
        base.as_ref().map(|p| p.loudness_target_dbfs),
        greet.as_ref().map(|p| p.loudness_target_dbfs),
        -21.0_f32 + (0.65_f32 - 0.5) * 12.0
    );
    let mut table: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    table.insert("chiti".to_string(), "x".to_string());
    let segs = vocal_core::text_normalization::split_for_overrides("Call Chiti now, and chitizone", &table);
    println!(
        "::error::PROBE 32 segments={} overrides={} text={:?}",
        segs.len(),
        segs.iter().filter(|s| s.is_override()).count(),
        segs.iter()
            .map(|s| match s {
                vocal_core::text_normalization::Segment::Text(t) => format!("[{t}]"),
                vocal_core::text_normalization::Segment::Phonemes(p) => format!("<{p}>"),
            })
            .collect::<Vec<_>>()
    );
    // Deliberately no assertion here: a periscope must not become a second gate, or its own red job
    // hides the green one it was reading. `DONE fixture-failures=N` is the number to look at.
    println!("::error::PROBE DONE fixture-failures={fails}");
}
