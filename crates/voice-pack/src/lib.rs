//! Chiti Voice Pack format handling
//!
//! `.cvpack` files are ZIP archives containing:
//! - Voice model files (ONNX, vocoder, etc.)
//! - Manifest (metadata, schema version, checksums)
//! - Persona configuration
//! - Provenance information

pub mod format;
pub mod loader;
pub mod manifest;
pub mod security;

pub use format::VoicePack;
pub use loader::{LoadError, LoadResult, PackLoader, MANIFEST_ENTRY};
pub use manifest::{
    IntentProfile, LoudnessConfig, PackFile, PackManifest, PersonaConfig, ProvenanceInfo, StyleConfig,
    StyleWeight, DEFAULT_LOUDNESS_TARGET_DBFS, DEFAULT_MAX_GAIN_DB, DEFAULT_PEAK_CEILING,
    STYLE_VECTOR_BYTES,
};
pub use security::{declared_paths, PackLimits, PackValidator};
