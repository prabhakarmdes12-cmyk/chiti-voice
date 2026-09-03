//! Voice pack loader
//!
//! Loads and validates `.cvpack` ZIP files.
//!
//! ## Load order (security-critical)
//!
//! 1. read `manifest.json` only (size-capped)
//! 2. parse + validate manifest (schema, paths, declared sizes, provenance)
//! 3. scan the archive's central directory and reject any entry that is undeclared,
//!    unsafe, oversized, or absurdly compressible — **without inflating it**
//! 4. inflate each allowed entry under an enforced byte budget
//! 5. verify per-file SHA-256 and size
//!
//! Steps 3-4 are the reason limits live here rather than in the validator alone:
//! validation that happens *after* decompression cannot prevent decompression
//! bombs.

use crate::format::VoicePack;
use crate::manifest::PackManifest;
use crate::security::{declared_paths, PackValidator};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use thiserror::Error;

/// Name of the manifest entry inside a `.cvpack`.
pub const MANIFEST_ENTRY: &str = "manifest.json";

/// Hard cap on the manifest itself, so a hostile pack cannot make us parse a
/// multi-gigabyte JSON document.
const MAX_MANIFEST_BYTES: u64 = 8 * 1024 * 1024;

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
    #[error("Resource limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type LoadResult<T> = Result<T, LoadError>;

/// Loads voice packs from `.cvpack` files.
pub struct PackLoader {
    validator: PackValidator,
}

impl PackLoader {
    /// Loader with default (desktop) resource limits.
    pub fn new() -> Self {
        Self {
            validator: PackValidator::new(),
        }
    }

    /// Loader with an explicit validator (e.g. embedded/tiny limits).
    pub fn with_validator(validator: PackValidator) -> Self {
        Self { validator }
    }

    /// Load a voice pack from a file path.
    pub fn load(&self, pack_path: &Path) -> LoadResult<VoicePack> {
        if !pack_path.exists() {
            return Err(LoadError::FileNotFound(
                pack_path.to_string_lossy().to_string(),
            ));
        }

        let file = std::fs::File::open(pack_path).map_err(LoadError::IoError)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| LoadError::InvalidZip(e.to_string()))?;

        let entry_count = archive.len();
        if entry_count > self.validator.limits().max_file_count + 1 {
            return Err(LoadError::LimitExceeded(format!(
                "archive declares {entry_count} entries, over the limit of {}",
                self.validator.limits().max_file_count
            )));
        }

        // ── 1. manifest (size-capped) ────────────────────────────────────────
        let manifest_content = {
            let mut manifest_file = archive
                .by_name(MANIFEST_ENTRY)
                .map_err(|_| LoadError::MissingManifest)?;
            if manifest_file.size() > MAX_MANIFEST_BYTES {
                return Err(LoadError::LimitExceeded(format!(
                    "manifest.json is {} bytes, over the {} byte cap",
                    manifest_file.size(),
                    MAX_MANIFEST_BYTES
                )));
            }
            let mut content = String::new();
            manifest_file
                .read_to_string(&mut content)
                .map_err(LoadError::IoError)?;
            content
        };

        // ── 2. parse + validate ─────────────────────────────────────────────
        let manifest: PackManifest = serde_json::from_str(&manifest_content)
            .map_err(|e| LoadError::InvalidManifest(e.to_string()))?;
        manifest.validate().map_err(LoadError::InvalidManifest)?;
        self.validator
            .validate_manifest(&manifest)
            .map_err(LoadError::ValidationFailed)?;

        // ── 3 + 4. guarded inflate ──────────────────────────────────────────
        let declared = declared_paths(&manifest);
        let max_file_bytes = self.validator.limits().max_file_bytes;
        let max_total_bytes = self.validator.limits().max_total_bytes;

        let mut files: HashMap<String, Vec<u8>> = HashMap::new();
        let mut running_total: u64 = 0;

        for i in 0..entry_count {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| LoadError::InvalidZip(e.to_string()))?;

            let name = entry.name().to_string();
            if name == MANIFEST_ENTRY {
                continue;
            }

            let compressed_size = entry.compressed_size();
            let advertised_size = entry.size();

            self.validator
                .validate_archive_entry(
                    &name,
                    &declared,
                    compressed_size,
                    advertised_size,
                    running_total,
                )
                .map_err(LoadError::ValidationFailed)?;

            let remaining_total = max_total_bytes.saturating_sub(running_total);
            let allowed = max_file_bytes.min(remaining_total);

            // Read one byte past the allowance so an entry that LIES about its
            // size in the central directory still gets caught.
            let mut content: Vec<u8> = Vec::new();
            entry
                .by_ref()
                .take(allowed.saturating_add(1))
                .read_to_end(&mut content)
                .map_err(LoadError::IoError)?;

            if content.len() as u64 > allowed {
                return Err(LoadError::LimitExceeded(format!(
                    "{name} expands beyond the allowed {allowed} bytes"
                )));
            }

            running_total = running_total.saturating_add(content.len() as u64);
            files.insert(name, content);
        }

        // ── 5. integrity ────────────────────────────────────────────────────
        self.validator
            .validate_files(&manifest, &files)
            .map_err(LoadError::ValidationFailed)?;

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

    #[test]
    fn test_missing_file_reports_not_found() {
        let loader = PackLoader::new();
        let err = loader
            .load(Path::new("/nonexistent/nope.cvpack"))
            .unwrap_err();
        assert!(matches!(err, LoadError::FileNotFound(_)), "got {err:?}");
    }

    #[test]
    fn test_not_a_zip() {
        let path = std::env::temp_dir().join("chiti_not_a_zip.cvpack");
        std::fs::write(&path, b"this is not a zip archive").unwrap();
        let loader = PackLoader::new();
        let err = loader.load(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(matches!(err, LoadError::InvalidZip(_)), "got {err:?}");
    }
}
