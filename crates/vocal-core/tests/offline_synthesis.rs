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

// ── TEMPORARY periscope (bodies lifted verbatim from `src/utterance_plan.rs`'s test module) ────────
// `Unit Tests` is red with no rustc diagnostic to read: an assertion failure produces none, and job
// logs are unreachable from this sandbox. This is the one test target CI runs with `--nocapture`, so a
// print here is the only way to see an assertion's values -- and `assert_eq!` already puts both sides in
// its panic payload, which `catch_unwind` turns into a line here instead of a red tick.
//
// The bodies are not restated: they are the unit tests' own text, so the harness cannot disagree with
// the thing it is diagnosing. If a scenario panics for a reason other than an assertion (an `unwrap` on
// a fixture that no longer parses), the payload says so too.
//
// Deleted as soon as `Unit Tests` is green: printing from a gate job is a diagnostic, not a test.
#[allow(clippy::all)]
#[test]
fn ci_probe_utterance_plan() {
    use vocal_core::phoneme_tokens::{encode, strip_to_vocab, MAX_PHONEME_UNITS, MAX_TOKENS};
    use vocal_core::utterance_plan::{plan_pieces, Piece, PlanPolicy, Utterance, DEFAULT_MAX_UNITS};
    use vocal_core::VoiceErrorCode;

    fn words(text: &str) -> Vec<Piece> {
        text.split_whitespace().map(Piece::phonemes).collect()
    }

    macro_rules! scenario {
        ($label:expr, $body:block) => {{
            let sink = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
            let hook_sink = std::sync::Arc::clone(&sink);
            std::panic::set_hook(Box::new(move |info| {
                if let Ok(mut slot) = hook_sink.lock() {
                    *slot = info.to_string();
                }
            }));
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body));
            let _ = std::panic::take_hook();
            match outcome {
                Ok(()) => println!("::notice::PROBE {} ok", $label),
                Err(_) => {
                    let payload = sink
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .clone();
                    let flat = payload.replace('\n', " ").replace('\r', " ");
                    let flat: String = flat.chars().filter(|c| !c.is_control()).collect();
                    println!("::error::PROBE {} :: {}", $label, flat);
                }
            }
        }};
    }

    scenario!("nothing_in_is_nothing_out", {
        let plan = plan_pieces(&[], &PlanPolicy::default()).unwrap();
        assert!(plan.is_empty());
        assert!(plan.utterances.is_empty(), "the plan and its Vec agree, which is all `len` could say");

        // A run the vocabulary filter erases entirely bills nothing, so it must not become a chunk of
        // its own. U+2603 is certainly absent from a 178-entry IPA table; a space is not, which is why
        // this is not written as " " -- that one really does cost a token.
        let dropped = Piece::phonemes("\u{2603}".to_string());
        assert_eq!(dropped.units(), 0);
        assert!(plan_pieces(&[dropped], &PlanPolicy::default()).unwrap().is_empty());
    });

    scenario!("units_are_billed_after_the_vocabulary_filter", {
        // U+026B (the l-with-tilde some Indic G2P output produces) is not in the 178-entry table, which
        // is why it must not be counted: a budget spent on a character `encode` drops is a budget spent
        // on nothing. Written as an escape, so no transport encoding can mangle it.
        let mut with_dropped = String::from("kala");
        with_dropped.push('\u{026B}');
        let piece = Piece::phonemes(with_dropped.clone());
        assert_eq!(piece.phonemes.chars().count(), 5, "the caller's string is longer");
        assert_eq!(piece.units(), 4, "the graph's is not");

        let plan = plan_pieces(&[piece], &PlanPolicy::default()).unwrap();
        assert_eq!(plan.utterances[0].phonemes, strip_to_vocab(&with_dropped));
        assert_eq!(plan.utterances[0].units, 4);
        assert_eq!(plan.style_rows(), vec![4], "and the row follows the filtered count");
    });

    scenario!("truncation_cannot_fire_on_a_planned_utterance", {
        // Far more input than the window holds. Every planned utterance must fit it exactly, which is
        // the difference between a long paragraph and a long paragraph minus its last clause.
        let sentence = "slovo na proveri, to je dovoljno dugo.";
        let pieces: Vec<Piece> = (0..200).flat_map(|_| words(sentence)).collect();
        let joined: String = pieces
            .iter()
            .map(|p| p.phonemes.clone())
            .collect::<Vec<String>>()
            .join(" ");
        assert!(
            joined.chars().count() > MAX_TOKENS,
            "this test is pointless unless the input overflows the window"
        );

        let plan = plan_pieces(&pieces, &PlanPolicy::default()).unwrap();
        assert!(plan.len() > 1, "one utterance cannot hold it: {:?}", plan.len());
        for utterance in &plan.utterances {
            let encoded = encode(&utterance.phonemes);
            assert_eq!(encoded.len(), utterance.units + 2, "nothing was truncated away");
            assert!(utterance.units <= DEFAULT_MAX_UNITS);
            assert_eq!(utterance.style_row, utterance.units, "no row was selected by the clamp");
        }
    });

    scenario!("planning_loses_nothing_and_invents_nothing", {
        let pieces = words("ha ha ha. ti si tu, a ja? ne.");
        let plan = plan_pieces(&pieces, &PlanPolicy { max_units: 9, min_chunk_units: 2 }).unwrap();
        let planned = plan
            .utterances
            .iter()
            .map(|u| u.phonemes.as_str())
            .collect::<Vec<&str>>()
            .join(" ");
        let expected: String = pieces
            .iter()
            .map(|p| strip_to_vocab(&p.phonemes))
            .collect::<Vec<String>>()
            .join(" ");
        assert_eq!(planned, expected, "the chunks must rejoin into the input, spaces included");
        let pieces_total: usize = plan.utterances.iter().map(|u| u.pieces).sum();
        assert_eq!(pieces_total, pieces.len(), "no run may be dropped or split");
    });

    scenario!("an_override_survives_a_boundary_whole", {
        // Sized so the ceiling falls inside the filler, right where the override sits: the rule that
        // matters is that the override is never what gets cut.
        let mut pieces: Vec<Piece> = Vec::new();
        for _ in 0..6 {
            pieces.push(Piece::phonemes("aaaa".to_string()));
        }
        pieces.push(Piece::from_override("t\u{0283}\u{0261}\u{026A}ti".to_string()));
        for _ in 0..6 {
            pieces.push(Piece::phonemes("bbbb".to_string()));
        }

        let plan = plan_pieces(&pieces, &PlanPolicy { max_units: 12, min_chunk_units: 1 }).unwrap();
        let with_override: Vec<&Utterance> =
            plan.utterances.iter().filter(|u| u.has_override).collect();
        assert_eq!(with_override.len(), 1, "the override belongs to exactly one chunk");
        let spelled = "\u{0283}\u{0261}\u{026A}";
        assert!(
            with_override[0].phonemes.contains(spelled),
            "and it must be there complete, not split across two rows: {:?}",
            with_override[0].phonemes
        );
    });

    scenario!("an_unsplittable_run_is_refused_with_its_numbers", {
        let too_big = Piece::phonemes("a".repeat(DEFAULT_MAX_UNITS + 1));
        let err = plan_pieces(&[too_big], &PlanPolicy::default()).unwrap_err();
        assert_eq!(err.code(), VoiceErrorCode::NormalizationFailed);
        let text = err.to_string();
        assert!(text.contains("no chunk boundary"), "{text}");
        assert!(
            text.contains(DEFAULT_MAX_UNITS.to_string().as_str()),
            "the message must quote the ceiling it exceeded: {text}"
        );
    });

    scenario!("a_policy_that_cannot_be_honoured_fails_before_any_input_is_walked", {
        for bad in [PlanPolicy { max_units: 0, min_chunk_units: 1 }, PlanPolicy {
            max_units: MAX_PHONEME_UNITS + 1,
            min_chunk_units: 1,
        }] {
            let err = plan_pieces(&words("ha."), &bad).unwrap_err();
            assert_eq!(err.code(), VoiceErrorCode::NormalizationFailed);
            assert!(err.to_string().contains("max_units"), "{err}");
        }
        let inverted = PlanPolicy { max_units: 8, min_chunk_units: 9 };
        assert!(plan_pieces(&words("ha."), &inverted)
            .unwrap_err()
            .to_string()
            .contains("min_chunk_units"));
    });

    scenario!("sentence_punctuation_closes_a_chunk_once_the_minimum_is_met", {
        let policy = PlanPolicy { max_units: 12, min_chunk_units: 4 };
        let plan = plan_pieces(&words("ha. ha. haha."), &policy).unwrap();
        assert_eq!(plan.len(), 2, "two sentences fill a chunk, the third starts the next");
        assert_eq!(plan.style_rows(), vec![7, 5], "rows are lengths, so they must add up by hand");
        assert_eq!(plan.policy, policy, "the policy travels with the plan it produced");

        // Below the minimum, punctuation does not fire: 13 units is a whole chunk under `max_units`,
        // and the sentences stay together. That is the rule that keeps a lone interjection from
        // arriving as its own prosody -- and the reason a short tail needs the fold below.
        let patient = PlanPolicy { max_units: 20, min_chunk_units: 9 };
        let together = plan_pieces(&words("ha. ha. haha."), &patient).unwrap();
        assert_eq!(together.len(), 1, "nothing is long enough to break for");
        assert_eq!(together.style_rows(), vec![13]);
    });

    scenario!("the_same_sentence_under_two_policies_is_two_different_sounds", {
        let pieces = words("ovo je jedna duga recenica koja hoce da se slusa.");
        let one = plan_pieces(&pieces, &PlanPolicy::default()).unwrap();
        let many = plan_pieces(
            &pieces,
            &PlanPolicy { max_units: 12, min_chunk_units: 4 },
        )
        .unwrap();

        assert_eq!(one.len(), 1, "the default ceiling holds a sentence of this size");
        assert!(many.len() > 1, "a tighter one splits it");
        assert_ne!(one.style_rows(), many.style_rows());
        assert_eq!(
            one.style_rows()[0],
            pieces.iter().map(|p| p.phonemes.chars().count()).sum::<usize>()
                + pieces.len() - 1,
            "one chunk reads the row for the whole sentence -- the row is the length, nothing else"
        );
    });

    scenario!("a_short_tail_folds_back_only_when_it_fits", {
        // "aaaa bbbb." closes on its period, leaving "cc." as a 3-unit tail. Under a 12-token ceiling
        // folding it back would need 14, so it stays alone -- and reads row 3, a different voice
        // cadence for three phonemes. This is what the fold exists to prevent.
        let stuck = plan_pieces(
            &words("aaaa bbbb. cc."),
            &PlanPolicy { max_units: 12, min_chunk_units: 5 },
        )
        .unwrap();
        assert_eq!(stuck.len(), 2, "{:?}", stuck.style_rows());
        assert_eq!(stuck.style_rows(), vec![10, 3], "the tail keeps a row of its own");

        // Same input, room to fold: one utterance, one row, no orphan prosody.
        let folded = plan_pieces(
            &words("aaaa bbbb. cc."),
            &PlanPolicy { max_units: 20, min_chunk_units: 5 },
        )
        .unwrap();
        assert_eq!(folded.len(), 1, "{:?}", folded.style_rows());
        assert_eq!(folded.style_rows(), vec![14], "10 + a space + 3");
    });
}
