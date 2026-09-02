//! Chiti Vocal Runtime - Core TTS Engine
//!
//! This crate provides the abstract voice engine interface and synthesis pipeline
//! for the Chiti Vocal Runtime platform. It handles:
//! - Voice engine abstraction (swappable backends)
//! - Text normalization pipeline
//! - Persona-based prosody application
//! - Voice pack loading and validation
//! - PCM audio output

pub mod engine;
pub mod error;
pub mod persona;
pub mod pipeline;
pub mod state;
pub mod synthesis;
pub mod text_normalization;
pub mod wav;

pub use engine::{VoiceEngine, VoiceEngineRegistry};
pub use error::{VoiceError, VoiceErrorCode, VoiceResult};
pub use persona::{Persona, PersonaRuntime};
pub use synthesis::{SynthesisRequest, SynthesisResponse, SynthesisFormat};

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
        // If a future PR implements ONNX inference, this test failing is the
        // intended signal to flip REAL_SYNTHESIS_AVAILABLE and re-verify the docs.
        assert!(!REAL_SYNTHESIS_AVAILABLE);

        // The corollary must hold too: the non-mock engine must refuse to synthesize,
        // and must not report Healthy while it cannot.
        use crate::engine::piper::PiperEngine;
        use crate::engine::VoiceEngine;

        let engine = PiperEngine::new();
        assert!(
            !matches!(engine.health().await.unwrap(), crate::engine::EngineHealth::Healthy),
            "PiperEngine reported Healthy while synthesis is unimplemented"
        );
    }
}
