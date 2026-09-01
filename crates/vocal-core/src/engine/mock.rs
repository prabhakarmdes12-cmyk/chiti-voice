//! Mock TTS Engine for testing
//!
//! A stub engine that produces silence instead of real speech.
//! Used for unit tests and CI validation without requiring model files.

use crate::engine::VoiceCapabilities;
use crate::error::{VoiceErrorCode, VoiceResult};
use crate::synthesis::{AudioMetadata, SynthesisFormat, SynthesisRequest, SynthesisResponse};
use async_trait::async_trait;

/// Mock engine for testing - produces silence
pub struct MockEngine {
    voices: Vec<VoiceCapabilities>,
    sample_rate: u32,
}

impl MockEngine {
    pub fn new() -> Self {
        Self {
            voices: vec![
                VoiceCapabilities {
                    voice_id: "tara-mock".to_string(),
                    display_name: "Tara (Mock)".to_string(),
                    supported_languages: vec!["en-IN".to_string()],
                    supported_formats: vec![
                        "pcm_f32".to_string(),
                        "wav".to_string(),
                        "ogg".to_string(),
                    ],
                    supports_streaming: true,
                    min_text_length: 1,
                    max_text_length: 5000,
                    engine_name: "mock".to_string(),
                    engine_version: "0.1.0".to_string(),
                },
                VoiceCapabilities {
                    voice_id: "kashi-mock".to_string(),
                    display_name: "Kashi (Mock)".to_string(),
                    supported_languages: vec!["hi-IN".to_string()],
                    supported_formats: vec![
                        "pcm_f32".to_string(),
                        "wav".to_string(),
                        "ogg".to_string(),
                    ],
                    supports_streaming: true,
                    min_text_length: 1,
                    max_text_length: 5000,
                    engine_name: "mock".to_string(),
                    engine_version: "0.1.0".to_string(),
                },
            ],
            sample_rate: 22050,
        }
    }

    /// Generate silent audio of specified length
    fn generate_silence(&self, duration_seconds: f32) -> Vec<u8> {
        let num_samples = (duration_seconds * self.sample_rate as f32) as usize;
        // Each sample is 4 bytes (f32)
        let silence = vec![0u8; num_samples * 4];
        silence
    }
}

impl Default for MockEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::engine::VoiceEngine for MockEngine {
    async fn initialize(&mut self) -> VoiceResult<()> {
        Ok(())
    }

    async fn health(&self) -> VoiceResult<crate::engine::EngineHealth> {
        Ok(crate::engine::EngineHealth::Healthy)
    }

    async fn list_voices(&self) -> VoiceResult<Vec<VoiceCapabilities>> {
        Ok(self.voices.clone())
    }

    async fn voice_capabilities(&self, voice_id: &str) -> VoiceResult<VoiceCapabilities> {
        self.voices
            .iter()
            .find(|v| v.voice_id == voice_id)
            .cloned()
            .ok_or_else(|| {
                crate::error::VoiceError::new(
                    VoiceErrorCode::VoiceNotFound,
                    format!("Voice not found: {}", voice_id),
                )
            })
    }

    async fn synthesize(&self, request: &SynthesisRequest) -> VoiceResult<SynthesisResponse> {
        // Validate voice exists
        let _caps = self.voice_capabilities(&request.voice).await?;

        // Generate silence (proportional to text length)
        let estimated_duration = (request.text.len() as f32) / 100.0; // ~100 chars per second
        let audio = self.generate_silence(estimated_duration);

        Ok(SynthesisResponse {
            audio,
            format: request.format.unwrap_or(SynthesisFormat::PcmF32),
            metadata: AudioMetadata {
                sample_rate: self.sample_rate,
                channels: 1,
                bit_depth: 32,
                duration_ms: (estimated_duration * 1000.0) as u32,
            },
        })
    }

    async fn stream(
        &self,
        request: &SynthesisRequest,
    ) -> VoiceResult<Box<dyn std::future::Future<Output = VoiceResult<Vec<u8>>> + Send>> {
        let request = request.clone();
        let sample_rate = self.sample_rate;

        let future = Box::new(async move {
            let estimated_duration = (request.text.len() as f32) / 100.0;
            let num_samples = (estimated_duration * sample_rate as f32) as usize;
            let silence = vec![0u8; num_samples * 4];
            Ok(silence)
        });

        Ok(future)
    }

    async fn stop(&self) -> VoiceResult<()> {
        Ok(())
    }

    async fn dispose(&mut self) -> VoiceResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_engine_initialization() {
        let mut engine = MockEngine::new();
        assert!(engine.initialize().await.is_ok());
    }

    #[tokio::test]
    async fn test_mock_engine_health() {
        let engine = MockEngine::new();
        let health = engine.health().await;
        assert!(health.is_ok());
    }

    #[tokio::test]
    async fn test_mock_engine_list_voices() {
        let engine = MockEngine::new();
        let voices = engine.list_voices().await.unwrap();
        assert_eq!(voices.len(), 2);
    }

    #[tokio::test]
    async fn test_mock_engine_synthesize() {
        let engine = MockEngine::new();
        let request = SynthesisRequest::new("tara-mock", "Hello, world!");
        let response = engine.synthesize(&request).await.unwrap();
        assert!(!response.audio.is_empty());
        assert_eq!(response.metadata.sample_rate, 22050);
    }

    #[test]
    fn test_generate_silence() {
        let engine = MockEngine::new();
        let silence = engine.generate_silence(1.0); // 1 second
        let expected_bytes = 1 * 22050 * 4; // 1 second * sample rate * 4 bytes per sample
        assert_eq!(silence.len(), expected_bytes);
    }
}
