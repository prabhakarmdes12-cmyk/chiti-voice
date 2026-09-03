//! Voice pack security validation
//!
//! Validates pack integrity, enforces resource limits, and rejects unsafe content.
//!
//! ## Ordering is the security property
//!
//! A `.cvpack` is attacker-controlled input. Two rules govern this module:
//!
//! 1. **Never trust the archive header, and never decompress before checking it.**
//!    A ZIP entry's declared uncompressed size is known before we inflate it, so
//!    [`PackValidator::validate_archive_entry`] rejects oversized/high-ratio entries
//!    up front. This is what stops a 1 KB "zip bomb" from becoming a 4 GB
//!    allocation — the previous implementation read every member into memory
//!    *first* and validated afterwards, which is not a defence at all.
//! 2. **Only files the manifest declares may exist.** Extra entries are silently
//!    ignored by an allowlist, so a pack cannot smuggle `payload.sh` alongside a
//!    valid model.

use crate::manifest::{PackFile, PackManifest};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

/// Resource limits enforced while loading a pack (VOICE_INV_011).
#[derive(Debug, Clone)]
pub struct PackLimits {
    /// Maximum uncompressed size of a single file.
    pub max_file_bytes: u64,
    /// Maximum total uncompressed size of all files in the pack.
    pub max_total_bytes: u64,
    /// Maximum allowed uncompressed/compressed ratio per file (zip-bomb guard).
    pub max_compression_ratio: f64,
    /// Maximum number of entries in the archive.
    pub max_file_count: usize,
}

impl Default for PackLimits {
    /// Desktop/default: sized for high-quality multi-hundred-MB models.
    fn default() -> Self {
        Self {
            max_file_bytes: 512 * 1024 * 1024,
            max_total_bytes: 1024 * 1024 * 1024,
            max_compression_ratio: 32.0,
            max_file_count: 256,
        }
    }
}

impl PackLimits {
    /// Embedded/low-memory profile (Raspberry Pi class, 1-2 GB RAM).
    ///
    /// Kept deliberately tight: on a 1 GB device, a permissive total limit is a
    /// remote denial-of-service against the user's own product.
    pub const fn embedded() -> Self {
        Self {
            max_file_bytes: 64 * 1024 * 1024,
            max_total_bytes: 128 * 1024 * 1024,
            max_compression_ratio: 24.0,
            max_file_count: 64,
        }
    }

    /// Aggressive low-RAM profile for toy/microcontroller-class devices.
    pub const fn tiny() -> Self {
        Self {
            max_file_bytes: 24 * 1024 * 1024,
            max_total_bytes: 32 * 1024 * 1024,
            max_compression_ratio: 24.0,
            max_file_count: 32,
        }
    }
}

/// Extensions that must never appear inside a voice pack, regardless of manifest.
const REJECTED_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "sh", "bash", "ps1", "bat", "cmd", "vbs", "js", "jar", "py",
    "whl", "deb", "rpm", "msi", "scpt",
];

/// Security validator for voice packs.
pub struct PackValidator {
    limits: PackLimits,
    require_provenance: bool,
}

impl PackValidator {
    pub fn new() -> Self {
        Self {
            limits: PackLimits::default(),
            require_provenance: true,
        }
    }

    pub fn with_limits(limits: PackLimits) -> Self {
        Self {
            limits,
            require_provenance: true,
        }
    }

    /// Skip the provenance-completeness requirement.
    ///
    /// Intended only for research/benchmark tooling that loads untrusted local
    /// experiments. Never use this in a shipped runtime path: provenance is
    /// VOICE_INV_008 and is how you avoid shipping a voice you may not redistribute.
    pub fn without_provenance_check(mut self) -> Self {
        self.require_provenance = false;
        self
    }

    pub fn limits(&self) -> &PackLimits {
        &self.limits
    }

