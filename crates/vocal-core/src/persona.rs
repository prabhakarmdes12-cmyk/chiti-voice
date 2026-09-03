//! Persona runtime: pack data in, engine parameters out.
//!
//! ## The one-paragraph truth
//!
//! A `.cvpack` persona (`voice_pack::PersonaConfig`) declares five prosody numbers. The Kokoro-class
//! engine measured in `docs/research/PERSONA_STYLE_VECTORS.md` accepts **one** of them: `speed`.
//! Pitch has no input, energy has no input, "warmth" has no input, and per-intent pitch cannot be
//! approximated even by casting, because casting is a build-time choice of one vector for the whole
//! persona.
//!
//! So this module does not carry a `pitch` field on [`Prosody`]. That is deliberate: the type is
//! where the finding is enforced. A pack that claims a pitch offset without saying it is baked into
//! the style vector is rejected by the pack validator at load time, and a caller that wants "raise
//! the pitch for this intent" finds no method to call, rather than a method that quietly ignores it.
//!
//! What *is* honoured, in order of how real it is:
//! * `speed` — the graph input, exact.
//! * `loudness` — a float64 gain stage in this crate (`audio_levels`), which is the honest
//!   approximation of the specs' `Energy`.
//! * `pause_factor` — runtime silence insertion, a post-stage the pipeline owns, not the model.
//! * `pronunciation_overrides` — pre-tokenisation word substitution, the only way to say "Chiti".

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
    /// Speech rate, i.e. the engine's `speed` scalar (1.0 = neutral). **Offset-vs-multiplier note:**
    /// `voice_pack::PersonaConfig::default_pitch` documents pitch as an offset in -0.5..=0.5, and
    /// the shipped packs used to say `1.0` there, meaning "multiplier, neutral". That contradiction
    /// is resolved in favour of the specs (offset), and the pack validator rejects the multiplier
    /// reading rather than letting both survive in different crates.
    pub default_rate: f32,
    /// Pitch offset in the specs' units. Present for round-tripping a pack, *not* for synthesis:
    /// see [`Prosody`], which has no pitch field, and [`Persona::pitch_is_honoured`] for what a
    /// non-zero value is allowed to mean.
    pub default_pitch: f32,
    /// True when the pitch offset is already realised in the persona's style vector.
    #[serde(default)]
    pub pitch_baked_into_style: bool,
    /// Loudness post-stage; `None` means the crate defaults.
    #[serde(default)]
    pub loudness: Option<voice_pack::LoudnessConfig>,
    /// Word -> phonemes, applied before tokenisation (see the module docs).
    #[serde(default)]
    pub pronunciation_overrides: std::collections::HashMap<String, String>,
    /// Intent-to-prosody mappings
    pub intent_profiles: std::collections::HashMap<String, IntentProfile>,
}

impl Persona {
    /// Bridge from the pack's declared persona. The only supported way to build one, so the fields
    /// the engine can honour and the fields it cannot stay distinguishable at the type level.
    pub fn from_pack(config: &voice_pack::PersonaConfig) -> Self {
        Self {
            id: config.id.clone(),
            display_name: config.display_name.clone(),
            description: config.description.clone(),
            default_rate: config.default_rate,
            default_pitch: config.default_pitch,
            pitch_baked_into_style: config.pitch_baked_into_style,
            loudness: config.loudness,
            pronunciation_overrides: config.pronunciation_overrides.clone(),
            intent_profiles: config
                .intent_profiles
                .iter()
                .map(|(name, profile)| {
                    (
                        name.clone(),
                        IntentProfile {
                            rate: profile.rate,
                            pitch: profile.pitch,
                            energy: profile.energy,
                            pause_factor: profile.pause_factor,
                        },
                    )
                })
                .collect(),
        }
    }

