//! Voice Engine abstraction layer
//!
//! Defines the VoiceEngine trait that all TTS backends must implement.
//! This enables provider-agnostic synthesis with swappable backends.

pub mod mock;
pub mod piper;

use crate::error::VoiceResult;
use crate::synthesis::{SynthesisRequest, SynthesisResponse};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Metadata about a voice's capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceCapabilities {
    /// Unique identifier for this voice
    pub voice_id: String,
    /// Human-readable name
    pub display_name: String,
    /// Supported languages (ISO 639 codes)
    pub supported_languages: Vec<String>,
    /// Supported output formats
    pub supported_formats: Vec<String>,
    /// Whether this voice supports streaming
    pub supports_streaming: bool,
    /// Minimum text length (characters)
    pub min_text_length: usize,
    /// Maximum text length (characters)
    pub max_text_length: usize,
    /// Engine name (e.g., "piper", "kokoro")
    pub engine_name: String,
    /// Engine version (e.g., "1.0.0")
    pub engine_version: String,
}

/// Health status of a voice engine
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EngineHealth {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

/// Abstract interface for TTS backends
///
/// All backend implementations (Piper, Kokoro, etc.) must implement this trait.
/// This design ensures that applications never depend directly on a specific backend.
#[async_trait::async_trait]
pub trait VoiceEngine: Send + Sync {
    /// Initialize the engine (load models, allocate memory, etc.)
    async fn initialize(&mut self) -> VoiceResult<()>;

    /// Check the health and readiness of the engine
    async fn health(&self) -> VoiceResult<EngineHealth>;

    /// Get the capabilities of all available voices
    async fn list_voices(&self) -> VoiceResult<Vec<VoiceCapabilities>>;

    /// Get capabilities of a specific voice
    async fn voice_capabilities(&self, voice_id: &str) -> VoiceResult<VoiceCapabilities>;

    /// Perform synthesis and return complete audio
    async fn synthesize(&self, request: &SynthesisRequest) -> VoiceResult<SynthesisResponse>;

    /// Perform streaming synthesis (returns first chunk quickly)
    async fn stream(
        &self,
        request: &SynthesisRequest,
    ) -> VoiceResult<Box<dyn std::future::Future<Output = VoiceResult<Vec<u8>>> + Send>>;

    /// Stop any in-progress synthesis
    async fn stop(&self) -> VoiceResult<()>;

    /// Clean up resources
    async fn dispose(&mut self) -> VoiceResult<()>;
}

/// Type alias for a boxed, dynamically-dispatched VoiceEngine
pub type BoxedEngine = Box<dyn VoiceEngine>;

/// Registry for managing multiple voice engines
///
/// This allows multiple TTS backends to be registered and used simultaneously.
/// The persona runtime or application can select which engine to use for a given synthesis request.
pub struct VoiceEngineRegistry {
    engines: HashMap<String, Arc<tokio::sync::Mutex<BoxedEngine>>>,
}

impl VoiceEngineRegistry {
    /// Create a new engine registry
    pub fn new() -> Self {
        Self {
            engines: HashMap::new(),
        }
    }

    /// Register a new voice engine
    pub fn register(&mut self, name: String, engine: BoxedEngine) {
        self.engines.insert(name, Arc::new(tokio::sync::Mutex::new(engine)));
    }

    /// Get a registered engine
    pub fn get(&self, name: &str) -> Option<Arc<tokio::sync::Mutex<BoxedEngine>>> {
        self.engines.get(name).cloned()
    }

    /// List all registered engine names
    pub fn list_engines(&self) -> Vec<String> {
        self.engines.keys().cloned().collect()
    }

    /// Remove an engine from the registry
    pub fn unregister(&mut self, name: &str) -> Option<Arc<tokio::sync::Mutex<BoxedEngine>>> {
        self.engines.remove(name)
    }
}

impl Default for VoiceEngineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = VoiceEngineRegistry::new();
        assert_eq!(registry.list_engines().len(), 0);
    }

    #[test]
    fn test_capabilities_serialization() {
        let caps = VoiceCapabilities {
            voice_id: "tara".to_string(),
            display_name: "Tara - Indian English".to_string(),
            supported_languages: vec!["en-IN".to_string()],
            supported_formats: vec!["pcm_f32".to_string(), "wav".to_string()],
            supports_streaming: true,
            min_text_length: 1,
            max_text_length: 5000,
            engine_name: "piper".to_string(),
            engine_version: "1.0.0".to_string(),
        };

        let json = serde_json::to_string(&caps).unwrap();
        let deserialized: VoiceCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.voice_id, "tara");
    }
}