    /// Validate manifest structure, declared paths, and declared sizes.
    ///
    /// Runs before any archive content is decompressed.
    pub fn validate_manifest(&self, manifest: &PackManifest) -> Result<(), String> {
        if manifest.schema_version != "1.0.0" {
            return Err(format!(
                "Unsupported schema version: {}",
                manifest.schema_version
            ));
        }

        if manifest.files.len() > self.limits.max_file_count {
            return Err(format!(
                "Pack declares {} files, exceeding the limit of {}",
                manifest.files.len(),
                self.limits.max_file_count
            ));
        }

        let mut seen: HashSet<&str> = HashSet::new();
        let mut declared_total: u64 = 0;

        for file in &manifest.files {
            self.check_path_safety(&file.path)?;
            self.check_extension_safety(&file.path)?;

            if !seen.insert(file.path.as_str()) {
                return Err(format!("Duplicate file entry in manifest: {}", file.path));
            }

            if file.size_bytes > self.limits.max_file_bytes {
                return Err(format!(
                    "Declared file {} is {} bytes, exceeding the per-file limit of {}",
                    file.path, file.size_bytes, self.limits.max_file_bytes
                ));
            }

            declared_total = declared_total.saturating_add(file.size_bytes);
            if declared_total > self.limits.max_total_bytes {
                return Err(format!(
                    "Pack declares {} total uncompressed bytes, exceeding the limit of {}",
                    declared_total, self.limits.max_total_bytes
                ));
            }

            if file.size_bytes == 0 {
                return Err(format!(
                    "Declared file {} has size 0 — a real model or config is never empty. \
                     This usually means the pack was built without the model present.",
                    file.path
                ));
            }

            if file.checksum_sha256.len() != 64 {
                return Err(format!(
                    "Invalid checksum for {}: expected 64 hex chars, got {:?}",
                    file.path, file.checksum_sha256
                ));
            }
        }

        // Persona claims are the manifest's own rules (`PackManifest::validate_persona`) so the CLI
        // and `chiti-voice verify` can run them without a validator; they belong in this gate too,
        // because "the pack promises prosody the engine cannot honour" is a load-time failure, not a
        // cosmetic one — the alternative is a persona that silently synthesises as somebody else.
        manifest
            .validate_persona()
            .map_err(|e| format!("persona: {e}"))?;

        if self.require_provenance {
            self.validate_provenance(manifest)?;
        }

        Ok(())
    }

    /// Voice provenance requirements (VOICE_INV_008).
    ///
    /// Placeholder packs are allowed to be incomplete, but must say so via
    /// `"status": "placeholder"`. That keeps the gate strict for real models while
    /// letting scaffolding/CI tests exist without fabricating provenance claims.
    fn validate_provenance(&self, manifest: &PackManifest) -> Result<(), String> {
        if manifest.is_placeholder() {
            return Ok(());
        }

        let provenance = manifest
            .provenance
            .as_ref()
            .ok_or_else(|| "Real voice pack is missing a `provenance` block".to_string())?;

        let mut missing: Vec<&str> = Vec::new();
        if provenance
            .training_data_statement
            .as_deref()
            .map(|s| !s.trim().is_empty())
            != Some(true)
        {
            missing.push("training_data_statement");
        }
        if provenance
            .model_license
            .as_deref()
            .map(|s| !s.trim().is_empty())
            != Some(true)
        {
            missing.push("model_license");
        }
        if provenance
            .dataset_attribution
            .as_deref()
            .map(|s| !s.trim().is_empty())
            != Some(true)
        {
            missing.push("dataset_attribution");
        }
        if provenance.consent_obtained != Some(true) {
            missing.push("consent_obtained (must be true)");
        }

        if !missing.is_empty() {
            return Err(format!(
                "Provenance incomplete for real voice pack '{}': missing/invalid {}",
                manifest.id,
                missing.join(", ")
            ));
        }

        Ok(())
    }

