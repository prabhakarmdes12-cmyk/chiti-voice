//! Chiti Voice CLI - Command-line interface for voice synthesis
//!
//! Usage: chiti-voice speak --voice <voice> "text to synthesize"

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;

#[derive(Parser)]
#[command(name = "chiti-voice")]
#[command(about = "Chiti Vocal Runtime CLI - Offline voice synthesis", long_about = None)]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Log level (error, warn, info, debug, trace)
    #[arg(global = true, long, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Synthesize speech from text
    Speak {
        /// Voice to use (e.g., "tara", "kashi", "bobo")
        #[arg(short, long)]
        voice: String,

        /// Text to synthesize
        #[arg(value_name = "TEXT")]
        text: String,

        /// Output audio format (pcm_f32, wav, ogg)
        #[arg(short, long, default_value = "pcm_f32")]
        format: String,

        /// Output file path (optional, default: play to speaker)
        #[arg(short, long)]
        output: Option<String>,

        /// Speech rate multiplier (0.5-2.0)
        #[arg(long, default_value = "1.0")]
        rate: f32,

        /// Pitch multiplier (0.5-2.0)
        #[arg(long, default_value = "1.0")]
        pitch: f32,

        /// Intent/style (warm, calm, alert, etc.)
        #[arg(short, long)]
        intent: Option<String>,
    },

    /// List available voices
    List,

    /// Show engine status
    Status,

    /// Install a voice pack
    Install {
        /// Path to .cvpack file
        #[arg(value_name = "PACK")]
        pack_path: String,
    },

    /// Show version information
    Version,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(&cli.log_level)?;

    match cli.command {
        Commands::Speak {
            voice,
            text,
            format,
            output,
            rate,
            pitch,
            intent,
        } => {
            cmd_speak(&voice, &text, &format, output, rate, pitch, intent).await?;
        }
        Commands::List => {
            cmd_list().await?;
        }
        Commands::Status => {
            cmd_status().await?;
        }
        Commands::Install { pack_path } => {
            cmd_install(&pack_path).await?;
        }
        Commands::Version => {
            cmd_version();
        }
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
        _ => tracing::Level::INFO,
    };

    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .init();

    Ok(())
}

async fn cmd_speak(
    voice: &str,
    text: &str,
    format: &str,
    _output: Option<String>,
    _rate: f32,
    _pitch: f32,
    _intent: Option<String>,
) -> Result<()> {
    info!("Synthesizing with voice: {}", voice);
    info!("Text: {}", text);
    info!("Format: {}", format);

    // TODO: Initialize vocal-core runtime
    // TODO: Load voice pack
    // TODO: Run synthesis
    // TODO: Output audio or play

    println!("Synthesis placeholder - implement voice engine integration");

    Ok(())
}

async fn cmd_list() -> Result<()> {
    println!("Available voices:");
    println!("  - tara      Warm, professional, Indian English");
    println!("  - kashi     Calm, measured, Hindi");
    println!("  - bobo      Playful, expressive, children's voice");

    Ok(())
}

async fn cmd_status() -> Result<()> {
    println!("Chiti Vocal Runtime Status:");
    println!("  Version: 0.1.0-alpha");
    println!("  Local Service: Not running (see chiti-service start)");
    println!("  Available Voices: 0 (install voice packs)");

    Ok(())
}

async fn cmd_install(pack_path: &str) -> Result<()> {
    info!("Installing voice pack: {}", pack_path);

    // TODO: Use voice-pack::PackLoader to load and validate pack
    // TODO: Extract to voice directory (~/.chiti-voice/voices/)
    // TODO: Register voice in local registry

    println!("Pack installation placeholder - implement loader integration");

    Ok(())
}

fn cmd_version() {
    println!("chiti-voice 0.1.0-alpha");
    println!("Chiti Vocal Runtime {} - Offline voice synthesis", 
             vocal_core::VOCAL_CORE_VERSION);
}
