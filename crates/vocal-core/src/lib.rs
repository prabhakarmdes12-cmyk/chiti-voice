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

pub use engine::{VoiceEngine, VoiceEngineRegistry};
pub use error::{VoiceError, VoiceErrorCode, VoiceResult};
pub use persona::{Persona, PersonaRuntime};
pub use synthesis::{SynthesisRequest, SynthesisResponse, SynthesisFormat};

/// The semantic version of Chiti Vocal Core
pub const VOCAL_CORE_VERSION: &str = "0.1.0-alpha";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(VOCAL_CORE_VERSION, "0.1.0-alpha");
    }
}