    /// Refuse a persona whose own pronunciation fixes this engine cannot spell.
    ///
    /// `pronunciation_overrides` exists for the words graph2pilot gets wrong -- the product's own name is
    /// the reason -- so a value the tokenizer table cannot represent is not a soft edge case: the engine
    /// would emit a pad token exactly where the pack asserted a sound, which is the failure the field was
    /// added to prevent. `voice-pack`'s validator cannot catch this (it has no view of the table, by
    /// design -- IPA validity is the tokenizer's business), and `docs`/`README` claimed the values "are
    /// checked for encodability in `vocal-core`": this is that check, so the claim is true and the load
    /// path has one place to call it from.
    #[must_use]
    pub fn check_overrides_encodable(&self) -> crate::error::VoiceResult<()> {
        let mut offenders: Vec<(String, Vec<char>)> = Vec::new();
        for (word, phonemes) in &self.pronunciation_overrides {
            let mut unmapped = crate::phoneme_tokens::unmapped_symbols(phonemes);
            if !unmapped.is_empty() {
                unmapped.sort();
                unmapped.dedup();
                offenders.push((word.clone(), unmapped));
            }
        }
        if offenders.is_empty() {
            return Ok(());
        }
        offenders.sort();
        let detail = offenders
            .iter()
            .map(|(word, chars)| {
                let listed = chars
                    .iter()
                    .map(|c| format!("{c:?} (U+{:04X})", u32::from(*c)))
                    .collect::<Vec<String>>()
                    .join(", ");
                let hint = if chars.contains(&'g') {
                    " -- this table spells /g/ as script-g U+0261, which is what espeak-style IPA must be mapped to"
                } else {
                    ""
                };
                format!("{word:?} uses {listed}{hint}")
            })
            .collect::<Vec<String>>()
            .join("; ");
        Err(crate::error::VoiceError::new(
            crate::error::VoiceErrorCode::PackInvalidFormat,
            format!(
                "persona {:?} declares pronunciation_overrides this engine cannot spell: {detail}. \
                 The graph would synthesize a pad token in its place, silently replacing the sound the \
                 override exists to fix.",
                self.id
            ),
        ))
    }

    /// What a non-zero `default_pitch` is allowed to mean here. The pack validator enforces the same
    /// rule; this exists so the engine can assert it once at load time with a message a user reads.
    pub fn pitch_is_honoured(&self) -> bool {
        self.default_pitch == 0.0 || self.pitch_baked_into_style
    }

    /// The pack's loudness intent as a crate-level spec, or the defaults when it declares none.
    pub fn loudness_spec(&self) -> crate::audio_levels::LoudnessSpec {
        match self.loudness {
            Some(l) => crate::audio_levels::LoudnessSpec {
                target_dbfs: l.target_dbfs,
                peak_ceiling: l.peak_ceiling,
                max_gain_db: l.max_gain_db,
            },
            None => crate::audio_levels::LoudnessSpec::default(),
        }
    }
}

/// Prosody parameters for a specific intent, as declared by the pack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentProfile {
    /// Rate multiplier for this intent, on the engine's `speed` scale.
    pub rate: f32,
    /// Must be 0.0 for any pack that loads (validator-enforced). Kept so a pack can be reported on
    /// honestly instead of having the claim dropped at parse time.
    pub pitch: f32,
    /// 0.0-1.0, mapped onto the loudness target by [`crate::audio_levels`]. Level, not articulation.
    pub energy: f32,
    /// Pause duration multiplier, applied by the runtime's pause stage.
    pub pause_factor: f32,
}

/// What a persona actually asks the synthesis path to do.
///
/// Three fields, because three is what can be honoured: `speed` reaches the graph, `loudness_target`
/// reaches the gain stage, `pause_factor` reaches the silence inserter. Pitch is absent by design —
/// see the module docs.
// No `Copy`: `intent` is a String, and the point of carrying it is to hand a log a name, not to be
// cheap to pass around. (Deriving `Copy` here was E0204: `Option<String>` is not `Copy`.)
#[derive(Debug, Clone, PartialEq)]
pub struct Prosody {
    /// The graph's `speed` scalar.
    pub speed: f32,
    /// Loudness target for this utterance, dBFS. `None` = the crate's default spec.
    pub loudness_target_dbfs: Option<f32>,
    /// Multiplier on inter-clause silence.
    pub pause_factor: f32,
    /// Which intent this reflects, for logging: an engine run should say *why* it sounds like that.
    pub intent: Option<String>,
}

impl Prosody {
    /// The loudness spec to run with, filling in the persona's declared limits.
    pub fn loudness_spec_with(&self, base: crate::audio_levels::LoudnessSpec) -> crate::audio_levels::LoudnessSpec {
        crate::audio_levels::LoudnessSpec {
            target_dbfs: self.loudness_target_dbfs.unwrap_or(base.target_dbfs),
            ..base
        }
    }
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

