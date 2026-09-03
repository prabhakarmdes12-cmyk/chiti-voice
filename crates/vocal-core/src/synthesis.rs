//! Synthesis request and response types
//!
//! Defines the API surface for text-to-speech synthesis requests and responses.

use serde::{Deserialize, Serialize};

/// Audio format for synthesis output
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SynthesisFormat {
    /// Raw PCM, 32-bit floating point, 22050 Hz
    #[serde(rename = "pcm_f32")]
    PcmF32,
    /// WAV format with PCM data
    #[serde(rename = "wav")]
    Wav,
    /// OGG/Vorbis format
    #[serde(rename = "ogg")]
    Ogg,
}

impl SynthesisFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PcmF32 => "pcm_f32",
            Self::Wav => "wav",
            Self::Ogg => "ogg",
        }
    }

    /// Deliberately an inherent `Option`-returning helper rather than
    /// `impl std::str::FromStr`: there is no error worth carrying for an unknown format
    /// name (every caller already knows its own vocabulary and maps the failure onto its
    /// own error type — the CLI onto a usage error, the pack loader onto a schema error),
    /// and `FromStr` would force an `Err` type onto the whole crate for no benefit.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pcm_f32" => Some(Self::PcmF32),
            "wav" => Some(Self::Wav),
            "ogg" => Some(Self::Ogg),
            _ => None,
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::PcmF32 => "audio/pcm",
            Self::Wav => "audio/wav",
            Self::Ogg => "audio/ogg",
        }
    }
}

/// Synthesis request with all parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisRequest {
    /// Voice identifier (e.g., "tara", "kashi", "bobo")
    pub voice: String,
    /// Text to synthesize
    pub text: String,
    /// Desired output format
    #[serde(default)]
    pub format: Option<SynthesisFormat>,
    /// Speed multiplier, the one prosody input the engine family actually has. 0.5 = half speed,
    /// 2.0 = double; 0.5..=1.6 is the band measured as intelligible, and it is the band
    /// `voice-pack` allows a persona to declare.
    #[serde(default)]
    pub rate: Option<f32>,
    /// Pitch **offset**, with the same units and neutral value (0.0) as
    /// `voice_pack::PersonaConfig::default_pitch` — not a multiplier. This pair used to disagree
    /// about what "pitch" meant while neither could be applied, which is the bug the schema now
    /// refuses: `PersonaConfig::default_pitch == 1.0` (a multiplier reading) is a load-time error.
    ///
    /// A backend has exactly two honest options for a non-zero value: realise it by selecting a
    /// different style vector (register is baked into the cast, see `pitch_baked_into_style`), or
    /// return an error. Silently ignoring it is what made the old packs lie.
    #[serde(default)]
    pub pitch: Option<f32>,
    /// Intent/style label (e.g., "warm", "calm", "alert")
    #[serde(default)]
    pub intent: Option<String>,
    /// Whether to enable streaming (return first chunk quickly)
    #[serde(default)]
    pub stream: Option<bool>,
    /// Language code (e.g., "en-IN", "hi-IN")
    #[serde(default)]
    pub language: Option<String>,
}

impl SynthesisRequest {
    pub fn new(voice: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            voice: voice.into(),
            text: text.into(),
            format: Some(SynthesisFormat::PcmF32),
            rate: None,
            pitch: None,
            intent: None,
            stream: None,
            language: None,
        }
    }

    pub fn with_format(mut self, format: SynthesisFormat) -> Self {
        self.format = Some(format);
        self
    }

    pub fn with_intent(mut self, intent: impl Into<String>) -> Self {
        self.intent = Some(intent.into());
        self
    }

    pub fn with_rate(mut self, rate: f32) -> Self {
        self.rate = Some(rate);
        self
    }

    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = Some(stream);
        self
    }
}

/// Metadata about synthesized audio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetadata {
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u8,
    /// Bit depth
    pub bit_depth: u8,
    /// Duration in milliseconds
    pub duration_ms: u32,
}

/// Synthesis response with audio data and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisResponse {
    /// The synthesized audio bytes
    #[serde(with = "serde_arrays")]
    pub audio: Vec<u8>,
    /// Format of the audio data
    pub format: SynthesisFormat,
    /// Metadata about the audio
    pub metadata: AudioMetadata,
}

mod serde_arrays {
    // `Deserialize` must be in scope for the `String::deserialize` call below. It was
    // missing in the original code — a genuine compile error (E0599) that had never
    // surfaced because the workspace manifest prevented the crate from compiling at all.
    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(data))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex::decode(&s).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_strings() {
        assert_eq!(SynthesisFormat::PcmF32.as_str(), "pcm_f32");
        assert_eq!(SynthesisFormat::Wav.as_str(), "wav");
    }

    #[test]
    fn test_format_from_str() {
        assert_eq!(
            SynthesisFormat::from_str("pcm_f32"),
            Some(SynthesisFormat::PcmF32)
        );
        assert_eq!(SynthesisFormat::from_str("invalid"), None);
    }

    #[test]
    fn test_synthesis_request_builder() {
        let req = SynthesisRequest::new("tara", "Hello world")
            .with_intent("warm")
            .with_rate(1.2);

        assert_eq!(req.voice, "tara");
        assert_eq!(req.text, "Hello world");
        assert_eq!(req.intent, Some("warm".to_string()));
        assert_eq!(req.rate, Some(1.2));
    }
}
