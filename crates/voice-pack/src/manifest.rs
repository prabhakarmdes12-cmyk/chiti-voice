//! Voice pack manifest format (manifest.json)
//!
//! Defines the structure and validation of the manifest inside a .cvpack file.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
    /// Build status of the pack's model assets.
    ///
    /// `"placeholder"` means the declared model files are stand-ins (this is how
    /// Phase 1 shipped) and the pack therefore CANNOT synthesize audio. Anything
    /// else — including `None` — is treated as a real, release-intended pack and
    /// is held to the full provenance requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
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
    /// The tokenizer table whose ids the graph was built against (`tokenizer.json`, ~4 KB).
    ///
    /// Declared because it is load-bearing: the 115-symbol vocab decides which phonemes actually
    /// reach the graph, so a pack without it cannot be re-verified after a model swap.
    #[serde(rename = "tokenizer")]
    Tokenizer,
    /// A 510 x 256 float32 style matrix — i.e. a *persona*, 522,240 bytes, exactly.
    ///
    /// This is the slot that makes "ship me a new voice" a 522 KB asset instead of an 88 MB model
    /// (docs/research/KOKORO_OFFLINE_SPIKE.md). The size is validated, not assumed: a truncated
    /// style vector reads as a voice with fewer rows and produces garbage speech silently.
    #[serde(rename = "style_vector")]
    StyleVector,
}

/// One persona's style source. Kokoro-class engines take one 256-float row per utterance, so a
/// persona is fully described by *where that matrix comes from*.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StyleConfig {
    /// A single stock voice id (e.g. `af_heart`). The engine resolves it from its own data dir, so a
    /// pack using this form is NOT self-contained: it names an asset it does not carry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_voice: Option<String>,

    /// A weighted blend of stock voices, i.e. the recipe form produced by
    /// `scripts/derive-persona-style.py`. Recorded so a persona stays auditable and re-derivable
    /// instead of being an opaque binary; the vectors themselves may be unshippable (derivative work).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blend: Vec<StyleWeight>,

    /// Path, within this pack, of a `style_vector` file. The only portable form: nothing to resolve,
    /// nothing to re-derive at install time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedded_file: Option<String>,
}

/// One term of a [`StyleConfig::blend`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleWeight {
    pub voice: String,
    pub weight: f32,
}

/// A persona's declared chunking policy -- the numbers `vocal_core::utterance_plan::PlanPolicy` takes.
///
/// Only coherence is checked here. The *bounds* depend on the model's token window, which belongs to the
/// engine; restating 510 in the pack format would make a second source of truth for a number
/// `vocal-core` already measures, and two sources of a table is how the last bug of this shape started.
/// `Persona::chunking_policy` is where a declared pair meets the real window.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkingConfig {
    /// Ceiling on content tokens per utterance, spaces between runs included.
    pub max_units: usize,
    /// Minimum size of a chunk: punctuation closes one at or above this, and a shorter tail folds back
    /// into its predecessor when that stays inside `max_units`.
    pub min_chunk_units: usize,
}

/// The loudness post-stage — the honest implementation of the specs' `Energy`, which is not an
/// engine input. Defaults are shared with `vocal_core::audio_levels` and a test pins that they agree.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LoudnessConfig {
    /// Target RMS in dBFS. Negative: 0 dBFS is full-scale RMS, which no speech should ask for.
    pub target_dbfs: f32,
    /// Hard cap on `|sample|` after gain, in float PCM units.
    pub peak_ceiling: f32,
    /// Cap on amplification only. Silence must not be raised out of the noise floor — a silent clip
    /// "normalised" to a loud target is how you get +147 dB of quantisation mush.
    pub max_gain_db: f32,
}

