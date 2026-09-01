//! Voice pack manifest format (manifest.json)
//!
//! Defines the structure and validation of the manifest inside a .cvpack file.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The manifest.json structure inside a .cvpack file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackManifest {
    /// Schema version (e.g., "1.0.0")
    pub schema_version: String,
    /// Unique voice pack identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Semantic version of this pack
    pub version: String,
    /// Voice pack author/creator
    pub author: String,
    /// License identifier (SPDX format)
    pub license: String,
    /// Description of the voice
    pub description: String,
    /// Engine family (e.g., "piper", "kokoro")
    pub engine_family: String,
    /// Minimum engine version required
    pub engine_version_min: String,
    /// Supported languages (ISO 639 codes)
    pub supported_languages: Vec<String>,
    /// Files declared in the pack
    pub files: Vec<PackFile>,
    /// Persona configuration
    #[serde(default)]
    pub persona: Option<PersonaConfig>,
    /// Provenance information
    #[serde(default)]
    pub provenance: Option<ProvenanceInfo>,
}

/// File entry in the manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackFile {
    /// Relative path within the pack (ZIP)
    pub path: String,
    /// SHA-256 checksum in hex
    pub checksum_sha256: String,
    /// File size in bytes
    pub size_bytes: u64,
    /// File type/purpose
    pub file_type: FileType,
}

/// Type of file in the pack
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "vocoder")]
    Vocoder,
    #[serde(rename = "phonemes")]
    Phonemes,
    #[serde(rename = "metadata")]
    Metadata,
    #[serde(rename = "config")]
    Config,
}

/// Persona configuration embedded in the pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaConfig {
    /// Persona identifier
    pub id: String,
    /// Display name
    pub display_name: String,
    /// Description
    pub description: String,
    /// Default speech rate
    pub default_rate: f32,
    /// Default pitch
    pub default_pitch: f32,
    /// Intent profiles
    pub intent_profiles: HashMap<String, IntentProfile>,
}

/// Intent-specific prosody
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentProfile {
    pub rate: f32,
    pub pitch: f32,
    pub energy: f32,
    pub pause_factor: f32,
}

/// Provenance and attribution information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceInfo {
    /// Training data statement
    pub training_data_statement: Option<String>,
    /// Model license
    pub model_license: Option<String>,
    /// Whether consent was obtained
    pub consent_obtained: Option<bool>,
    /// Dataset attribution
    pub dataset_attribution: Option<String>,
    /// Build timestamp (RFC 3339 format)
    pub build_timestamp: Option<String>,
    /// Digital signature (Ed25519)
    pub signature: Option<String>,
    /// Signature status
    pub signature_status: Option<String>,
}

impl PackManifest {
    /// Validate the manifest structure
    pub fn validate(&self) -> Result<(), String> {
        // Check required fields
        if self.schema_version.is_empty() {
            return Err("schema_version is required".to_string());
        }
        if self.id.is_empty() {
            return Err("id is required".to_string());
        }
        if self.name.is_empty() {
            return Err("name is required".to_string());
        }
        if self.files.is_empty() {
            return Err("files list cannot be empty".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_validation() {
        let manifest = PackManifest {
            schema_version: "1.0.0".to_string(),
            id: "tara".to_string(),
            name: "Tara".to_string(),
            version: "1.0.0".to_string(),
            author: "Chiti".to_string(),
            license: "Proprietary".to_string(),
            description: "Indian English voice".to_string(),
            engine_family: "piper".to_string(),
            engine_version_min: "1.0.0".to_string(),
            supported_languages: vec!["en-IN".to_string()],
            files: vec![PackFile {
                path: "model.onnx".to_string(),
                checksum_sha256: "abc123".to_string(),
                size_bytes: 1000,
                file_type: FileType::Model,
            }],
            persona: None,
            provenance: None,
        };

        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_manifest_validation_fail() {
        let manifest = PackManifest {
            schema_version: "1.0.0".to_string(),
            id: "".to_string(),
            name: "".to_string(),
            version: "1.0.0".to_string(),
            author: "Chiti".to_string(),
            license: "Proprietary".to_string(),
            description: "Indian English voice".to_string(),
            engine_family: "piper".to_string(),
            engine_version_min: "1.0.0".to_string(),
            supported_languages: vec![],
            files: vec![],
            persona: None,
            provenance: None,
        };

        assert!(manifest.validate().is_err());
    }
}
