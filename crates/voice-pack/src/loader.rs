//! Voice pack loader
//!
//! Loads and validates .cvpack ZIP files

use crate::format::VoicePack;
use crate::manifest::PackManifest;
use crate::security::PackValidator;
use std::collections::HashMap;
use std::path::Path;
use std::io::Read;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("Pack file not found: {0}")]
    FileNotFound(String),
    #[error("Invalid ZIP format: {0}")]
    InvalidZip(String),
    #[error("Missing manifest.json")]
    MissingManifest,
    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type LoadResult<T> = Result<T, LoadError>;

/// Loads voice packs from .cvpack files
pub struct PackLoader {
    validator: PackValidator,
}

impl PackLoader {
    pub fn new() -> Self {
        Self {
            validator: PackValidator::new(),
        }
    }

    /// Load a voice pack from a file path
    pub fn load(&self, pack_path: &Path) -> LoadResult<VoicePack> {
        // Check if file exists
        if !pack_path.exists() {
            return Err(LoadError::FileNotFound(
                pack_path.to_string_lossy().to_string(),
            ));
        }

        // Open ZIP archive
        let file = std::fs::File::open(pack_path)
            .map_err(|e| LoadError::IoError(e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| LoadError::InvalidZip(e.to_string()))?;

        // Read manifest.json
        let mut manifest_file = archive
            .by_name("manifest.json")
            .map_err(|_| LoadError::MissingManifest)?;

        let mut manifest_content = String::new();
        std::io::Read::read_to_string(&mut manifest_file, &mut manifest_content)
            .map_err(|e| LoadError::IoError(e))?;

        let manifest: PackManifest = serde_json::from_str(&manifest_content)
            .map_err(|e| LoadError::InvalidManifest(e.to_string()))?;

        // Validate manifest
        manifest
            .validate()
            .map_err(|e| LoadError::InvalidManifest(e))?;

        // Load all files
        let mut files = HashMap::new();
        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| LoadError::InvalidZip(e.to_string()))?;
            if !file.is_dir() && file.name() != "manifest.json" {
                let mut content = Vec::new();
                std::io::Read::read_to_end(&mut file, &mut content)
                    .map_err(|e| LoadError::IoError(e))?;
                files.insert(file.name().to_string(), content);
            }
        }

        // Run security validations
        self.validator
            .validate_manifest(&manifest)
            .map_err(|e| LoadError::ValidationFailed(e))?;

        self.validator
            .validate_files(&manifest, &files)
            .map_err(|e| LoadError::ValidationFailed(e))?;

        Ok(VoicePack::new(
            pack_path.to_string_lossy().to_string(),
            manifest,
            files,
        ))
    }
}

impl Default for PackLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loader_creation() {
        let _loader = PackLoader::new();
    }
}
