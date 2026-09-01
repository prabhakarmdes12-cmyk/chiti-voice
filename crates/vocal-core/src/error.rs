//! Error types for Chiti Vocal Runtime
//!
//! All errors are structured typed values with stable machine-readable codes.

use thiserror::Error;

pub type VoiceResult<T> = Result<T, VoiceError>;

/// Error codes matching the specification in PRD Section 15
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceErrorCode {
    VoiceNotFound,
    PackNotFound,
    PackInvalidFormat,
    PackSchemaMismatch,
    PackChecksumFailed,
    PackPathTraversal,
    PackSizeExceeded,
    PackExecutableContent,
    PackProvenanceIncomplete,
    EngineNotAvailable,
    EngineVersionMismatch,
    SynthesisFailed,
    SynthesisCancelled,
    NormalizationFailed,
    LocaleNotSupported,
    DaemonNotRunning,
    DaemonAuthFailed,
    AudioDeviceError,
}

impl VoiceErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VoiceNotFound => "VOICE_NOT_FOUND",
            Self::PackNotFound => "PACK_NOT_FOUND",
            Self::PackInvalidFormat => "PACK_INVALID_FORMAT",
            Self::PackSchemaMismatch => "PACK_SCHEMA_MISMATCH",
            Self::PackChecksumFailed => "PACK_CHECKSUM_FAILED",
            Self::PackPathTraversal => "PACK_PATH_TRAVERSAL",
            Self::PackSizeExceeded => "PACK_SIZE_EXCEEDED",
            Self::PackExecutableContent => "PACK_EXECUTABLE_CONTENT",
            Self::PackProvenanceIncomplete => "PACK_PROVENANCE_INCOMPLETE",
            Self::EngineNotAvailable => "ENGINE_NOT_AVAILABLE",
            Self::EngineVersionMismatch => "ENGINE_VERSION_MISMATCH",
            Self::SynthesisFailed => "SYNTHESIS_FAILED",
            Self::SynthesisCancelled => "SYNTHESIS_CANCELLED",
            Self::NormalizationFailed => "NORMALIZATION_FAILED",
            Self::LocaleNotSupported => "LOCALE_NOT_SUPPORTED",
            Self::DaemonNotRunning => "DAEMON_NOT_RUNNING",
            Self::DaemonAuthFailed => "DAEMON_AUTH_FAILED",
            Self::AudioDeviceError => "AUDIO_DEVICE_ERROR",
        }
    }

    pub fn user_message(&self) -> &'static str {
        match self {
            Self::VoiceNotFound => "Voice is not available. Install the voice pack and try again.",
            Self::PackNotFound => "Voice pack file not found.",
            Self::PackInvalidFormat => "Voice pack format is invalid.",
            Self::PackSchemaMismatch => "Voice pack schema version is not supported. Update Chiti Vocal Runtime.",
            Self::PackChecksumFailed => "Voice pack integrity check failed. The file may be corrupted.",
            Self::PackPathTraversal => "Voice pack was rejected for security reasons.",
            Self::PackSizeExceeded => "Voice pack exceeds maximum allowed size.",
            Self::PackExecutableContent => "Voice pack was rejected for security reasons.",
            Self::PackProvenanceIncomplete => "Voice pack is missing provenance information.",
            Self::EngineNotAvailable => "The voice engine required by this voice pack is not available.",
            Self::EngineVersionMismatch => "Voice engine version is too old. Update the engine.",
            Self::SynthesisFailed => "Voice synthesis failed. Check logs for details.",
            Self::SynthesisCancelled => "Voice synthesis was stopped.",
            Self::NormalizationFailed => "Could not process the input text.",
            Self::LocaleNotSupported => "This voice does not support the requested language.",
            Self::DaemonNotRunning => "Chiti Vocal Runtime is not running. Start the Vocal Local Service.",
            Self::DaemonAuthFailed => "Request origin is not permitted.",
            Self::AudioDeviceError => "Could not access audio output device.",
        }
    }
}

/// Structured error type for all Chiti Vocal Runtime operations
#[derive(Debug, Error)]
#[error("{message}")]
pub struct VoiceError {
    pub code: VoiceErrorCode,
    pub message: String,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl VoiceError {
    pub fn new(code: VoiceErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(
        code: VoiceErrorCode,
        message: impl Into<String>,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            source: Some(source),
        }
    }

    pub fn code(&self) -> VoiceErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_strings() {
        assert_eq!(VoiceErrorCode::VoiceNotFound.as_str(), "VOICE_NOT_FOUND");
        assert_eq!(VoiceErrorCode::PackChecksumFailed.as_str(), "PACK_CHECKSUM_FAILED");
    }

    #[test]
    fn test_error_creation() {
        let err = VoiceError::new(VoiceErrorCode::VoiceNotFound, "Tara not installed");
        assert_eq!(err.code, VoiceErrorCode::VoiceNotFound);
        assert_eq!(err.message, "Tara not installed");
    }
}
