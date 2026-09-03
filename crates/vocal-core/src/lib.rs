//! Chiti Vocal Runtime - Core TTS Engine
//!
//! This crate provides the abstract voice engine interface and synthesis pipeline
//! for the Chiti Vocal Runtime platform. It handles:
//! - Voice engine abstraction (swappable backends)
//! - Text normalization pipeline
//! - Persona-based prosody application
//! - Voice pack loading and validation
//! - PCM audio output

pub mod audio_levels;
pub mod engine;
pub mod error;
pub mod persona;
pub mod phoneme_tokens;
pub mod pipeline;
pub mod style_matrix;
pub mod state;
pub mod synthesis;
pub mod text_normalization;
pub mod utterance_plan;
pub mod wav;

pub use audio_levels::{LoudnessApplied, LoudnessSpec};
pub use engine::{VoiceEngine, VoiceEngineRegistry};
pub use error::{VoiceError, VoiceErrorCode, VoiceResult};
pub use persona::{Persona, PersonaRuntime};
pub use synthesis::{SynthesisRequest, SynthesisResponse, SynthesisFormat};
pub use utterance_plan::{Piece, Plan, PlanPolicy, Utterance, plan_pieces};

/// The semantic version of Chiti Vocal Core
pub const VOCAL_CORE_VERSION: &str = "0.1.0-alpha";

/// **Truth gate.** Does any engine in this build produce real speech?
///
/// `false` until an ONNX inference path exists. `PiperEngine` is a voice *registry*
/// plus a `TODO`, and `MockEngine` emits digital silence, so no code path in this
/// crate can currently produce audible audio.
///
/// This constant exists so that documentation, CI, and applications can assert the
/// capability instead of trusting prose. `README.md` and `PHASE1_COMPLETE.md` both
/// claimed audible output while this was false; the `docs-truth` CI job now fails if
/// they claim voice capability while this constant says `false`. Flip it to `true`
/// in the same PR that lands working ONNX inference, with a test that decodes a
/// real `.cvpack` model and asserts non-zero PCM.
pub const REAL_SYNTHESIS_AVAILABLE: bool = false;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(VOCAL_CORE_VERSION, "0.1.0-alpha");
    }

    #[tokio::test]
    async fn truth_gate_reports_no_real_synthesis() {
        use crate::engine::piper::PiperEngine;
        use crate::engine::VoiceEngine;

        // The flag must agree with observable behaviour, so this is not a tautology: a PR
        // that implements inference without flipping REAL_SYNTHESIS_AVAILABLE fails here,
        // and so does one that flips the flag without implementing anything. Pinning both
        // directions is the entire reason the constant exists.
        let mut engine = PiperEngine::new();
        engine.initialize().await.unwrap();
        let refuses = engine
            .synthesize(&crate::synthesis::SynthesisRequest::new(
                "tara",
                "one sentence",
            ))
            .await
            .is_err();
        assert_eq!(
            REAL_SYNTHESIS_AVAILABLE,
            !refuses,
            "REAL_SYNTHESIS_AVAILABLE={} while PiperEngine::synthesize {} — flag and engine must change in the same commit",
            REAL_SYNTHESIS_AVAILABLE,
            if refuses { "refuses to speak" } else { "produces audio" }
        );

        // A capability lie is worse than a missing feature: an engine that cannot produce
        // audio must not report Healthy, or applications will build against it.
        assert!(
            !matches!(
                engine.health().await.unwrap(),
                crate::engine::EngineHealth::Healthy
            ),
            "PiperEngine reported Healthy while synthesis is unimplemented"
        );
    }
}
