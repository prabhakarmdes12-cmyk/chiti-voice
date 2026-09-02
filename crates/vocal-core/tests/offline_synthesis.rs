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
        .synthesize(&SynthesisRequest::new("kashi-mock", "Namaste from Kashi, longer text"))
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
        ("tara-mock", "Your appointment is confirmed for Thursday at three PM."),
        ("tara-mock", "We'll be with you shortly — thank you for your patience."),
        ("tara-mock", "The total amount due is twelve thousand five hundred rupees."),
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

    let err: vocal_core::VoiceError = voice_pack::LoadError::ValidationFailed(
        "Checksum mismatch for model.onnx".to_string(),
    )
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
        "reqwest", "hyper", "ureq", "awc", "surf", "isahc", "curl", "openssl", "rustls",
        "native-tls", "openai", "anthropic", "elevenlabs", "eleven-labs", "azure", "aws-sdk",
        "google-cloud", "polly", "huggingface", "hf-hub", "mockito", "wiremock", "httpc",
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
        for needle in ["reqwest::", "hyper::", "TcpStream::connect", "UnixStream::connect"] {
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

    let processed = futures_block_on(vocal_core::pipeline::SynthesisPipeline::new().process("hi", None));
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
