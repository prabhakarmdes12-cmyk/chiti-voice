//! The persona block is now load-bearing, so these tests are the contract: what a pack may claim,
//! and what gets rejected instead of silently ignored.
//!
//! Context worth having before reading the assertions: the persona specs (`docs/personas/*.md`)
//! describe five prosody parameters, and the engine measured in
//! `docs/research/PERSONA_STYLE_VECTORS.md` accepts one (`speed`). Rather than parsing the other four
//! and doing nothing with them, the manifest refuses the claims that have no implementation — so a
//! pack cannot promise a dial that will not turn.

use voice_pack::{FileType, PackLimits, PackManifest, PackValidator};

/// A minimal pack template with the persona block, file list and status spliced in.
///
/// The fixtures are JSON rather than struct literals on purpose: every new field carries
/// `#[serde(default)]`, and "a manifest written before these slots existed still parses" is part of
/// the compatibility claim being tested.
const TEMPLATE: &str = r#"{
  "schema_version": "1.0.0",
  "id": "test-voice",
  "name": "Test Voice",
  "version": "1.0.0",
  "author": "tests",
  "license": "MIT",
  "description": "fixture",
  "engine_family": "kokoro",
  "engine_version_min": "1.0.0",
  "supported_languages": ["en-IN"],
  "files": [@files@],
  "persona": @persona@@status@
}"#;

const PLACEHOLDER: &str = ", \"status\": \"placeholder\"";

/// The IPA string the three shipped packs declare for "Chiti", written as escapes so no transport
/// encoding can damage it. An earlier revision of this file carried the mojibake of exactly this value
/// (`\u{00cb}\u{02c6}...` where `\u{02c8}...` was meant), and nothing rejected it: the validator's rules
/// for an override are "single-word key, non-blank value under 256 bytes", because IPA validity is the
/// tokenizer table's business, not the manifest's. So a mangled value is *representable* — which is why
/// the test below asserts the round trip byte for byte instead of trusting the file.
const IPA_CHITI: &str = "@IPA@";

fn from_json(persona: &str, files: &str, status: &str) -> PackManifest {
    let json = TEMPLATE
        .replace("@persona@", persona)
        .replace("@files@", files)
        .replace("@status@", status);
    serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("fixture manifest must parse; if it does not, that is the finding: {e}\n{json}"))
}

fn pack(persona: &str, files: &str) -> PackManifest {
    from_json(persona, files, "")
}

/// The persona fields every fixture shares, plus whatever the case under test adds.
///
/// A case may re-declare a key it wants to override (e.g. `default_pitch`, or `style` itself);
/// serde's last-value-wins for duplicate keys is what makes the template composable without a builder
/// per field.
///
/// `style` is in the shared part because the validator reports it first: a fixture asserting that a
/// bad `loudness` block is rejected only proves the rule if the pack is otherwise loadable. Without it
/// every loudness/pronunciation case in this file failed on "declare persona.style" instead — which is
/// exactly what the local mirror of these rules caught, since CI reports an assertion failure as
/// nothing but a red job.
fn persona(inner: &str) -> String {
    format!(
        r#"{{ "id": "tara", "display_name": "Tara", "description": "warm, professional",
             "default_rate": 1.0, "default_pitch": 0.0, "intent_profiles": {{}},
             "style": {{ "source_voice": "af_heart" }}, {inner} }}"#
    )
}

fn persona_no_style(inner: &str) -> String {
    format!(
        r#"{{ "id": "tara", "display_name": "Tara", "description": "warm, professional",
             "default_rate": 1.0, "default_pitch": 0.0, "intent_profiles": {{}}{inner} }}"#
    )
}

const MODEL_ONLY: &str = r#"{ "path": "model.onnx", "checksum_sha256": "0000000000000000000000000000000000000000000000000000000000000000", "size_bytes": 10, "file_type": "model" }"#;

const WITH_STYLE_FILE: &str = r#"{ "path": "model.onnx", "checksum_sha256": "0000000000000000000000000000000000000000000000000000000000000000", "size_bytes": 10, "file_type": "model" },
  { "path": "persona.bin", "checksum_sha256": "1111111111111111111111111111111111111111111111111111111111111111", "size_bytes": 522240, "file_type": "style_vector" }"#;

