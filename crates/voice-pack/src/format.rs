//! Voice pack format (.cvpack)
//!
//! A .cvpack file is a ZIP archive with:
//! - manifest.json at the root
//! - Voice model files
//! - Persona configuration

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Represents a loaded voice pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoicePack {
    /// Path to the .cvpack file
    pub file_path: String,
    /// Extracted manifest
    pub manifest: crate::manifest::PackManifest,
    /// File contents (mapped by relative path)
    pub files: std::collections::HashMap<String, Vec<u8>>,
}

impl VoicePack {
    pub fn new(
        file_path: String,
        manifest: crate::manifest::PackManifest,
        files: std::collections::HashMap<String, Vec<u8>>,
    ) -> Self {
        Self {
            file_path,
            manifest,
            files,
        }
    }

    pub fn get_file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(|v| v.as_slice())
    }

    pub fn file_size(&self, path: &str) -> Option<usize> {
        self.files.get(path).map(|v| v.len())
    }

    pub fn list_files(&self) -> Vec<&str> {
        self.files.keys().map(|k| k.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_pack_creation() {
        let manifest = crate::manifest::PackManifest {
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
            files: vec![],
            persona: None,
            provenance: None,
        };

        let pack = VoicePack::new("tara.cvpack".to_string(), manifest, Default::default());
        assert_eq!(pack.file_path, "tara.cvpack");
    }
}
