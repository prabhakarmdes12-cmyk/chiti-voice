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

impl From<voice_pack::loader::LoadError> for VoiceError {
    /// Maps pack-loader failures onto stable machine-readable `VOICE_PACK_*` codes.
    ///
    /// Before this, `VoiceErrorCode::{PackNotFound, PackInvalidFormat, PackSizeExceeded,
    /// PackPathTraversal, PackExecutableContent, PackProvenanceIncomplete}` were defined
    /// and documented but unreachable from the loader — an API that promised codes it
    /// never returned.
    ///
    /// TODO(voice-pack): classify from typed errors instead of message keywords. The
    /// validator currently returns `Result<(), String>`, which forces this stringly
    /// match; converting `security.rs` to a typed `ValidationError` is a follow-up.
    fn from(err: voice_pack::loader::LoadError) -> Self {
        use voice_pack::loader::LoadError;
        let message = err.to_string();
        let code = match &err {
            LoadError::FileNotFound(_) | LoadError::IoError(_) => VoiceErrorCode::PackNotFound,
            LoadError::MissingManifest => VoiceErrorCode::PackInvalidFormat,
            LoadError::InvalidZip(_) => VoiceErrorCode::PackInvalidFormat,
            LoadError::InvalidManifest(m) => {
                if m.to_lowercase().contains("schema") {
                    VoiceErrorCode::PackSchemaMismatch
                } else {
                    VoiceErrorCode::PackInvalidFormat
                }
            }
            LoadError::LimitExceeded(_) => VoiceErrorCode::PackSizeExceeded,
            LoadError::ValidationFailed(m) => {
                let m = m.to_lowercase();
                if m.contains("traversal") || m.contains("absolute path") || m.contains("null byte") {
                    VoiceErrorCode::PackPathTraversal
                } else if m.contains("executable") {
                    VoiceErrorCode::PackExecutableContent
                } else if m.contains("provenance") {
                    VoiceErrorCode::PackProvenanceIncomplete
                } else if m.contains("limit") || m.contains("exceeds") || m.contains("exceed") {
                    VoiceErrorCode::PackSizeExceeded
                } else if m.contains("checksum") {
                    VoiceErrorCode::PackChecksumFailed
                } else {
                    VoiceErrorCode::PackInvalidFormat
                }
            }
        };
        VoiceError::new(code, message)
    }
}

#[cfg(test)]
mod pack_error_mapping_tests {
    use super::*;

    fn mapped(err: voice_pack::loader::LoadError) -> VoiceErrorCode {
        VoiceError::from(err).code()
    }

    #[test]
    fn load_errors_map_to_stable_codes() {
        use voice_pack::loader::LoadError;
        assert_eq!(mapped(LoadError::FileNotFound("x".into())), VoiceErrorCode::PackNotFound);
        assert_eq!(mapped(LoadError::MissingManifest), VoiceErrorCode::PackInvalidFormat);
        assert_eq!(
            mapped(LoadError::InvalidManifest("missing field `schema_version`".into())),
            VoiceErrorCode::PackSchemaMismatch,
            "a manifest that fails to match the schema is a schema mismatch, not a parse error"
        );
        assert_eq!(
            mapped(LoadError::InvalidManifest("expected value at line 1 column 1".into())),
            VoiceErrorCode::PackInvalidFormat,
            "unparseable JSON has no schema to disagree with"
        );
        assert_eq!(
            mapped(LoadError::LimitExceeded("too big".into())),
            VoiceErrorCode::PackSizeExceeded
        );
        assert_eq!(
            mapped(LoadError::ValidationFailed("Path traversal not allowed: ../x".into())),
            VoiceErrorCode::PackPathTraversal
        );
        assert_eq!(
            mapped(LoadError::ValidationFailed("Checksum mismatch for model.onnx".into())),
            VoiceErrorCode::PackChecksumFailed
        );
        assert_eq!(
            mapped(LoadError::ValidationFailed(
                "Provenance incomplete for real voice pack 'tara'".into()
            )),
            VoiceErrorCode::PackProvenanceIncomplete
        );
        assert_eq!(
            mapped(LoadError::ValidationFailed(
                "Executable/script content not allowed in a voice pack: a.sh".into()
            )),
            VoiceErrorCode::PackExecutableContent
        );
    }
}