fn err_of(persona: String, files: &str) -> String {
    pack(&persona, files)
        .validate_persona()
        .expect_err("this fixture is meant to be rejected")
}

#[test]
fn a_pre_persona_manifest_still_parses_and_validates() {
    // Compatibility, actually tested: packs built before the style/loudness/override slots existed
    // must not become unreadable. `"status": "placeholder"` is what exempts them from declaring a
    // style source, so the exemption is asserted here rather than discovered at a release build.
    let manifest = from_json(&persona_no_style(", \"pitch_baked_into_style\": false"), MODEL_ONLY, PLACEHOLDER);
    assert!(manifest.is_placeholder());
    let persona = manifest.persona.as_ref().expect("persona parsed");
    assert!(persona.style.is_none(), "absent, not defaulted to something");
    assert!(persona.loudness.is_none());
    assert!(persona.pronunciation_overrides.is_empty());
    manifest
        .validate_persona()
        .expect("a placeholder persona with no style source is valid");

    // And the same JSON without the placeholder marker is what fails: the rule, not the parse.
    let as_release = from_json(&persona_no_style(", \"pitch_baked_into_style\": false"), MODEL_ONLY, "");
    let err = as_release.validate_persona().unwrap_err();
    assert!(err.contains("persona.style"), "{err}");
}

