
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use tracing::{info, warn};

use vocal_core::engine::mock::MockEngine;
use vocal_core::engine::piper::{PiperEngine, PiperVoiceConfig};
use vocal_core::engine::{BoxedEngine, EngineHealth, VoiceCapabilities, VoiceEngine};
use vocal_core::error::{VoiceError, VoiceErrorCode};
use vocal_core::synthesis::{SynthesisFormat, SynthesisRequest};
use voice_pack::{PackLimits, PackLoader, PackValidator, VoicePack};

#[derive(Parser)]
#[command(name = "chiti-voice")]
#[command(about = "Chiti Vocal Runtime CLI — offline voice synthesis", long_about = None)]
#[command(version = concat!("0.1.0-alpha / core ", env!("CARGO_PKG_VERSION")))]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, default_value = "info", global = true)]
    log_level: String,

    /// Directory holding installed .cvpack files
    #[arg(long, global = true, env = "CHITI_VOICE_DIR")]
    voices_dir: Option<PathBuf>,

    /// Resource limits profile for pack loading: desktop | embedded | tiny
    #[arg(long, default_value = "desktop", global = true)]
    limits: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Synthesize text and write an audio file
    Speak {
        /// Voice id (matches manifest `id` of an installed pack)
        #[arg(short, long)]
        voice: String,

        /// Text to synthesize
        #[arg(value_name = "TEXT")]
        text: String,

        /// Output format: wav | pcm_f32
        #[arg(short, long, default_value = "wav")]
        format: String,

        /// Output file path (default: ./<voice>.<ext>)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Load a pack directly instead of using the voices directory
        #[arg(long)]
        pack: Option<PathBuf>,

        /// Engine to use: mock | piper
        #[arg(long, default_value = "mock")]
        engine: String,

        /// Speech rate multiplier (0.5-2.0)
        #[arg(long, default_value = "1.0")]
        rate: f32,

        /// Pitch multiplier (0.5-2.0)
        #[arg(long, default_value = "1.0")]
        pitch: f32,

        /// Intent/style label defined by the voice pack (e.g. warm, calm, alert)
        #[arg(short, long)]
        intent: Option<String>,

        /// Accept that the mock engine emits silence (do not fail). Needed for
        /// pipeline/CI testing; without this flag, silence is treated as an error.
        #[arg(long)]
        allow_silence: bool,
    },

    /// List installed voices and their real capability status
    List,

    /// Verify a .cvpack: schema, paths, limits, checksums, model presence
    Verify {
        /// Path to .cvpack file
        #[arg(value_name = "PACK")]
        pack_path: PathBuf,
    },

    /// Show runtime status (engines, voices, what this build can actually do)
    Status,

    /// Install a voice pack into the voices directory (validates first)
    Install {
        /// Path to .cvpack file
        #[arg(value_name = "PACK")]
        pack_path: PathBuf,

        /// Install even if the pack contains placeholder models
        #[arg(long)]
        allow_placeholder: bool,

        /// Overwrite an existing pack with the same filename
        #[arg(long)]
        force: bool,
    },

    /// Show version information
    Version,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(&cli.log_level)?;

    let limits = parse_limits(&cli.limits)?;
    let dir = voices_dir(cli.voices_dir.as_deref())?;

    let result = match cli.command {
        Commands::Speak {
            voice,
            text,
            format,
            output,
            pack,
            engine,
            rate,
            pitch,
            intent,
            allow_silence,
        } => {
            cmd_speak(
                &dir, &limits, &voice, &text, &format, output.as_deref(), pack.as_deref(),
                &engine, rate, pitch, intent, allow_silence,
            )
            .await
        }
        Commands::List => cmd_list(&dir, &limits).await,
        Commands::Verify { pack_path } => cmd_verify(&dir, &limits, &pack_path).await,
        Commands::Status => cmd_status(&dir, &limits).await,
        Commands::Install {
            pack_path,
            allow_placeholder,
            force,
        } => cmd_install(&dir, &limits, &pack_path, allow_placeholder, force).await,
        Commands::Version => {
            cmd_version();
            Ok(())
        }
    };

    if let Err(err) = result {
        if let Some(voice_err) = err.downcast_ref::<VoiceError>() {
            eprintln!(
                "error[{}]: {} — detail: {}",
                voice_err.code().as_str(),
                voice_err.code().user_message(),
                voice_err.message()
            );
        } else {
            eprintln!("error: {err:#}");
        }
        std::process::exit(1);
    }
    Ok(())
}

