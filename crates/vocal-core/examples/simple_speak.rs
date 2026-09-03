//! Minimal end-to-end example: engine -> synthesize -> WAV file on disk.
//!
//! Run:
//!     cargo run -p vocal-core --example simple_speak
//!     cargo run -p vocal-core --example simple_speak -- "Namaste, I am Tara." out.wav
//!
//! IMPORTANT — what this does and does not prove. It drives `MockEngine`, which emits
//! digital **silence**, so the WAV it writes is valid but inaudible. It proves the
//! request -> engine -> response -> file plumbing compiles and runs; it does NOT prove
//! speech synthesis. Real voice needs a model-backed engine plus a real `.onnx` inside
//! a `.cvpack`. See `vocal_core::REAL_SYNTHESIS_AVAILABLE` and docs/ROADMAP_EMBEDDED.md.

use std::path::Path;

use vocal_core::engine::mock::MockEngine;
use vocal_core::engine::VoiceEngine;
use vocal_core::synthesis::{SynthesisFormat, SynthesisRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let text = args
        .next()
        .unwrap_or_else(|| "Hello from the Chiti Vocal Runtime.".to_string());
    let out_path = args
        .next()
        .unwrap_or_else(|| "simple_speak.wav".to_string());

    let mut engine = MockEngine::new();
    engine.initialize().await?;

    let request = SynthesisRequest::new("tara-mock", text).with_format(SynthesisFormat::PcmF32);
    let response = engine.synthesize(&request).await?;

    let bytes_written = vocal_core::wav::write_response_wav(&response, Path::new(&out_path))?;

    println!(
        "wrote {out_path} ({bytes_written} bytes; {} ms @ {} Hz mono s16le)",
        response.metadata.duration_ms, response.metadata.sample_rate
    );
    if !vocal_core::REAL_SYNTHESIS_AVAILABLE {
        println!(
            "\nNOTE: engine=mock, so this file is digital silence. \
             No TTS model backend is implemented in this build yet."
        );
    }

    engine.dispose().await?;
    Ok(())
}