    /// Validate the decompressed content of every declared file.
    pub fn validate_files(
        &self,
        manifest: &PackManifest,
        files: &HashMap<String, Vec<u8>>,
    ) -> Result<(), String> {
        for declared_file in &manifest.files {
            let file_data = match files.get(&declared_file.path) {
                Some(data) => data,
                None => return Err(format!("Declared file missing: {}", declared_file.path)),
            };

            if file_data.len() as u64 != declared_file.size_bytes {
                return Err(format!(
                    "Size mismatch for {}: expected {}, got {}",
                    declared_file.path,
                    declared_file.size_bytes,
                    file_data.len()
                ));
            }

            let checksum = self.compute_sha256(file_data);
            if !checksum_eq(&checksum, &declared_file.checksum_sha256) {
                return Err(format!(
                    "Checksum mismatch for {}: expected {}, got {}",
                    declared_file.path, declared_file.checksum_sha256, checksum
                ));
            }
        }

        Ok(())
    }

    /// Check a single archive entry against the allowlist and resource limits,
    /// using only information already present in the ZIP central directory.
    ///
    /// This must be called *before* the entry is inflated.
    pub fn validate_archive_entry(
        &self,
        name: &str,
        declared_paths: &HashSet<String>,
        compressed_size: u64,
        uncompressed_size: u64,
        running_total: u64,
    ) -> Result<(), String> {
        if name == "manifest.json" {
            return Ok(());
        }

        if name.ends_with('/') {
            return Err(format!("Directory entries are not allowed: {name}"));
        }

        self.check_path_safety(name)?;
        self.check_extension_safety(name)?;

        if !declared_paths.contains(name) {
            return Err(format!(
                "Pack contains undeclared entry {name}; only files listed in manifest.json are permitted"
            ));
        }

        if uncompressed_size > self.limits.max_file_bytes {
            return Err(format!(
                "Entry {name} expands to {uncompressed_size} bytes, over the per-file limit of {}",
                self.limits.max_file_bytes
            ));
        }

        let projected_total = running_total.saturating_add(uncompressed_size);
        if projected_total > self.limits.max_total_bytes {
            return Err(format!(
                "Pack would expand to {projected_total} bytes, over the total limit of {}",
                self.limits.max_total_bytes
            ));
        }

        if compressed_size > 0 {
            let ratio = uncompressed_size as f64 / compressed_size as f64;
            if ratio > self.limits.max_compression_ratio {
                return Err(format!(
                    "Entry {name} has a compression ratio of {ratio:.1} (limit {:.1}) — \
                     refusing to inflate a probable zip bomb",
                    self.limits.max_compression_ratio
                ));
            }
        }

        Ok(())
    }

    /// Check for path traversal, absolute paths, and other unsafe paths.
    ///
    /// Per-component, not substring: rejecting any path containing `..` would
    /// wrongly reject legitimate names like `voice-1..2.onnx`, and would miss a
    /// backslash traversal (`..\..\windows`) on Windows.
    fn check_path_safety(&self, path: &str) -> Result<(), String> {
        if path.is_empty() {
            return Err("Empty file path in pack".to_string());
        }
        if path.starts_with('/') || path.starts_with('\\') {
            return Err(format!("Absolute paths not allowed: {path}"));
        }
        // Windows drive letters / UNC
        let bytes = path.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
            return Err(format!("Drive-absolute paths not allowed: {path}"));
        }
        if path.contains('\0') {
            return Err(format!("Null bytes not allowed: {path}"));
        }
        if path.contains("->") {
            return Err(format!("Symlink-like target not allowed: {path}"));
        }