    /// Resolve what the synthesis path should do for a persona + intent.
    ///
    /// `None` only when the persona is unknown. An unknown *intent* is not an error: it falls back
    /// to the persona defaults and says so through [`Prosody::intent`] being `None`, which is the
    /// behaviour a hands-free product needs (a mis-typed intent must not silence the device) while
    /// still being inspectable by callers and logs.
    pub fn prosody(&self, persona_id: &str, intent: Option<&str>) -> Option<Prosody> {
        let persona = self.get_persona(persona_id)?;
        let profile = intent.and_then(|name| persona.intent_profiles.get(name));

        // 0.5 is neutral: with no intent (or an unknown one) the persona's declared target must
        // reach the gain stage untouched. `map_or(1.0, ..)` here asked for +6 dB on every plain
        // sentence, which the persona docs never claimed.
        let energy = profile.map_or(0.5, |p| p.energy);
        Some(Prosody {
            speed: profile.map_or(persona.default_rate, |p| p.rate),
            // Energy is a *relative* request, so it offsets the persona's declared target rather
            // than replacing it: 0.5 is neutral and the endpoints of the 0.0-1.0 range move the
            // target by 6 dB either way (12 dB of swing). That mapping is documented in
            // PERSONA_STYLE_VECTORS.md because it is an approximation the specs never had to
            // justify, and it is the whole implementation of `Energy` until something better exists.
            loudness_target_dbfs: persona
                .loudness
                .map(|l| l.target_dbfs + (energy - 0.5) * 12.0),
            pause_factor: profile.map_or(1.0, |p| p.pause_factor),
            intent: intent.map(str::to_owned),
        })
    }

    /// Look up a declared pronunciation for a word, if the pack has one.
    ///
    /// Case-insensitive on the key because the matching happens on raw text before tokenisation and
    /// `Chiti` at the start of a sentence is not the same token as `chiti` inside one.
    pub fn pronunciation(&self, persona_id: &str, word: &str) -> Option<&str> {
        let persona = self.get_persona(persona_id)?;
        persona
            .pronunciation_overrides
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(word))
            .map(|(_, value)| value.as_str())
    }
}