fn init_logging(level: &str) -> Result<()> {
    let level = match level {
        "error" => tracing::Level::ERROR,
        "warn" => tracing::Level::WARN,
        "info" => tracing::Level::INFO,
        "debug" => tracing::Level::DEBUG,
        "trace" => tracing::Level::TRACE,
        other => bail!("unknown --log-level {other:?}"),
    };

    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .try_init()
        .ok();

    Ok(())
}

fn parse_limits(raw: &str) -> Result<PackLimits> {
    Ok(match raw {
        "desktop" => PackLimits::default(),
        "embedded" => PackLimits::embedded(),
        "tiny" => PackLimits::tiny(),
        other => bail!("unknown --limits {other:?} (expected desktop | embedded | tiny)"),
    })
}

fn loader_for(limits: &PackLimits) -> PackLoader {
    PackLoader::with_validator(PackValidator::with_limits(limits.clone()))
}

/// Where installed `.cvpack` files live.
///
/// `--voices-dir` > `$CHITI_VOICE_DIR` > `$HOME/.chiti-voice/voices`.
/// Implemented without a `dirs` crate dependency on purpose: adding one is easy, but
/// every dependency in this project must clear the "no network client, ships on a
/// Raspberry Pi" bar, so we read the two env vars that cover unix + Windows.
fn voices_dir(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .context("no --voices-dir, and neither HOME nor USERPROFILE is set")?;
    Ok(home.join(".chiti-voice").join("voices"))
}

/// Discover installed packs, keyed by manifest `id` (falls back to file stem).
///
/// Packs that fail to load are reported rather than hidden: an install that silently
/// disappears is how "all three voices load ✅" became false in this repo.
fn discover(dir: &Path, limits: &PackLimits) -> (BTreeMap<String, VoicePack>, Vec<(PathBuf, String)>) {
    let mut packs = BTreeMap::new();
    let mut broken = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return (packs, broken),
    };

    let loader = loader_for(limits);
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("cvpack"))
        .collect();
    paths.sort();

    for path in paths {
        match loader.load(&path) {
            Ok(pack) => {
                let key = if pack.manifest.id.is_empty() {
                    path.file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                } else {
                    pack.manifest.id.clone()
                };
                packs.insert(key, pack);
            }
            Err(err) => broken.push((path, err.to_string())),
        }
    }

    (packs, broken)
}

