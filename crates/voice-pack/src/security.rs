//! Voice pack security validation
//!
//! Validates pack integrity, prevents malicious content, and checks file safety

use crate::manifest::{PackFile, PackManifest};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Security validator for voice packs
pub struct PackValidator;

impl PackValidator {
    pub fn new() -> Self {
        Self
    }

    /// Validate manifest structure and security
    pub fn validate_manifest(&self, manifest: &PackManifest) -> Result<(), String> {
        // Check schema version compatibility
        if manifest.schema_version != "1.0.0" {
            return Err(format!(
                "Unsupported schema version: {}",
                manifest.schema_version
            ));
        }

        // Check for path traversal attempts in declared files
        for file in &manifest.files {
            self.check_path_safety(&file.path)?;
        }

        Ok(())
    }

    /// Validate file integrity
    pub fn validate_files(
        &self,
        manifest: &PackManifest,
        files: &HashMap<String, Vec<u8>>,
    ) -> Result<(), String> {
        for declared_file in &manifest.files {
            // Check file exists
            if !files.contains_key(&declared_file.path) {
                return Err(format!("Declared file missing: {}", declared_file.path));
            }

            // Check checksum
            let file_data = &files[&declared_file.path];
            let checksum = self.compute_sha256(file_data);
            if checksum != declared_file.checksum_sha256 {
                return Err(format!(
                    "Checksum mismatch for {}: expected {}, got {}",
                    declared_file.path, declared_file.checksum_sha256, checksum
                ));
            }

            // Check file size
            if file_data.len() as u64 != declared_file.size_bytes {
                return Err(format!(
                    "Size mismatch for {}: expected {}, got {}",
                    declared_file.path,
                    declared_file.size_bytes,
                    file_data.len()
                ));
            }
        }

        Ok(())
    }

    /// Check for path traversal, absolute paths, and other unsafe paths
    fn check_path_safety(&self, path: &str) -> Result<(), String> {
        // Reject absolute paths
        if path.starts_with('/') {
            return Err(format!("Absolute paths not allowed: {}", path));
        }

        // Reject path traversal attempts
        if path.contains("..") {
            return Err(format!("Path traversal not allowed: {}", path));
        }

        // Reject symlinks (can't fully check in ZIP, but reject common patterns)
        if path.contains("->") {
            return Err(format!("Symlinks not allowed: {}", path));
        }

        // Reject null bytes
        if path.contains('\0') {
            return Err(format!("Null bytes not allowed: {}", path));
        }

        Ok(())
    }

    /// Compute SHA-256 checksum of data
    fn compute_sha256(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }
}

impl Default for PackValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_safety_reject_traversal() {
        let validator = PackValidator::new();
        assert!(validator.check_path_safety("../../etc/passwd").is_err());
    }

    #[test]
    fn test_path_safety_reject_absolute() {
        let validator = PackValidator::new();
        assert!(validator.check_path_safety("/etc/passwd").is_err());
    }

    #[test]
    fn test_path_safety_accept_relative() {
        let validator = PackValidator::new();
        assert!(validator.check_path_safety("models/model.onnx").is_ok());
    }

    #[test]
    fn test_sha256_computation() {
        let validator = PackValidator::new();
        let hash = validator.compute_sha256(b"test");
        // SHA256 of "test" is 9f86d081884c7d6d9ffd60bb51e3ab3d4f0eb...
        assert_eq!(hash.len(), 64); // SHA256 is 32 bytes = 64 hex chars
    }
}