/// `vocal_core::audio_levels::DEFAULT_PEAK_CEILING`. Measured need: 8 of 54 stock voices exceed
/// 0.9 peak on one plain sentence and two clip at 1.000.
pub const DEFAULT_PEAK_CEILING: f32 = 0.98;
/// `vocal_core::audio_levels::DEFAULT_MAX_GAIN_DB`.
pub const DEFAULT_MAX_GAIN_DB: f32 = 12.0;
/// `vocal_core::audio_levels::LoudnessSpec::default().target_dbfs`.
pub const DEFAULT_LOUDNESS_TARGET_DBFS: f32 = -20.0;
/// A style vector is 510 rows of 256 float32: this exact number, no more, no less.
pub const STYLE_VECTOR_BYTES: u64 = 510 * 256 * 4;

impl Default for LoudnessConfig {
    fn default() -> Self {
        Self {
            target_dbfs: DEFAULT_LOUDNESS_TARGET_DBFS,
            peak_ceiling: DEFAULT_PEAK_CEILING,
            max_gain_db: DEFAULT_MAX_GAIN_DB,
        }
    }
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
    /// Speech rate, mapped onto the engine's `speed` scalar (1.0 = neutral). The only prosody value
    /// a Kokoro-class graph accepts, which is why the allowed range is the graph's, not a taste.
    pub default_rate: f32,
    /// Pitch **offset** in the persona specs' units (-0.5..+0.5), *not* a multiplier: `vocal-core`'s
    /// `Persona::default_pitch` used to document "1.0 = normal" for the same field, and the two
    /// crates disagreed about what a pack number meant. Offset semantics win, because that is what
    /// `docs/personas/*.md` and the PRD specify.
    ///
    /// There is no pitch input in the engine, so a non-zero value here is a statement about the
    /// style vector and must be declared as such via `pitch_baked_into_style`; the validator rejects
    /// any other combination rather than letting a pack promise a dial that will do nothing.
    pub default_pitch: f32,
    /// True when `default_pitch` is already realised in the persona's style vector (i.e. the cast
    /// was chosen for that register) rather than applied at synthesis time.
    #[serde(default)]
    pub pitch_baked_into_style: bool,
    /// Where the persona's 510 x 256 style matrix comes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<StyleConfig>,
    /// How the engine draws utterance boundaries for this persona.
    ///
    /// Declared rather than inherited from a default, because the style row is selected by *utterance
    /// length*: two builds that chunk one sentence differently read different rows, so a persona whose
    /// measurements were taken inside a single chunk stops being that persona once something else splits
    /// it. Absent means "whatever the engine defaults to" -- honest for a pack built before the slot
    /// existed, and a claim a pack citing measurements should not leave unstated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunking: Option<ChunkingConfig>,
    /// Loudness post-stage (the specs' `Energy`). Absent means the engine default, not "off": a
    /// release pack that ships a persona should say what it wants and be held to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loudness: Option<LoudnessConfig>,
    /// Word -> phoneme string, for proper nouns the graph2pilot cannot guess ("Chiti" came out as
    /// `tʃˈaːɾi`). Applied before tokenisation, so encodability is checked in
    /// `vocal-core` where the table lives: `Persona::check_overrides_encodable`, run by the CLI's
    /// `verify` and against every tracked pack in CI. This crate deliberately does not check it — it has
    /// no view of the 115 symbols, and inventing a second copy here is how two sources of truth start.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub pronunciation_overrides: HashMap<String, String>,
    /// Intent profiles
    pub intent_profiles: HashMap<String, IntentProfile>,
}