/// One flat parameter list because every argument is a parsed CLI option: wrapping them in
/// a struct would add a type without removing a coupling, and clap already owns the
/// grouping (`#[command(flatten)]`). The count is the lint's complaint, not a design smell.
#[allow(clippy::too_many_arguments)]
async fn cmd_speak(
    dir: &Path,
    limits: &PackLimits,
    voice: &str,
    text: &str,
    format: &str,
    output: Option<&Path>,
    pack: Option<&Path>,
    engine: &str,
    rate: f32,
    pitch: f32,
    intent: Option<String>,
    allow_silence: bool,
) -> Result<()> {
    if !(0.5..=2.0).contains(&rate) || !(0.5..=2.0).contains(&pitch) {
        bail!("--rate and --pitch must be within 0.5..=2.0");
    }

    let format = SynthesisFormat::from_str(format)
        .with_context(|| format!("unknown --format {format:?} (expected wav | pcm_f32 | ogg)"))?;

    let loaded = match pack {
        Some(path) => Some(
            loader_for(limits)
                .load(path)
                .map_err::<VoiceError, _>(|e| e.into())
                .with_context(|| format!("failed to load {}", path.display()))?,
        ),
        None => None,
    };

    let packs = if loaded.is_some() {
        BTreeMap::new()
    } else {
        let (found, broken) = discover(dir, limits);
        for (path, reason) in &broken {
            warn!("ignoring unloadable pack {}: {reason}", path.display());
        }
        found
    };

    let active: Option<&VoicePack> = match (loaded.as_ref(), packs.get(voice)) {
        (Some(p), _) => Some(p),
        (None, Some(p)) => Some(p),
        (None, None) => {
            return Err(anyhow::Error::from(VoiceError::new(
                VoiceErrorCode::VoiceNotFound,
                format!(
                    "no installed voice with id {voice:?} in {} (installed: [{}])",
                    dir.display(),
                    packs.keys().cloned().collect::<Vec<_>>().join(", ")
                ),
            )));
        }
    };

    if let Some(p) = active {
        if p.is_placeholder() {
            println!(
                "warning: voice pack '{}' contains PLACEHOLDER model files (status=\"placeholder\") \
                 — it cannot produce real speech",
                p.manifest.id
            );
        }
        report_prosody(p, intent.as_deref(), rate, pitch);
    }

    let mut engine_impl: BoxedEngine = match engine {
        "mock" => Box::new(MockEngine::new()),
        "piper" => {
            let mut piper = PiperEngine::new();
            if let Some(p) = active {
                piper.register_voice(PiperVoiceConfig {
                    voice_id: p.manifest.id.clone(),
                    model_path: "model.onnx".to_string(),
                    language: p
                        .manifest
                        .supported_languages
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "en".to_string()),
                    sample_rate: 22050,
                    phonemes: vec![],
                });
            }
            Box::new(piper)
        }
        other => bail!("unknown --engine {other:?} (expected mock | piper)"),
    };

    engine_impl.initialize().await?;

    // Resolve the voice id the *engine* knows. MockEngine seeds `tara-mock`, while the
    // pack id is `tara`, so a naive passthrough would fail with VOICE_NOT_FOUND.
    let mut engine_voice_id = voice.to_string();
    if engine_impl.voice_capabilities(&engine_voice_id).await.is_err() {
        let candidate = format!("{voice}-mock");
        if engine == "mock" && engine_impl.voice_capabilities(&candidate).await.is_ok() {
            info!("resolved voice {voice:?} to engine id {candidate:?}");
            engine_voice_id = candidate;
        } else {
            let known: Vec<String> = engine_impl
                .list_voices()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|v| v.voice_id)
                .collect();
            return Err(anyhow::Error::from(VoiceError::new(
                VoiceErrorCode::VoiceNotFound,
                format!(
                    "engine {engine:?} has no voice {voice:?} (known: [{}])",
                    known.join(", ")
                ),
            )));
        }
    }

    if engine == "piper" {
        match engine_impl.health().await? {
            EngineHealth::Healthy => {}
            other => println!("warning: piper engine health = {other:?}"),
        }
    }

    let request = SynthesisRequest::new(engine_voice_id, text)
        .with_format(format)
        .with_rate(rate);
    let request = match intent {
        Some(intent) => request.with_intent(intent),
        None => request,
    };

    info!("synthesizing {} chars with format {format:?}", request.text.len());
    let response = engine_impl.synthesize(&request).await?;
    engine_impl.dispose().await?;

    let out_path = output
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from(format!("{voice}.{}", ext_for(format))));

    let written = match format {
        SynthesisFormat::Wav => {
            vocal_core::wav::write_response_wav(&response, &out_path)?
        }
        SynthesisFormat::PcmF32 => {
            if let Some(parent) = out_path.parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(&out_path, &response.audio)?;
            response.audio.len() as u64
        }
        SynthesisFormat::Ogg => bail!(
            "ogg output is not implemented; no encoder exists in this build \
             (capabilities advertise it, which is itself a bug — see docs/ROADMAP_EMBEDDED.md)"
        ),
    };

    let is_silence = response.audio.iter().all(|b| *b == 0);
    println!(
        "wrote {} ({} bytes, {} ms @ {} Hz, engine={})",
        out_path.display(),
        written,
        response.metadata.duration_ms,
        response.metadata.sample_rate,
        engine
    );

    if is_silence {
        let msg = "synthesized audio is digital silence: no speech backend is implemented \
                   (vocal_core::REAL_SYNTHESIS_AVAILABLE == false)";
        if !allow_silence && engine != "mock" {
            return Err(anyhow::Error::from(VoiceError::new(VoiceErrorCode::SynthesisFailed, msg)));
        }
        println!("NOTE: {msg}");
    }

    Ok(())
}

fn ext_for(format: SynthesisFormat) -> &'static str {
    match format {
        SynthesisFormat::Wav => "wav",
        SynthesisFormat::PcmF32 => "f32",
        SynthesisFormat::Ogg => "ogg",
    }
}

