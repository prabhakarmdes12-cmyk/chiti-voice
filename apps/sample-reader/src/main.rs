//! A sample consumer project for Chiti Voice.
//!
//! This crate exists to be *wrong* in ways the in-crate tests cannot be. Every other test in this
//! workspace lives inside `crates/vocal-core` or `crates/voice-pack` and is compiled with access to
//! whatever those crates export, so "the public API is enough to build something" has never been
//! checked. Here it is: this file may only use `vocal_core::*` and `voice_pack::*`, and CI runs it as
//! a subprocess in `tests/integration.rs`, so an item that is `pub(crate)`, a type that cannot be
//! named from outside, or an error that loses its message fails the build.
//!
//! It also documents the boundary nobody should pretend away: **there is no text-to-phoneme converter
//! in Rust in this repository.** The engine's input is a phoneme string, so a consumer either brings
//! its own phonemiser (espeak-ng, or the permissive `open-phonemizer` path measured in
//! `docs/research/KOKORO_OFFLINE_SPIKE.md`) or speaks orthography to a build that cannot pronounce it.
//! The sample therefore takes phonemes on the command line and says so in its output.

use std::path::{Path, PathBuf};

use vocal_core::engine::mock::MockEngine;
use vocal_core::engine::VoiceEngine;
use vocal_core::phoneme_tokens::encode;
use vocal_core::persona::Persona;
use vocal_core::synthesis::{SynthesisFormat, SynthesisRequest};
use vocal_core::utterance_plan::{plan_pieces, Piece};
use voice_pack::{PackLoader, VoicePack};

type Exit = Result<(), Box<dyn std::error::Error + Send + Sync>>;

fn default_pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../voice-packs/dist/tara.cvpack")
}

fn usage() -> ! {
    eprintln!(
        "usage: sample_reader [--pack PATH] [--voice ID] (--text PHONEMES | --lines FILE) [--out FILE]\n\
         \n\
         Text is a phoneme string, not orthography: this build has no grapheme-to-phoneme converter.\n\
         Example: sample_reader --text \"hələʊ wɜːld\" --out /tmp/hello.wav"
    );
    std::process::exit(2)
}

struct Args {
    pack: PathBuf,
    voice: String,
    text: Option<String>,
    lines: Option<PathBuf>,
    out: PathBuf,
}