#[test]
fn a_release_pack_must_say_where_its_persona_style_comes_from() {
    let manifest = pack(&persona_no_style(""), MODEL_ONLY);
    let err = manifest.validate_persona().unwrap_err();
    assert!(
        err.contains("persona.style"),
        "the message must name the missing slot: {err}"
    );

    pack(&persona(r#""style": { "source_voice": "af_heart" }"#), MODEL_ONLY)
        .validate_persona()
        .expect("a single stock voice is a legitimate cast");
}

#[test]
fn blend_weights_must_sum_to_one_and_must_not_repeat_a_voice() {
    let tara = persona(
        r#""style": { "blend": [ {"voice": "af_bella", "weight": 0.40},
                                  {"voice": "af_heart", "weight": 0.35},
                                  {"voice": "af_aoede", "weight": 0.25} ] }"#,
    );
    pack(&tara, MODEL_ONLY)
        .validate_persona()
        .expect("the recipe as generated by derive-persona-style.py must validate verbatim");

    let drifted = persona(
        r#""style": { "blend": [ {"voice": "af_bella", "weight": 0.40},
                                  {"voice": "af_heart", "weight": 0.40} ] }"#,
    );
    let err = err_of(drifted, MODEL_ONLY);
    assert!(err.contains("sum to"), "expected the sum rule, got: {err}");

    let doubled = persona(
        r#""style": { "blend": [ {"voice": "af_heart", "weight": 0.5},
                                  {"voice": "af_heart", "weight": 0.5} ] }"#,
    );
    let err = err_of(doubled, MODEL_ONLY);
    assert!(err.contains("twice"), "a duplicated voice is a weight bug: {err}");

    let out_of_range = persona(
        r#""style": { "blend": [ {"voice": "af_heart", "weight": 1.5},
                                  {"voice": "af_bella", "weight": -0.5} ] }"#,
    );
    assert!(err_of(out_of_range, MODEL_ONLY).contains("0.0..=1.0"));

    let nine = (0..9)
        .map(|i| format!(r#"{{"voice": "v{i}", "weight": {:.5}}}"#, 1.0_f32 / 9.0))
        .collect::<Vec<_>>()
        .join(", ");
    let bloated = persona(&format!(r#""style": {{ "blend": [ {nine} ] }}"#));
    let err = err_of(bloated, MODEL_ONLY);
    assert!(
        err.contains("terms"),
        "nine terms is averaging toward everyone, not casting: {err}"
    );
}

#[test]
fn style_sources_are_mutually_exclusive() {
    let both = persona(
        r#""style": { "source_voice": "af_heart", "embedded_file": "persona.bin" }"#,
    );
    let err = err_of(both, WITH_STYLE_FILE);
    assert!(
        err.contains("exactly one"),
        "choosing two sources is not a merge: {err}"
    );
}

#[test]
fn an_embedded_style_vector_is_checked_against_the_file_list_to_the_byte() {
    let declared = persona(r#""style": { "embedded_file": "persona.bin" }"#);
    pack(&declared, WITH_STYLE_FILE)
        .validate_persona()
        .expect("declared, right type, right size");

    let missing = persona(r#""style": { "embedded_file": "absent.bin" }"#);
    assert!(err_of(missing, WITH_STYLE_FILE).contains("not declared"));

    let wrong_type = persona(r#""style": { "embedded_file": "model.onnx" }"#);
    assert!(err_of(wrong_type, WITH_STYLE_FILE).contains("not style_vector"));

    // One byte short: the file would "load" as a voice with fewer rows and speak as someone else.
    let truncated = WITH_STYLE_FILE.replace("522240", "522239");
    let err = err_of(declared, &truncated);
    assert!(
        err.contains("522240") && err.contains("bytes"),
        "the size rule must quote both numbers: {err}"
    );
}

#[test]
fn unimplementable_prosody_claims_are_rejected_by_name() {
    let pitch_no_owner = persona(r#""default_pitch": -0.10"#);
    let err = err_of(pitch_no_owner, MODEL_ONLY);
    assert!(
        err.contains("no pitch input"),
        "the rejection must explain the engine limit, not just the range: {err}"
    );

    let pitch_baked = persona(r#""default_pitch": -0.10, "pitch_baked_into_style": true"#);
    pack(&pitch_baked, MODEL_ONLY)
        .validate_persona()
        .expect("a register realised by the cast is legitimate, and says so");

    // The three shipped packs wrote `1.0` here meaning "multiplier, neutral". That reading is now
    // refused, which is the point: two crates disagreed about the units of the same field.
    let multiplier = persona(r#""default_pitch": 1.0"#);
    let err = err_of(multiplier, MODEL_ONLY);
    assert!(err.contains("unit mix-up"), "{err}");
}

#[test]
fn intent_profiles_may_change_rate_level_and_pauses_only() {
    let intent_pitch = persona(
        r#""intent_profiles": { "GREETING": {"rate": 1.05, "pitch": 1.05, "energy": 0.65, "pause_factor": 1.0} }"#,
    );
    assert!(err_of(intent_pitch, MODEL_ONLY).contains("per-intent pitch"));

    let intent_energy = persona(
        r#""intent_profiles": { "GREETING": {"rate": 1.05, "pitch": 0.0, "energy": 1.1, "pause_factor": 1.0} }"#,
    );
    assert!(
        err_of(intent_energy, MODEL_ONLY).contains("0.0..=1.0"),
        "energy is a 0-1 knob in the specs, not a multiplier as the packs had it"
    );

    let intent_rate = persona(
        r#""intent_profiles": { "GREETING": {"rate": 2.0, "pitch": 0.0, "energy": 0.65, "pause_factor": 1.0} }"#,
    );
    assert!(err_of(intent_rate, MODEL_ONLY).contains("0.5..=1.6"));

    let intent_pause = persona(
        r#""intent_profiles": { "GREETING": {"rate": 1.05, "pitch": 0.0, "energy": 0.65, "pause_factor": 9.0} }"#,
    );
    assert!(err_of(intent_pause, MODEL_ONLY).contains("pause_factor"));

    let tara_table = persona(
        r#""intent_profiles": { "GREETING": {"rate": 1.05, "pitch": 0.0, "energy": 0.65, "pause_factor": 1.0},
                                "WARNING": {"rate": 0.90, "pitch": 0.0, "energy": 0.45, "pause_factor": 1.15},
                                "CELEBRATION": {"rate": 1.10, "pitch": 0.0, "energy": 0.75, "pause_factor": 1.0} }"#,
    );
    pack(&tara_table, MODEL_ONLY)
        .validate_persona()
        .expect("rows from docs/personas/TARA.md, mapped onto the fields that exist");
}

#[test]
fn loudness_limits_are_bounded_by_what_a_speaker_can_take() {
    let too_loud = persona(r#""loudness": {"target_dbfs": 0.0, "peak_ceiling": 0.98, "max_gain_db": 12.0}"#);
    assert!(err_of(too_loud, MODEL_ONLY).contains("full-scale distortion"));

    let too_quiet = persona(r#""loudness": {"target_dbfs": -60.0, "peak_ceiling": 0.98, "max_gain_db": 12.0}"#);
    assert!(err_of(too_quiet, MODEL_ONLY).contains("target_dbfs"));

    let no_ceiling = persona(r#""loudness": {"target_dbfs": -20.0, "peak_ceiling": 1.0, "max_gain_db": 12.0}"#);
    assert!(
        err_of(no_ceiling, MODEL_ONLY).contains("clipping"),
        "the ceiling exists because 2 of 54 stock voices clip on a plain sentence"
    );

    let unlimited_gain = persona(r#""loudness": {"target_dbfs": -20.0, "peak_ceiling": 0.98, "max_gain_db": 60.0}"#);
    assert!(err_of(unlimited_gain, MODEL_ONLY).contains("max_gain_db"));

    let tara = persona(r#""loudness": {"target_dbfs": -21.0, "peak_ceiling": 0.98, "max_gain_db": 12.0}"#);
    pack(&tara, MODEL_ONLY)
        .validate_persona()
        .expect("tara's measured settings");
}

#[test]
fn pronunciations_are_single_words_with_something_to_say() {
    let good = persona(&format!(r#""pronunciation_overrides": {{ "chiti": "{IPA_CHITI}" }}"#));
    let parsed = pack(&good, MODEL_ONLY);
    parsed
        .validate_persona()
        .expect("the override that fixes the product's own name");
    assert_eq!(
        parsed.persona.as_ref().expect("persona").pronunciation_overrides["chiti"],
        IPA_CHITI,
        "the IPA a pack declares must be the IPA the loader hands back, byte for byte"
    );

    let phrase = persona(r#""pronunciation_overrides": {"new delhi": "njuː dɛlhi"}"#);
    assert!(err_of(phrase, MODEL_ONLY).contains("single non-empty word"));

    let blank = persona(r#""pronunciation_overrides": {"chiti": "   "}"#);
    assert!(err_of(blank, MODEL_ONLY).contains("malformed"));
}

#[test]
fn the_validator_gate_runs_the_persona_rules_too() {
    // `validate_persona` would be decorative if the gate everyone actually calls never reached it.
    let manifest = pack(
        &persona(r#""default_pitch": 0.30, "pitch_baked_into_style": false"#),
        MODEL_ONLY,
    );
    let validator = PackValidator::with_limits(PackLimits::embedded());
    let err = validator
        .validate_manifest(&manifest)
        .expect_err("the persona must fail the real gate");
    assert!(
        err.starts_with("persona:"),
        "and it must be attributable to that stage: {err}"
    );

    // Same pack, claim withdrawn: the persona no longer blocks it (provenance still does, which is
    // the other gate's job and is asserted in pack_security.rs).
    let fixed = pack(&persona(r#""style": { "source_voice": "af_heart" }"#), MODEL_ONLY);
    let err = validator
        .validate_manifest(&fixed)
        .expect_err("a valid persona still needs provenance to load");
    assert!(
        !err.starts_with("persona:"),
        "the remaining complaint must not be about the persona: {err}"
    );
}

#[test]
fn the_new_file_types_parse_and_an_unknown_one_does_not() {
    let manifest = pack(
        &persona(r#""style": { "embedded_file": "persona.bin" }"#),
        WITH_STYLE_FILE,
    );
    let kinds: Vec<FileType> = manifest.files.iter().map(|f| f.file_type).collect();
    assert!(kinds.contains(&FileType::StyleVector), "style_vector parsed: {kinds:?}");
    assert!(!kinds.contains(&FileType::Tokenizer));

    let renamed = WITH_STYLE_FILE.replace("\"style_vector\"", "\"wav\"");
    let json = TEMPLATE
        .replace("@persona@", "null")
        .replace("@files@", &renamed)
        .replace("@status@", "");
    let parsed: Result<PackManifest, _> = serde_json::from_str(&json);
    assert!(
        parsed.is_err(),
        "an unknown file_type must fail to parse, not silently become Config: {parsed:?}"
    );
}

// ── TEMPORARY periscope ──────────────────────────────────────────────────────────────────────
// `Unit Tests` is red, and the job says only "process completed with exit code 101": an assertion
// failure produces no rustc diagnostic, so the wrapper channel that reports compile errors cannot see
// it, and Actions logs are unreachable from the sandbox this is developed in. But libtest replays a
// failing test's captured output into the step log verbatim, at column 0, and GitHub turns
// line-initial `::error::` text in that log into a check-run annotation.
//
// So this runs each test above inside `catch_unwind` and reports which one unwinds and with what
// payload -- which distinguishes the three things a single red tick conflates: a fixture whose JSON no
// longer matches the struct (a serde panic from `from_json`), a rule firing before the one the case
// targets, and an assertion whose expected substring the validator stopped writing. It calls the real
// test functions rather than re-deriving their fixtures, so it cannot drift from them, and it asserts
// nothing about the rules themselves.
//
// The hook is process-global, so with tests on several threads a payload can be attributed to the
// wrong name; the pass/fail verdicts come from `catch_unwind` and stay exact. Delete this whole block
// once `Unit Tests` is green -- a periscope left in a gate job is just another way to be red.
#[test]
fn ci_probe_which_test_fails() {
    let cases: [(&str, fn()); 11] = [
        ("a_pre_persona_manifest_still_parses_and_validates", a_pre_persona_manifest_still_parses_and_validates),
        ("a_release_pack_must_say_where_its_persona_style_comes_from", a_release_pack_must_say_where_its_persona_style_comes_from),
        ("blend_weights_must_sum_to_one_and_must_not_repeat_a_voice", blend_weights_must_sum_to_one_and_must_not_repeat_a_voice),
        ("style_sources_are_mutually_exclusive", style_sources_are_mutually_exclusive),
        ("an_embedded_style_vector_is_checked_against_the_file_list_to_the_byte", an_embedded_style_vector_is_checked_against_the_file_list_to_the_byte),
        ("unimplementable_prosody_claims_are_rejected_by_name", unimplementable_prosody_claims_are_rejected_by_name),
        ("intent_profiles_may_change_rate_level_and_pauses_only", intent_profiles_may_change_rate_level_and_pauses_only),
        ("loudness_limits_are_bounded_by_what_a_speaker_can_take", loudness_limits_are_bounded_by_what_a_speaker_can_take),
        ("pronunciations_are_single_words_with_something_to_say", pronunciations_are_single_words_with_something_to_say),
        ("the_validator_gate_runs_the_persona_rules_too", the_validator_gate_runs_the_persona_rules_too),
        ("the_new_file_types_parse_and_an_unknown_one_does_not", the_new_file_types_parse_and_an_unknown_one_does_not),
    ];
    let mut lines = Vec::new();
    let mut red = 0;
    for (name, body) in cases {
        let sink = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let hook_sink = std::sync::Arc::clone(&sink);
        std::panic::set_hook(Box::new(move |info| {
            if let Ok(mut slot) = hook_sink.lock() {
                *slot = info.to_string();
            }
        }));
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
        let _ = std::panic::take_hook();
        let mut payload = sink.lock().map(|slot| slot.clone()).unwrap_or_default();
        // One annotation per case: a payload that could carry its own line-initial marker must not be
        // able to fabricate a second one.
        payload = payload.replace('\n', " ").replace('\r', " ");
        payload = payload.chars().filter(|c| !c.is_control()).collect::<String>();
        match outcome {
            Ok(()) => lines.push(format!("::notice::PROBE pass  {name}")),
            Err(_) => {
                red += 1;
                lines.push(format!("::error::PROBE FAIL {name} :: {payload}"));
            }
        }
    }
    lines.push(format!("::notice::PROBE done {red} of 11 unwound"));
    assert_eq!(red, 0, "\n{}", lines.join("\n"));
}