/// Report (not fake) the persona prosody that the pack declares for this intent.
fn report_prosody(pack: &VoicePack, intent: Option<&str>, rate: f32, pitch: f32) {
    let Some(persona) = pack.manifest.persona.as_ref() else {
        return;
    };

    println!(
        "persona {} (default rate {:.2}, pitch {:.2})",
        persona.display_name, persona.default_rate, persona.default_pitch
    );

    // Look the intent up with the inner &str; `intent` itself is an Option, and
    // HashMap::get takes a borrowed key, not an Option. (E0308)
    let profile = intent.and_then(|name| persona.intent_profiles.get(name));
    match (intent, profile) {
        (Some(name), Some(profile)) => println!(
            "intent {name:?}: rate {:.2}, pitch {:.2}, energy {:.2}, pause_factor {:.2}",
            profile.rate, profile.pitch, profile.energy, profile.pause_factor
        ),
        (Some(name), None) => {
            let mut known: Vec<&str> = persona.intent_profiles.keys().map(|k| k.as_str()).collect();
            known.sort_unstable();
            println!(
                "warning: intent {name:?} is not defined by this pack (available: {})",
                known.join(", ")
            );
        }
        (None, _) => {}
    }

    if !vocal_core::REAL_SYNTHESIS_AVAILABLE {
        println!(
            "(prosody is parsed and validated but NOT applied: the mock engine ignores \
             rate={rate:.2} pitch={pitch:.2}; a real backend must consume them)"
        );
    }
}

async fn cmd_list(dir: &Path, limits: &PackLimits) -> Result<()> {
    let (packs, broken) = discover(dir, limits);

    println!("voices dir: {}", dir.display());
    if packs.is_empty() && broken.is_empty() {
        println!("  (none installed — run: chiti-voice install <pack.cvpack>)");
    }

    for (id, pack) in &packs {
        let manifest = &pack.manifest;
        let model_bytes = pack
            .get_file("model.onnx")
            .map(|b| b.len())
            .unwrap_or_default();
        println!(
            "  {id:<10} v{:<8} {:<22} {:<12} model={:>9} bytes{}",
            manifest.version,
            manifest.engine_family,
            manifest.supported_languages.join("+"),
            model_bytes,
            if pack.is_placeholder() {
                "  [PLACEHOLDER — cannot speak]"
            } else {
                ""
            }
        );
    }

    for (path, reason) in &broken {
        println!("  {}  UNLOADABLE: {reason}", path.display());
    }

    if !broken.is_empty() {
        bail!("{} installed pack(s) failed validation", broken.len());
    }
    Ok(())
}

async fn cmd_verify(dir: &Path, limits: &PackLimits, pack_path: &Path) -> Result<()> {
    println!("verifying {}", pack_path.display());
    let pack = loader_for(limits)
        .load(pack_path)
        .map_err::<VoiceError, _>(|e| e.into())
        .with_context(|| format!("failed to load {}", pack_path.display()))?;

    let manifest = &pack.manifest;
    println!("  schema_version   {}", manifest.schema_version);
    println!("  id / name        {} / {}", manifest.id, manifest.name);
    println!("  version          {}", manifest.version);
    println!("  engine_family    {} (min {})", manifest.engine_family, manifest.engine_version_min);
    println!("  languages        {}", manifest.supported_languages.join(", "));
    println!("  license          {}", manifest.license);

    for file in &manifest.files {
        let actual = pack.get_file(&file.path).map(|b| b.len()).unwrap_or_default();
        println!(
            "  file             {:<24} {:>10} bytes  sha256={}…  OK",
            file.path,
            actual,
            &file.checksum_sha256[..12.min(file.checksum_sha256.len())]
        );
    }

    let model_bytes = pack.get_file("model.onnx").map(|b| b.len()).unwrap_or_default();
    if pack.is_placeholder() {
        println!("  status           PLACEHOLDER (declared; not a usable voice)");
    } else if model_bytes < 1_000_000 {
        println!(
            "  status           FAIL — model.onnx is {model_bytes} bytes; no real ONNX model is \
             present, so this pack cannot synthesize speech"
        );
        return Err(anyhow::Error::from(VoiceError::new(
            VoiceErrorCode::PackInvalidFormat,
            format!("model.onnx is only {model_bytes} bytes — a usable voice model is missing"),
        )));
    } else {
        println!("  status           OK (model present)");
    }

    if let Some(prov) = manifest.provenance.as_ref() {
        println!(
            "  provenance       consent_obtained={:?} model_license={:?} signature={:?}",
            prov.consent_obtained, prov.model_license, prov.signature_status
        );
    } else {
        println!("  provenance       MISSING");
    }

    let (installed, _) = discover(dir, limits);
    println!(
        "\npass: manifest + limits + checksums + sizes validated ({limits:?})",
    );
    println!("installed voices after this check: {}", installed.len());
    Ok(())
}