fn parse_args() -> Result<Args, String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut args = Args {
        pack: default_pack(),
        voice: "tara".to_string(),
        text: None,
        lines: None,
        out: PathBuf::from("sample-out.wav"),
    };
    let mut i = 0;
    while i < argv.len() {
        let flag = argv[i].clone();
        match flag.as_str() {
            "--pack" | "--voice" | "--text" | "--lines" | "--out" => {
                if i + 1 >= argv.len() {
                    return Err(format!("{flag} expects a value"));
                }
                let value = argv[i + 1].clone();
                i += 2;
                match flag.as_str() {
                    "--pack" => args.pack = PathBuf::from(value),
                    "--voice" => args.voice = value,
                    "--text" => args.text = Some(value),
                    "--lines" => args.lines = Some(PathBuf::from(value)),
                    _ => args.out = PathBuf::from(value),
                }
            }
            "--help" | "-h" => usage(),
            other => return Err(format!("unexpected argument: {other}")),
        }
    }
    Ok(args)
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Exit {
    let args = parse_args()?;

    // 1. Load a pack. The loader verifies every declared checksum before handing out a file, so an
    //    error here means the data is bad -- not that this program is.
    let pack: VoicePack = PackLoader::new().load(&args.pack)?;
    let name = args
        .pack
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string();
    println!(
        "pack={name} id={} placeholder={}",
        pack.manifest.id,
        pack.is_placeholder()
    );

    // 2. The persona carries the policy the pack was measured under. Absent persona => no policy,
    //    and a sample that silently invented one would be a bug report waiting to happen.
    let config = match pack.manifest.persona.as_ref() {
        Some(config) => config,
        None => {
            return Err(format!(
                "pack {name} declares no persona, so there is no chunking policy or loudness target to render under"
            )
            .into())
        }
    };
    let persona = Persona::from_pack(config);
    persona.check_overrides_encodable()?;
    let policy = persona.chunking_policy()?;
    let declared = if config.chunking.is_some() { "pack" } else { "engine default" };
    println!(
        "policy max_units={} min_chunk_units={} declared={declared}",
        policy.max_units, policy.min_chunk_units
    );
    if let Some(loudness) = config.loudness.as_ref() {
        println!(
            "loudness target_dbfs={} peak_ceiling={} max_gain_db={}",
            loudness.target_dbfs, loudness.peak_ceiling, loudness.max_gain_db
        );
    }

    // 3. Read the input, then plan it. The plan is what makes the render reproducible: the style row
    //    a chunk reads is its own token count, so a different policy is a different performance.
    let inputs: Vec<String> = match (&args.lines, &args.text) {
        (_, Some(text)) => vec![text.clone()],
        (Some(path), None) => std::fs::read_to_string(path)?
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_string)
            .collect(),
        (None, None) => vec!["hələʊ wɜːld".to_string()],
    };

    for (index, line) in inputs.iter().enumerate() {
        let pieces: Vec<Piece> = line.split_whitespace().map(Piece::phonemes).collect();
        let plan = plan_pieces(&pieces, &policy)?;
        let units: usize = plan.utterances.iter().map(|u| u.units).sum();
        // Measured per utterance, never per line. Two independent reasons the line is the wrong unit:
        // plan_pieces splits a long line into several utterances, and `encode` truncates at MAX_TOKENS,
        // so `encode(line)` on an over-long input saturates at 512 and reports nothing about the plan.
        // And `encode` frames its input -- PAD, characters, PAD -- so the tensor an integrator
        // allocates is 2 rows wider than the content it counts. That off-by-two is the trap here: size
        // `input_ids` from `units` instead of `units + 2` and the model reads a sequence one short.
        let framed: usize = plan.utterances.iter().map(|u| encode(&u.phonemes).len()).sum();
        let rows_match = plan.utterances.iter().all(|u| u.style_row == u.units);
        let framed_ok = plan
            .utterances
            .iter()
            .all(|u| encode(&u.phonemes).len() == u.units + 2);
        println!(
            "line {} chunks={} units={} framed={framed} row_matches_units={rows_match} framed_ok={framed_ok}",
            index + 1,
            plan.utterances.len(),
            units
        );
        // Print first, then refuse: a report line carrying the false flag is what a failing run leaves
        // behind, and an error alone would hide which of the two properties broke.
        if !rows_match {
            return Err(format!(
                "line {}: a style row disagreed with its utterance's unit count, which means the \
                 index into the voice vector moved -- this is a silent voice change, not a cosmetic one",
                index + 1
            )
            .into());
        }
        if !framed_ok {
            return Err(format!(
                "line {}: an utterance's content units and its encoded length disagreed by something \
                 other than the two framing PADs, so the tensor width and the style row cannot both be right",
                index + 1
            )
            .into());
        }
    }

    // 4. Render. MockEngine seeds `<id>-mock`, while a pack's id is `tara`, so resolve the engine's
    //    own name rather than assuming the two agree.
    let mut engine = MockEngine::new();
    engine.initialize().await?;
    let mut engine_voice = args.voice.clone();
    if engine.voice_capabilities(&engine_voice).await.is_err() {
        let candidate = format!("{engine_voice}-mock");
        if engine.voice_capabilities(&candidate).await.is_ok() {
            engine_voice = candidate;
        } else {
            let known: Vec<String> = engine
                .list_voices()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|v| v.voice_id)
                .collect();
            return Err(format!(
                "the engine has no voice {:?} (known: [{}])",
                args.voice,
                known.join(", ")
            )
            .into());
        }
    }

    let request = SynthesisRequest::new(engine_voice.clone(), inputs.join(" "))
        .with_format(SynthesisFormat::Wav);
    let response = engine.synthesize(&request).await?;
    engine.dispose().await?;

    if let Some(parent) = args.out.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let written = vocal_core::wav::write_response_wav(&response, &args.out)?;
    let silent = response.audio.iter().all(|byte| *byte == 0);
    println!(
        "render voice={engine_voice} bytes={written} file={} silent={silent}",
        args.out.display()
    );

    // 5. Say what the bytes are. A sample that prints a WAV and stays quiet about the engine being a
    //    mock is how a repository ends up with a README claiming audible output.
    println!(
        "note: vocal_core::REAL_SYNTHESIS_AVAILABLE={} -- the file above is the mock engine's output, \
         not speech",
        vocal_core::REAL_SYNTHESIS_AVAILABLE
    );
    Ok(())
}