/// Intent-specific prosody
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentProfile {
    /// Rate multiplier for this intent, on the engine's `speed` scale (see
    /// `PersonaConfig::default_rate` for the range and why).
    pub rate: f32,
    /// Per-intent pitch is **not implementable**: the graph has no pitch input and the style row is
    /// chosen by utterance length, not by intent. The field stays for spec compatibility and is
    /// required to be 0.0 by the validator, rather than silently ignored at synthesis time.
    pub pitch: f32,
    /// Energy 0.0-1.0, honoured only as a loudness target by the runtime's post-stage. It changes
    /// level, not articulation, and `docs/research/PERSONA_STYLE_VECTORS.md` says so plainly.
    pub energy: f32,
    /// Pause duration multiplier applied by the runtime's pause stage (silence insertion), not the
    /// model.
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
    /// What the pack author needs the next person to know, in the pack's own words: what is
    /// unverified, what must be checked before release.
    ///
    /// This is not decoration. The shipped packs have always written a `notes` field carrying the
    /// licence warning (Piper's phonemiser is GPL, voice models are licensed per model), and serde
    /// dropped it on load because no such field existed — so the one sentence a release manager most
    /// needed was invisible exactly where it mattered: in anything that loads a pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl PackManifest {
    /// True when this pack ships placeholder assets instead of a real model.
    pub fn is_placeholder(&self) -> bool {
        matches!(self.status.as_deref(), Some("placeholder"))
    }

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

        self.validate_persona()?;
        Ok(())
    }

    /// Validate the persona block: numeric ranges, the style source, and the claims a
    /// Kokoro-class engine cannot honour.
    ///
    /// These rules exist because the persona specs were written against a runtime with five
    /// prosody dials, and the model this repo measured has one (`speed`). Rather than accepting any
    /// number and discarding it later, the manifest is the place where "this pack promises
    /// something the engine cannot do" becomes a load-time error with a reason.
    pub fn validate_persona(&self) -> Result<(), String> {
        let Some(persona) = &self.persona else {
            return Ok(());
        };

        if persona.id.trim().is_empty() {
            return Err("persona.id must not be empty".to_string());
        }
        if !persona.default_rate.is_finite()
            || !(PERSONA_RATE_MIN..=PERSONA_RATE_MAX).contains(&persona.default_rate)
        {
            return Err(format!(
                "persona.default_rate must be within {PERSONA_RATE_MIN}..={PERSONA_RATE_MAX} (the engine's usable `speed` range, measured); got {}",
                persona.default_rate
            ));
        }
        if !persona.default_pitch.is_finite() || persona.default_pitch.abs() > 0.5 {
            return Err(format!(
                "persona.default_pitch is an offset in -0.5..=0.5, got {} (a multiplier such as 1.0 is a unit mix-up, not a pitch)",
                persona.default_pitch
            ));
        }
        if persona.default_pitch != 0.0 && !persona.pitch_baked_into_style {
            return Err(format!(
                "persona.default_pitch is {}, but the engine has no pitch input: the only honest places for a pitch offset are the style vector (set `pitch_baked_into_style: true` when the cast was chosen for that register) or 0.0",
                persona.default_pitch
            ));
        }

        for (name, profile) in &persona.intent_profiles {
            if !profile.rate.is_finite()
                || !(PERSONA_RATE_MIN..=PERSONA_RATE_MAX).contains(&profile.rate)
            {
                return Err(format!(
                    "persona.intent_profiles.{name}.rate must be within {PERSONA_RATE_MIN}..={PERSONA_RATE_MAX}; got {}",
                    profile.rate
                ));
            }
            if profile.pitch != 0.0 {
                return Err(format!(
                    "persona.intent_profiles.{name}.pitch is {}; per-intent pitch cannot be honoured (no pitch input, and the style row follows utterance length), so it must be 0.0 rather than silently dropped",
                    profile.pitch
                ));
            }
            if !profile.energy.is_finite() || !(0.0..=1.0).contains(&profile.energy) {
                return Err(format!(
                    "persona.intent_profiles.{name}.energy must be 0.0..=1.0; got {}",
                    profile.energy
                ));
            }
            if !profile.pause_factor.is_finite()
                || !(PAUSE_FACTOR_MIN..=PAUSE_FACTOR_MAX).contains(&profile.pause_factor)
            {
                return Err(format!(
                    "persona.intent_profiles.{name}.pause_factor must be within {PAUSE_FACTOR_MIN}..={PAUSE_FACTOR_MAX}; got {}",
                    profile.pause_factor
                ));
            }
        }

        self.validate_style(persona)?;

        if let Some(loudness) = &persona.loudness {
            if !loudness.target_dbfs.is_finite()
                || !(LOUDNESS_TARGET_MIN_DBFS..=LOUDNESS_TARGET_MAX_DBFS).contains(&loudness.target_dbfs)
            {
                return Err(format!(
                    "persona.loudness.target_dbfs must be within {LOUDNESS_TARGET_MIN_DBFS}..={LOUDNESS_TARGET_MAX_DBFS} (0 dBFS RMS is full-scale distortion on a small speaker); got {}",
                    loudness.target_dbfs
                ));
            }
            if !loudness.peak_ceiling.is_finite()
                || !(0.1..=PEAK_CEILING_MAX).contains(&loudness.peak_ceiling)
            {
                return Err(format!(
                    "persona.loudness.peak_ceiling must be within 0.1..={PEAK_CEILING_MAX}; a ceiling of 1.0 admits the clipping the 54-voice survey measured in 2 voices, got {}",
                    loudness.peak_ceiling
                ));
            }
            if !loudness.max_gain_db.is_finite()
                || !(0.0..=MAX_GAIN_DB_LIMIT).contains(&loudness.max_gain_db)
            {
                return Err(format!(
                    "persona.loudness.max_gain_db must be within 0.0..={MAX_GAIN_DB_LIMIT}; got {}",
                    loudness.max_gain_db
                ));
            }
        }

        for (word, phonemes) in &persona.pronunciation_overrides {
            if word.trim().is_empty() || word.split_whitespace().count() != 1 {
                return Err(format!(
                    "pronunciation_overrides key {word:?} must be a single non-empty word: matching is per-token before tokenisation, so multi-word keys would never hit"
                ));
            }
            if word.len() > 64 || phonemes.trim().is_empty() || phonemes.len() > 256 {
                return Err(format!(
                    "pronunciation_overrides[{word:?}] is malformed (value {} bytes, limit 256; key limit 64)",
                    phonemes.len()
                ));
            }
        }

        if let Some(chunking) = &persona.chunking {
            // Both numbers are token budgets, so 0 is not "unspecified" here -- it is a policy under
            // which no utterance can exist. Reject it rather than let a default swallow the typo.
            if chunking.max_units == 0 || chunking.min_chunk_units == 0 {
                return Err(format!(
                    "persona.chunking counts are token budgets and must be >= 1: max_units {}, \
                     min_chunk_units {}",
                    chunking.max_units, chunking.min_chunk_units
                ));
            }
            if chunking.min_chunk_units > chunking.max_units {
                return Err(format!(
                    "persona.chunking.min_chunk_units {} exceeds max_units {}: the fold rule would then \
                     merge every chunk into its predecessor, and punctuation could never close one",
                    chunking.min_chunk_units, chunking.max_units
                ));
            }
        }

        Ok(())
    }

    /// Exactly one style source, weights that sum, and — if the vector ships inside the pack — a
    /// declared file of the right type and the right size to the byte.
    fn validate_style(&self, persona: &PersonaConfig) -> Result<(), String> {
        let Some(style) = &persona.style else {
            if self.is_placeholder() {
                return Ok(());
            }
            return Err(
                "a release pack must declare persona.style (source_voice, blend, or embedded_file): without it the engine has no idea which 256 floats make this persona, and \"the default voice\" is how a persona silently becomes somebody else"
                    .to_string(),
            );
        };

        let chosen = [
            style.source_voice.is_some(),
            !style.blend.is_empty(),
            style.embedded_file.is_some(),
        ];
        if chosen.iter().filter(|set| **set).count() != 1 {
            return Err(
                "persona.style must set exactly one of source_voice, blend, embedded_file (they mean different things about portability and licence, so combining them is not a merge)".to_string(),
            );
        }

        if let Some(voice) = &style.source_voice {
            if voice.trim().is_empty() {
                return Err("persona.style.source_voice must not be blank".to_string());
            }
        }

        if !style.blend.is_empty() {
            if style.blend.len() > BLEND_MAX_TERMS {
                return Err(format!(
                    "persona.style.blend has {} terms; more than {BLEND_MAX_TERMS} is averaging toward the mean of everything, which the survey shows flattens prosody",
                    style.blend.len()
                ));
            }
            let mut seen: HashSet<&str> = HashSet::new();
            let mut sum = 0.0f64;
            for term in &style.blend {
                if term.voice.trim().is_empty() {
                    return Err("persona.style.blend entry has an empty voice id".to_string());
                }
                if !seen.insert(term.voice.as_str()) {
                    return Err(format!(
                        "persona.style.blend lists {:?} twice; a duplicated term is a weight bug pretending to be a voice",
                        term.voice
                    ));
                }
                if !term.weight.is_finite() || !(0.0..=1.0).contains(&term.weight) {
                    return Err(format!(
                        "persona.style.blend weight for {:?} is {}, expected 0.0..=1.0",
                        term.voice, term.weight
                    ));
                }
                sum += f64::from(term.weight);
            }
            if (sum - 1.0).abs() > f64::from(BLEND_WEIGHT_SUM_TOLERANCE) {
                return Err(format!(
                    "persona.style.blend weights sum to {sum:.6}, expected 1.0: the blend is an interpolation of style rows, so a sum above or below 1 changes loudness and register by accident (re-derive with scripts/derive-persona-style.py, which normalises and records the weights it used)"
                ));
            }
        }

        if let Some(path) = &style.embedded_file {
            let entry = self
                .files
                .iter()
                .find(|declared| &declared.path == path)
                .ok_or_else(|| {
                    format!(
                        "persona.style.embedded_file {path:?} is not declared in `files`; undeclared archive entries are rejected by the loader, so this would be a persona that resolves to nothing"
                    )
                })?;
            if entry.file_type != FileType::StyleVector {
                return Err(format!(
                    "persona.style.embedded_file {path:?} is declared as {:?}, not style_vector",
                    entry.file_type
                ));
            }
            if entry.size_bytes != STYLE_VECTOR_BYTES {
                return Err(format!(
                    "persona.style.embedded_file {path:?} declares {} bytes, expected {STYLE_VECTOR_BYTES} (510 rows x 256 x f32); a short style vector would read as a voice with fewer rows and mispronounce nothing — it would just speak as someone else",
                    entry.size_bytes
                ));
            }
        }

        Ok(())
    }
}