        for component in path.split(['/', '\\']) {
            match component {
                "" | "." => {
                    return Err(format!("Empty or '.' path component not allowed: {path}"));
                }
                ".." => {
                    return Err(format!("Path traversal not allowed: {path}"));
                }
                c if c.contains(':') => {
                    return Err(format!(
                        "Suspicious path component not allowed: {path} ({c})"
                    ));
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Reject files that should never be shipped inside a data-only voice pack.
    fn check_extension_safety(&self, path: &str) -> Result<(), String> {
        let name = path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(path)
            .to_ascii_lowercase();

        for ext in REJECTED_EXTENSIONS {
            if name.ends_with(&format!(".{ext}")) {
                return Err(format!(
                    "Executable/script content not allowed in a voice pack: {path}"
                ));
            }
        }

        // ONNX External Data files are only dangerous if they can escape the pack,
        // which the allowlist above already prevents; nothing extra to reject here.
        Ok(())
    }

    /// Compute SHA-256 checksum of data.
    fn compute_sha256(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }
}

/// Case-insensitive hex comparison that never short-circuits on length alone.
fn checksum_eq(computed: &str, declared: &str) -> bool {
    // Digests are compared case-insensitively because some tooling emits uppercase hex.
    // ASCII-only folding is deliberate, not a shortcut: Unicode case-folding is not
    // injective (dotless/dotted i, long s, final sigma), so a Unicode-aware comparison
    // would let an attacker's spelling collide with a declared digest.
    computed.eq_ignore_ascii_case(declared)
}

impl Default for PackValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Declared paths of all files in a manifest, for allowlist checks.
pub fn declared_paths(manifest: &PackManifest) -> HashSet<String> {
    manifest
        .files
        .iter()
        .map(|f: &PackFile| f.path.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{FileType, ProvenanceInfo};

    #[test]
    fn test_path_safety_reject_traversal() {
        let validator = PackValidator::new();
        assert!(validator.check_path_safety("../../etc/passwd").is_err());
        assert!(validator
            .check_path_safety("models/../../etc/passwd")
            .is_err());
        assert!(validator
            .check_path_safety("..\\..\\windows\\system32")
            .is_err());
    }

    #[test]
    fn test_path_safety_reject_absolute() {
        let validator = PackValidator::new();
        assert!(validator.check_path_safety("/etc/passwd").is_err());
        assert!(validator.check_path_safety("C:\\evil.onnx").is_err());
        assert!(validator.check_path_safety("\\\\host\\share\\x").is_err());
    }

    #[test]
    fn test_path_safety_accept_relative() {
        let validator = PackValidator::new();
        assert!(validator.check_path_safety("models/model.onnx").is_ok());
        // Substring `..` inside a filename must NOT be a false rejection:
        assert!(validator.check_path_safety("voices/tara-1..2.onnx").is_ok());
    }

    #[test]
    fn test_rejects_executable_extensions() {
        let validator = PackValidator::new();
        assert!(validator
            .check_extension_safety("hooks/post_install.sh")
            .is_err());
        assert!(validator
            .check_extension_safety("native/payload.dll")
            .is_err());
        assert!(validator.check_extension_safety("model.onnx").is_ok());
        assert!(validator.check_extension_safety("MODEL.ONNX").is_ok());
    }

    #[test]
    fn test_sha256_computation() {
        let validator = PackValidator::new();
        let hash = validator.compute_sha256(b"test");
        assert_eq!(hash.len(), 64);
        // Known SHA-256 of "test".
        assert_eq!(
            hash,
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
        );
    }

    #[test]
    fn test_checksum_eq() {
        assert!(checksum_eq("abcd", "ABCD"));
        assert!(!checksum_eq("abcd", "abc"));
        assert!(!checksum_eq("abcd", "abce"));
    }

    fn manifest_with(files: Vec<PackFile>, status: Option<&str>) -> PackManifest {
        PackManifest {
            schema_version: "1.0.0".to_string(),
            id: "tara".to_string(),
            name: "Tara".to_string(),
            version: "1.0.0".to_string(),
            author: "Chiti Technologies".to_string(),
            license: "Proprietary".to_string(),
            description: "test".to_string(),
            engine_family: "piper".to_string(),
            engine_version_min: "1.0.0".to_string(),
            supported_languages: vec!["en-IN".to_string()],
            files,
            persona: None,
            provenance: Some(ProvenanceInfo {
                training_data_statement: Some("12 h studio English".to_string()),
                model_license: Some("MIT".to_string()),
                consent_obtained: Some(true),
                dataset_attribution: Some("recorded under contract".to_string()),
                build_timestamp: Some("2026-09-03T00:00:00Z".to_string()),
                signature: None,
                signature_status: Some("UNSIGNED".to_string()),
            }),
            status: status.map(|s| s.to_string()),
        }
    }

    fn file(path: &str, size: u64) -> PackFile {
        PackFile {
            path: path.to_string(),
            checksum_sha256: "a".repeat(64),
            size_bytes: size,
            file_type: FileType::Model,
        }
    }

    #[test]
    fn test_zero_sized_declared_file_rejected() {
        // This is exactly the bug that made every shipped .cvpack in this repo
        // unloadable: manifests declared size 0 / zero-hash for a placeholder.
        let m = manifest_with(vec![file("model.onnx", 0)], None);
        let err = PackValidator::new().validate_manifest(&m).unwrap_err();
        assert!(err.contains("size 0"), "unexpected error: {err}");
    }

    #[test]
    fn test_duplicate_manifest_entries_rejected() {
        let m = manifest_with(
            vec![file("model.onnx", 100), file("model.onnx", 100)],
            Some("placeholder"),
        );
        assert!(PackValidator::new().validate_manifest(&m).is_err());
    }

    #[test]
    fn test_provenance_required_for_real_pack() {
        let mut m = manifest_with(vec![file("model.onnx", 100)], None);
        m.provenance = None;
        let err = PackValidator::new().validate_manifest(&m).unwrap_err();
        assert!(err.contains("provenance"), "unexpected error: {err}");
    }

    #[test]
    fn test_provenance_not_required_for_placeholder() {
        let mut m = manifest_with(vec![file("model.onnx", 100)], Some("placeholder"));
        m.provenance = None;
        assert!(PackValidator::new().validate_manifest(&m).is_ok());
    }

    #[test]
    fn test_declared_total_over_limit_rejected() {
        let m = manifest_with(vec![file("model.onnx", 200 * 1024 * 1024)], None);
        let validator = PackValidator::with_limits(PackLimits::embedded());
        let err = validator.validate_manifest(&m).unwrap_err();
        assert!(err.contains("limit"), "unexpected error: {err}");
    }

    #[test]
    fn test_zip_bomb_entry_rejected_before_inflation() {
        let validator = PackValidator::with_limits(PackLimits::embedded());
        let declared: HashSet<String> = ["model.onnx".to_string()].into_iter().collect();

        // 2 GB uncompressed from 1 MB compressed: must be refused on header sizes.
        let err = validator
            .validate_archive_entry("model.onnx", &declared, 1_048_576, 2_147_483_648, 0)
            .unwrap_err();
        assert!(
            err.contains("per-file limit") || err.contains("zip bomb") || err.contains("expand"),
            "unexpected error: {err}"
        );

        // Undeclared entry is refused even if small.
        assert!(validator
            .validate_archive_entry("extra.sh", &declared, 10, 10, 0)
            .is_err());

        // Normal entry passes.
        assert!(validator
            .validate_archive_entry("model.onnx", &declared, 900_000, 1_000_000, 0)
            .is_ok());
    }

    #[test]
    fn test_tiny_profile_rejects_piper_large_model() {
        // Documents the real constraint: a "large"/high-quality Piper voice is
        // ~100+ MB and will not pass a 24 MB/embedded budget.
        let large = 120 * 1024 * 1024;
        assert!(PackValidator::with_limits(PackLimits::tiny())
            .validate_manifest(&manifest_with(vec![file("model.onnx", large)], None))
            .is_err());
        assert!(PackValidator::new()
            .validate_manifest(&manifest_with(vec![file("model.onnx", large)], None))
            .is_ok());
    }
}
