//! Piper TTS Engine Adapter
//!
//! Adapts the Piper TTS model to the VoiceEngine interface.
//! Piper is a fast, lightweight neural TTS model suitable for embedded and offline synthesis.

use crate::engine::VoiceCapabilities;
use crate::error::{VoiceErrorCode, VoiceResult};
use crate::synthesis::{SynthesisRequest, SynthesisResponse};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

/// Configuration for a Piper voice model
#[derive(Debug, Clone)]
pub struct PiperVoiceConfig {
    /// Voice identifier (e.g., "tara-en-IN", "kashi-hi-IN")
    pub voice_id: String,
    /// Path to the ONNX model file within the voice pack
    pub model_path: String,
    /// Supported language (ISO 639 code)
    pub language: String,
    /// Sample rate of the model
    pub sample_rate: u32,
    /// Phoneme set used by the model
    pub phonemes: Vec<String>,
}

/// Piper TTS Engine implementation
pub struct PiperEngine {
    voices: HashMap<String, PiperVoiceConfig>,
    capabilities: Vec<VoiceCapabilities>,
    current_voice: Arc<Mutex<Option<String>>>,
}

impl PiperEngine {
    pub fn new() -> Self {
        Self {
            voices: HashMap::new(),
            capabilities: Vec::new(),
            current_voice: Arc::new(Mutex::new(None)),
        }
    }

    /// Register a voice model with this engine
    pub fn register_voice(&mut self, config: PiperVoiceConfig) {
        let voice_id = config.voice_id.clone();
        let language = config.language.clone();

        // Create capability info
        let cap = VoiceCapabilities {
            voice_id: voice_id.clone(),
            display_name: format!("Piper {}", voice_id),
            supported_languages: vec![language],
            supported_formats: vec![
                "pcm_f32".to_string(),
                "wav".to_string(),
                "ogg".to_string(),
            ],
            supports_streaming: true,
            min_text_length: 1,
            max_text_length: 5000,
            engine_name: "piper".to_string(),
            engine_version: "1.0.0".to_string(),
        };

        // `voice_id` is used again by the log line below, so it must not be moved into
        // the map. (E0382 — pre-existing in this file, previously masked because the
        // workspace never compiled.)
        self.voices.insert(voice_id.clone(), config);
        self.capabilities.push(cap);

        debug!("Registered Piper voice: {voice_id}");
    }

    /// Get configuration for a specific voice
    pub fn get_voice_config(&self, voice_id: &str) -> Option<&PiperVoiceConfig> {
        self.voices.get(voice_id)
    }
}

impl Default for PiperEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::engine::VoiceEngine for PiperEngine {
    async fn initialize(&mut self) -> VoiceResult<()> {
        // TODO: Initialize ONNX Runtime, load model metadata
        // For now, this is a no-op
        info!("Piper engine initialized");
        Ok(())
    }

    async fn health(&self) -> VoiceResult<crate::engine::EngineHealth> {
        // Deliberately never reports Healthy: there is no ONNX inference path in
        // this engine yet, so a "Healthy" status would be a lie that applications
        // would build against.
        if self.voices.is_empty() {
            Ok(crate::engine::EngineHealth::Unhealthy(
                "No voices registered".to_string(),
            ))
        } else {
            Ok(crate::engine::EngineHealth::Unhealthy(
                "Piper backend not implemented: voices are registered but no audio can be synthesized (ONNX inference pending)".to_string(),
            ))
        }
    }

    async fn list_voices(&self) -> VoiceResult<Vec<VoiceCapabilities>> {
        Ok(self.capabilities.clone())
    }

    async fn voice_capabilities(&self, voice_id: &str) -> VoiceResult<VoiceCapabilities> {
        self.capabilities
            .iter()
            .find(|v| v.voice_id == voice_id)
            .cloned()
            .ok_or_else(|| {
                error!("Voice not found: {}", voice_id);
                crate::error::VoiceError::new(
                    VoiceErrorCode::VoiceNotFound,
                    format!("Voice not found: {}", voice_id),
                )
            })
    }

    async fn synthesize(&self, _request: &SynthesisRequest) -> VoiceResult<SynthesisResponse> {
        // Capability is reported before request validation, deliberately. Refusing with
        // VoiceNotFound for an unknown id would tell the caller to fix the request and
        // retry, when in fact no request can succeed in this build. (Same reasoning as
        // an HTTP API returning 503 rather than 404 for a bad parameter on a dead model.)
        // The trait documents this ordering contract for all engines.
        //
        // NOT IMPLEMENTED. No code in this crate references `ort`, so there is nothing
        // to validate against yet; this engine can list voices but cannot produce audio,
        // and says so loudly rather than returning silence that looks like success.
        // When inference lands (docs/ROADMAP_EMBEDDED.md Step 1), validate
        // `request.voice` against `self.get_voice_config` *after* this refusal and
        // return VoiceErrorCode::VoiceNotFound for ids that are not registered.
        Err(crate::error::VoiceError::new(
            VoiceErrorCode::EngineNotAvailable,
            "Piper synthesis is not implemented: no ONNX inference path exists yet (see docs/ROADMAP_EMBEDDED.md, Step 1)",
        ))
    }

    async fn stream(
        &self,
        _request: &SynthesisRequest,
    ) -> VoiceResult<
        std::pin::Pin<Box<dyn std::future::Future<Output = VoiceResult<Vec<u8>>> + Send>>,
    > {
        Err(crate::error::VoiceError::new(
            VoiceErrorCode::EngineNotAvailable,
            "Piper streaming is not implemented: no ONNX inference path exists yet",
        ))
    }

    async fn stop(&self) -> VoiceResult<()> {
        // Update current voice state
        let mut voice = self.current_voice.lock().await;
        *voice = None;
        Ok(())
    }

    async fn dispose(&mut self) -> VoiceResult<()> {
        // TODO: Clean up ONNX Runtime resources
        self.voices.clear();
        self.capabilities.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The trait is implemented via a fully-qualified path above
    // (`impl crate::engine::VoiceEngine for ...`), so its methods are not in
    // scope here unless the trait itself is imported.
    use crate::engine::VoiceEngine;

    #[test]
    fn test_piper_engine_creation() {
        let engine = PiperEngine::new();
        assert!(engine.voices.is_empty());
    }

    #[test]
    fn test_register_voice() {
        let mut engine = PiperEngine::new();
        let config = PiperVoiceConfig {
            voice_id: "tara-en-IN".to_string(),
            model_path: "models/tara.onnx".to_string(),
            language: "en-IN".to_string(),
            sample_rate: 22050,
            phonemes: vec![],
        };

        engine.register_voice(config);
        assert_eq!(engine.capabilities.len(), 1);
        assert!(engine.get_voice_config("tara-en-IN").is_some());
    }

    #[tokio::test]
    async fn test_piper_health() {
        let engine = PiperEngine::new();
        let health = engine.health().await.unwrap();
        assert!(matches!(health, crate::engine::EngineHealth::Unhealthy(_)));
    }

    #[tokio::test]
    async fn test_piper_voice_not_found() {
        let engine = PiperEngine::new();
        let result = engine.voice_capabilities("nonexistent").await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), VoiceErrorCode::VoiceNotFound);
    }
}