/// The engine's measured working range for `speed`. 2.0 synthesises and 0.5 stretches, but both are
/// unusable as speech (docs/research/KOKORO_OFFLINE_SPIKE.md), so a pack asking for them is a bug
/// in the pack rather than a feature.
pub const PERSONA_RATE_MIN: f32 = 0.5;
/// See [`PERSONA_RATE_MIN`].
pub const PERSONA_RATE_MAX: f32 = 1.6;
/// Pause scaling outside this band is either inaudible or a stutter.
pub const PAUSE_FACTOR_MIN: f32 = 0.5;
/// See [`PAUSE_FACTOR_MIN`].
pub const PAUSE_FACTOR_MAX: f32 = 3.0;
pub const LOUDNESS_TARGET_MIN_DBFS: f32 = -40.0;
pub const LOUDNESS_TARGET_MAX_DBFS: f32 = -6.0;
/// Below 1.0 on purpose: at 1.0 the ceiling is not a ceiling.
pub const PEAK_CEILING_MAX: f32 = 0.999;
pub const MAX_GAIN_DB_LIMIT: f32 = 24.0;
/// Blending past this many terms is averaging, not casting (see PERSONA_STYLE_VECTORS.md §4).
pub const BLEND_MAX_TERMS: usize = 8;
/// Weights must sum to 1 within this tolerance; the Python generator normalises, so a pack that
/// fails this was edited by hand after the fact.
pub const BLEND_WEIGHT_SUM_TOLERANCE: f32 = 1e-3;

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
            status: None,
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
            status: None,
        };

        assert!(manifest.validate().is_err());
    }
}