async fn cmd_status(dir: &Path, limits: &PackLimits) -> Result<()> {
    println!("Chiti Vocal Runtime Status");
    println!("  vocal-core version      {}", vocal_core::VOCAL_CORE_VERSION);
    println!(
        "  real synthesis available {}",
        vocal_core::REAL_SYNTHESIS_AVAILABLE
    );
    println!("  voices dir              {}", dir.display());

    let (packs, broken) = discover(dir, limits);
    println!("  loadable voice packs    {}", packs.len());
    if !broken.is_empty() {
        println!("  unloadable packs        {}", broken.len());
        for (path, reason) in &broken {
            println!("      {}: {reason}", path.display());
        }
    }
    let usable: Vec<&String> = packs
        .iter()
        .filter(|(_, p)| !p.is_placeholder())
        .map(|(id, _)| id)
        .collect();
    println!("  usable voices           {}", if usable.is_empty() { "none".to_string() } else { usable.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ") });

    println!("  engines");
    println!("      mock   available (silence only — for pipeline tests)");
    println!("      piper  NOT AVAILABLE: no ONNX inference implemented");
    if let Some(cap) = first_capabilities().await {
        println!("      registered voices: {cap}");
    }

    println!(
        "\n  audio output: WAV / raw PCM written to a file. No audio-device playback \
         in this build, so `speak` never plays sound."
    );
    if !vocal_core::REAL_SYNTHESIS_AVAILABLE {
        println!(
            "  This build cannot produce speech. Everything above is plumbing, not voice."
        );
    }
    Ok(())
}

async fn first_capabilities() -> Option<String> {
    let mut engine = MockEngine::new();
    engine.initialize().await.ok()?;
    let voices: Vec<VoiceCapabilities> = engine.list_voices().await.ok()?;
    Some(
        voices
            .iter()
            .map(|v| v.voice_id.as_str())
            .collect::<Vec<_>>()
            .join(", "),
    )
}

async fn cmd_install(
    dir: &Path,
    limits: &PackLimits,
    pack_path: &Path,
    allow_placeholder: bool,
    force: bool,
) -> Result<()> {
    // Validate BEFORE copying: an install that accepts a broken pack is how a repo
    // ends up shipping three packs that all fail their own checksums.
    let pack = loader_for(limits)
        .load(pack_path)
        .map_err::<VoiceError, _>(|e| e.into())
        .with_context(|| format!("failed to load {}", pack_path.display()))?;

    if pack.is_placeholder() && !allow_placeholder {
        bail!(
            "refusing to install {}: the pack contains placeholder models. \
             Pass --allow-placeholder to install it anyway for pipeline testing.",
            pack_path.display()
        );
    }

    std::fs::create_dir_all(dir)
        .with_context(|| format!("cannot create voices dir {}", dir.display()))?;

    let file_name = pack_path
        .file_name()
        .context("pack path has no file name")?;
    let dest = dir.join(file_name);

    if dest.exists() && !force {
        bail!("{} already exists (use --force to overwrite)", dest.display());
    }

    // We copy the archive and load from inside it; we never extract entries to disk.
    // That is a deliberate security choice: extracting ZIP entries is where zip-slip
    // bugs come from, and `.cvpack` has no reason to exist unpacked.
    std::fs::copy(pack_path, &dest)
        .with_context(|| format!("cannot copy to {}", dest.display()))?;

    println!(
        "installed {} (id={}, version={}, languages={})",
        dest.display(),
        pack.manifest.id,
        pack.manifest.version,
        pack.manifest.supported_languages.join(", ")
    );
    if pack.is_placeholder() {
        println!("warning: installed a PLACEHOLDER pack — synthesis will not work");
    }
    Ok(())
}

fn cmd_version() {
    println!("chiti-voice 0.1.0-alpha");
    println!(
        "Chiti Vocal Runtime {} - offline voice synthesis (real synthesis available: {})",
        vocal_core::VOCAL_CORE_VERSION, vocal_core::REAL_SYNTHESIS_AVAILABLE
    );
}