impl PersonaRuntime {
    /// The persona's declared loudness spec, or the crate defaults when it declares none (and for an
    /// unknown persona, which is what makes this safe to call before validating the id).
    #[must_use]
    pub fn loudness_of(&self, persona_id: &str) -> crate::audio_levels::LoudnessSpec {
        self.get_persona(persona_id).map_or_else(
            crate::audio_levels::LoudnessSpec::default,
            Persona::loudness_spec,
        )
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
    use voice_pack::{LoudnessConfig, PersonaConfig};
    use std::collections::HashMap;

    fn pack_persona() -> PersonaConfig {
        PersonaConfig {
            id: "tara".into(),
            display_name: "Tara".into(),
            description: "warm, professional".into(),
            default_rate: 1.0,
            default_pitch: 0.0,
            pitch_baked_into_style: false,
            style: None,
            loudness: Some(LoudnessConfig {
                target_dbfs: -21.0,
                peak_ceiling: 0.98,
                max_gain_db: 12.0,
            }),
            pronunciation_overrides: HashMap::from([("chiti".to_string(), "ˈtʃɪti".to_string())]),
            intent_profiles: HashMap::from([(
                "GREETING".to_string(),
                voice_pack::IntentProfile {
                    rate: 1.05,
                    pitch: 0.0,
                    energy: 0.65,
                    pause_factor: 1.0,
                },
            )]),
        }
    }

    fn runtime() -> PersonaRuntime {
        let mut r = PersonaRuntime::new();
        r.register_persona(Persona::from_pack(&pack_persona()));
        r
    }

    #[test]
    fn loudness_defaults_agree_across_crates() {
        // The pack format and the DSP module must not drift into two different "default voices":
        // one number, one place to change it, and a test that fails when they disagree.
        let pack = LoudnessConfig::default();
        let core = crate::audio_levels::LoudnessSpec::default();
        assert_eq!(pack.target_dbfs, core.target_dbfs);
        assert_eq!(pack.peak_ceiling, core.peak_ceiling);
        assert_eq!(pack.max_gain_db, core.max_gain_db);
    }

    #[test]
    fn from_pack_carries_every_field_the_engine_can_use() {
        let p = Persona::from_pack(&pack_persona());
        assert_eq!(p.id, "tara");
        assert_eq!(p.default_rate, 1.0);
        assert_eq!(p.default_pitch, 0.0);
        assert!(!p.pitch_baked_into_style);
        assert!(p.pitch_is_honoured(), "a 0.0 offset needs no exemption");
        assert_eq!(p.loudness_spec().target_dbfs, -21.0);
        assert_eq!(p.loudness_spec().peak_ceiling, 0.98);
        assert_eq!(p.pronunciation_overrides.len(), 1);
    }

    #[test]
    fn prosody_maps_the_honourable_subset_and_names_the_intent() {
        let r = runtime();
        let base = r.prosody("tara", None).expect("persona registered");
        assert_eq!(base.speed, 1.0);
        assert_eq!(base.pause_factor, 1.0);
        assert_eq!(base.intent, None);
        assert_eq!(base.loudness_target_dbfs, Some(-21.0));

        let greeting = r.prosody("tara", Some("GREETING")).expect("intent defined");
        assert_eq!(greeting.speed, 1.05);
        assert_eq!(greeting.intent.as_deref(), Some("GREETING"));
        // energy 0.65 on a 12 dB swing around the persona target: louder than neutral, not 2x.
        let expected = -21.0 + (0.65 - 0.5) * 12.0;
        assert!(
            (greeting.loudness_target_dbfs.unwrap() - expected).abs() < 1e-5,
            "energy mapping drifted: {:?} vs {expected}",
            greeting.loudness_target_dbfs
        );

        // Unknown intent: fall back, but keep reporting that we did.
        let unknown = r.prosody("tara", Some("TELEPHONY")).expect("still synthesises");
        assert_eq!(unknown.speed, 1.0);
        assert_eq!(unknown.intent.as_deref(), Some("TELEPHONY"));
        assert!(r.prosody("nobody", None).is_none(), "unknown persona is not silence");
    }

    #[test]
    fn prosody_loudness_spec_keeps_the_persona_limits() {
        let r = runtime();
        let base = r.loudness_of("tara");
        let prosody = r.prosody("tara", Some("GREETING")).unwrap();
        let applied = prosody.loudness_spec_with(base);
        assert_eq!(applied.peak_ceiling, base.peak_ceiling);
        assert_eq!(applied.max_gain_db, base.max_gain_db);
        assert!(applied.target_dbfs > base.target_dbfs);
    }

    #[test]
    fn pronunciations_are_case_insensitive_and_scoped_to_the_persona() {
        let r = runtime();
        assert_eq!(r.pronunciation("tara", "Chiti"), Some("ˈtʃɪti"));
        assert_eq!(r.pronunciation("tara", "chiti"), Some("ˈtʃɪti"));
        assert_eq!(r.pronunciation("tara", "Kashi"), None);
        assert_eq!(r.pronunciation("nope", "chiti"), None);
    }

    #[test]
    fn a_pitch_claim_without_an_owner_is_visible() {
        let mut config = pack_persona();
        config.default_pitch = -0.10;
        let p = Persona::from_pack(&config);
        assert!(!p.pitch_is_honoured(), "the pack validator is what rejects this, but the engine must be able to say the same thing");
        config.pitch_baked_into_style = true;
        assert!(Persona::from_pack(&config).pitch_is_honoured());
    }

    #[test]
    fn an_override_the_table_cannot_spell_is_refused_at_load() {
        // The shipped value is fine, and saying so keeps the rule from being "everything fails".
        let clean = Persona::from_pack(&pack_persona());
        clean.check_overrides_encodable().expect("shipped IPA is spellable");

        // ASCII 'g' is the trap: the table carries script-g (U+0261), so an espeak-style IPA string for
        // "go" would otherwise synthesize as "o" plus a pad -- the one thing a pronouncer exists to stop.
        let mut config = pack_persona();
        config
            .pronunciation_overrides
            .insert("go".to_string(), "go\u{028A}".to_string());
        let err = Persona::from_pack(&config)
            .check_overrides_encodable()
            .expect_err("an unspellable override must not load silently");
        let msg = err.to_string();
        assert_eq!(err.code(), crate::error::VoiceErrorCode::PackInvalidFormat);
        assert!(msg.contains("go"), "the message must name the word: {msg}");
        assert!(msg.contains("U+0261"), "and the replacement it should have used: {msg}");

        // More than one offender, reported together and in a stable order: a pack author fixing this
        // should see the whole list, not one entry per rebuild.
        let mut config = pack_persona();
        config
            .pronunciation_overrides
            .insert("go".to_string(), "go\u{028A}".to_string());
        config
            .pronunciation_overrides
            .insert("llan".to_string(), "\u{026C}an".to_string());
        let msg = Persona::from_pack(&config)
            .check_overrides_encodable()
            .expect_err("every offender, not the first one")
            .to_string();
        assert!(
            msg.contains("\"go\"") && msg.contains("\"llan\""),
            "the whole list, so an author fixes them in one pass: {msg}"
        );
        assert!(msg.contains("tara"), "and which persona is at fault: {msg}");
    }
}
