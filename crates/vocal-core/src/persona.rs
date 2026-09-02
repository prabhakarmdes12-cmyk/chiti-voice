//! Persona runtime for managing voice personas and their prosody
//!
//! Maps text, intent, and persona configuration to synthesis parameters.

use serde::{Deserialize, Serialize};

/// A voice persona with identity and prosody parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persona {
    /// Unique persona identifier
    pub id: String,
    /// Display name
    pub display_name: String,
    /// Persona description
    pub description: String,
    /// Default speech rate (1.0 = normal)
    pub default_rate: f32,
    /// Default pitch (1.0 = normal)
    pub default_pitch: f32,
    /// Intent-to-prosody mappings
    pub intent_profiles: std::collections::HashMap<String, IntentProfile>,
}

/// Prosody parameters for a specific intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentProfile {
    /// Rate multiplier for this intent
    pub rate: f32,
    /// Pitch multiplier for this intent
    pub pitch: f32,
    /// Energy/intensity multiplier
    pub energy: f32,
    /// Pause duration multiplier
    pub pause_factor: f32,
}

/// Persona runtime that applies persona-specific transformations
pub struct PersonaRuntime {
    personas: std::collections::HashMap<String, Persona>,
}

impl PersonaRuntime {
    pub fn new() -> Self {
        Self {
            personas: std::collections::HashMap::new(),
        }
    }

    pub fn register_persona(&mut self, persona: Persona) {
        self.personas.insert(persona.id.clone(), persona);
    }

    pub fn get_persona(&self, id: &str) -> Option<&Persona> {
        self.personas.get(id)
    }

    /// Get prosody parameters for a voice + intent combination
    pub fn get_prosody(&self, persona_id: &str, intent: Option<&str>) -> Option<(f32, f32)> {
        let persona = self.get_persona(persona_id)?;

        if let Some(intent_name) = intent {
            if let Some(profile) = persona.intent_profiles.get(intent_name) {
                return Some((profile.rate, profile.pitch));
            }
        }

        Some((persona.default_rate, persona.default_pitch))
    }
}

impl Default for PersonaRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persona_creation() {
        let tara = Persona {
            id: "tara".to_string(),
            display_name: "Tara".to_string(),
            description: "Warm professional Indian English".to_string(),
            default_rate: 1.0,
            default_pitch: 1.0,
            intent_profiles: Default::default(),
        };

        assert_eq!(tara.id, "tara");
    }

    #[test]
    fn test_persona_runtime() {
        let mut runtime = PersonaRuntime::new();
        let tara = Persona {
            id: "tara".to_string(),
            display_name: "Tara".to_string(),
            description: "Warm professional Indian English".to_string(),
            default_rate: 1.0,
            default_pitch: 1.0,
            intent_profiles: Default::default(),
        };

        runtime.register_persona(tara);
        assert!(runtime.get_persona("tara").is_some());
    }
}
