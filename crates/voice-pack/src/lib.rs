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
pub use loader::PackLoader;
pub use manifest::{PackManifest, PersonaConfig, IntentProfile};
pub use security::PackValidator;
