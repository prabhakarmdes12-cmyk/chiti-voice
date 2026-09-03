//! WAV encoding helpers.
//!
//! Small, dependency-free, and deliberately explicit: engines produce raw PCM, and
//! every consumer (CLI file output today, the local daemon's `/v1/speak` in Phase 2)
//! needs the same canonical 44-byte-header mono PCM writer.

use crate::error::{VoiceError, VoiceErrorCode, VoiceResult};
use crate::synthesis::SynthesisResponse;
use std::path::Path;

/// Interpret a little-endian `f32` PCM buffer as 16-bit PCM bytes, leniently.
///
/// The scale is `clamp(floor(x * 32767), -32768, 32767)` via [`crate::audio_levels`], which owns the
/// rule. It used to be `.round()` here, and that single character was why a Rust engine could never
/// have reproduced `assets/offline-spike/*.wav` bit for bit: the reference export floors, and
/// rounding moves up to one sample in every `[-0.5, -1e-5]`-ish neighbourhood. `tests/dsp_parity.rs`
/// pins the current rule against the graph's own float output.
///
/// Values outside `[-1.0, 1.0]` saturate. Use [`response_to_wav`] for engine output: it refuses
/// non-finite samples instead of quietly encoding a rail.
pub fn f32_bytes_to_pcm16(pcm_f32: &[u8]) -> Vec<u8> {
    // as_chunks::<4>() instead of chunks_exact(4): same semantics, and the discarded tail
    // is named so the intent is legible. A short buffer means a truncated sample, which we
    // deliberately drop rather than fabricate — locked by ignores_trailing_partial_sample.
    let (chunks, _trailing) = pcm_f32.as_chunks::<4>();
    let mut out = Vec::with_capacity(chunks.len() * 2);
    for chunk in chunks {
        let sample = f32::from_le_bytes(*chunk);
        out.extend_from_slice(&crate::audio_levels::scale_to_i16(sample, 1.0).to_le_bytes());
    }
    out
}

/// Wrap mono 16-bit PCM samples in a canonical WAV container.
pub fn encode_wav_mono16(pcm16: &[u8], sample_rate: u32) -> Vec<u8> {
    let data_len = pcm16.len() as u32;
    let mut out: Vec<u8> = Vec::with_capacity(44 + pcm16.len());

    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // audio format: PCM integer
    out.extend_from_slice(&1u16.to_le_bytes()); // channels: mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // bytes per second
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm16);

    out
}

/// Encode a synthesis response as WAV bytes.
///
/// Only `f32` PCM engine output is converted; if an engine ever emits an
/// already-encoded container we refuse rather than double-wrapping it.
pub fn response_to_wav(response: &SynthesisResponse) -> VoiceResult<Vec<u8>> {
    if response.metadata.bit_depth != 32 {
        return Err(VoiceError::new(
            VoiceErrorCode::SynthesisFailed,
            format!(
                "WAV encoding expects 32-bit float PCM from the engine, got {}-bit",
                response.metadata.bit_depth
            ),
        ));
    }
    if response.metadata.channels != 1 {
        return Err(VoiceError::new(
            VoiceErrorCode::SynthesisFailed,
            format!(
                "mono WAV encoding cannot wrap {} channels",
                response.metadata.channels
            ),
        ));
    }

    // Decode, then encode strictly: a waveform containing NaN or infinity is a broken model run,
    // and silence or full-scale rails would make it look like a quiet or loud utterance.
    let (chunks, _trailing) = response.audio.as_chunks::<4>();
    let samples: Vec<f32> = chunks.iter().map(|chunk| f32::from_le_bytes(*chunk)).collect();
    let pcm16 = crate::audio_levels::encode_strict(&samples, 1.0)?;
    let mut pcm16_bytes = Vec::with_capacity(pcm16.len() * 2);
    for sample in &pcm16 {
        pcm16_bytes.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(encode_wav_mono16(&pcm16_bytes, response.metadata.sample_rate))
}

/// Write a response as a WAV file.
pub fn write_response_wav(response: &SynthesisResponse, path: &Path) -> VoiceResult<u64> {
    let bytes = response_to_wav(response)?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                VoiceError::new(
                    VoiceErrorCode::SynthesisFailed,
                    format!("cannot create output directory {}: {e}", parent.display()),
                )
            })?;
        }
    }
    std::fs::write(path, &bytes).map_err(|e| {
        VoiceError::new(
            VoiceErrorCode::SynthesisFailed,
            format!("cannot write {}: {e}", path.display()),
        )
    })?;
    Ok(bytes.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthesis::{AudioMetadata, SynthesisFormat};

    #[test]
    fn f32_maps_and_clamps() {
        let one = 1.0f32.to_le_bytes();
        let half = 0.5f32.to_le_bytes();
        let mut buf = Vec::new();
        buf.extend_from_slice(&one);
        buf.extend_from_slice(&half);
        buf.extend_from_slice(&8.0f32.to_le_bytes());
        let out = f32_bytes_to_pcm16(&buf);
        assert_eq!(i16::from_le_bytes([out[0], out[1]]), 32767);
        // 0.5 * 32767 = 16383.5, and flooring gives 16383 — not the 16384 the old `.round()`
        // produced. Both are "right" as audio; only one reproduces the reference bytes.
        assert_eq!(i16::from_le_bytes([out[2], out[3]]), 16383);
        assert_eq!(i16::from_le_bytes([out[4], out[5]]), 32767);
        assert_eq!(i16::from_le_bytes([out[2], out[3]]), crate::audio_levels::to_pcm16(&[0.5])[0]);
    }

    #[test]
    fn ignores_trailing_partial_sample() {
        assert!(f32_bytes_to_pcm16(&[0, 0, 0]).is_empty());
    }

    #[test]
    fn header_is_canonical() {
        let bytes = encode_wav_mono16(&[0u8; 8], 22050);
        assert_eq!(bytes.len(), 52);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(
            u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            36 + 8
        );
        assert_eq!(
            u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
            22050
        );
        assert_eq!(
            u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]),
            8
        );
    }

    #[test]
    fn silence_encodes_to_all_zero_samples() {
        let response = SynthesisResponse {
            audio: vec![0u8; 4 * 100],
            format: SynthesisFormat::PcmF32,
            metadata: AudioMetadata {
                sample_rate: 22050,
                channels: 1,
                bit_depth: 32,
                duration_ms: 4,
            },
        };
        let wav = response_to_wav(&response).unwrap();
        assert_eq!(&wav[44..], &[0u8; 200][..]);
    }

    #[test]
    fn rejects_non_f32_or_stereo() {
        let mut response = SynthesisResponse {
            audio: vec![0u8; 8],
            format: SynthesisFormat::Wav,
            metadata: AudioMetadata {
                sample_rate: 22050,
                channels: 2,
                bit_depth: 32,
                duration_ms: 1,
            },
        };
        let err = response_to_wav(&response).unwrap_err();
        assert_eq!(err.code(), VoiceErrorCode::SynthesisFailed);

        response.metadata.channels = 1;
        response.metadata.bit_depth = 16;
        assert!(response_to_wav(&response).is_err());
    }

    #[test]
    fn writes_file_and_creates_parent() {
        let dir = std::env::temp_dir().join(format!("chiti-wav-{}", std::process::id()));
        let path = dir.join("nested/out.wav");
        let response = SynthesisResponse {
            audio: vec![0u8; 4],
            format: SynthesisFormat::PcmF32,
            metadata: AudioMetadata {
                sample_rate: 16000,
                channels: 1,
                bit_depth: 32,
                duration_ms: 1,
            },
        };
        let written = write_response_wav(&response, &path).unwrap();

        // One 32-bit float sample (4 input bytes) becomes one 16-bit sample (2 bytes)
        // inside a 44-byte canonical header: 46, not 48. The header fields are checked
        // against each other so a future change to either side cannot silently pass.
        assert_eq!(written, 46, "44-byte header + one 16-bit sample");
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(
            &bytes[4..8],
            &((written as u32 - 8).to_le_bytes()),
            "RIFF chunk size must equal file size - 8"
        );
        // Canonical layout: RIFF(0..4) size(4..8) WAVE(8..12) "fmt "(12..16)
        // fmt-chunk 16..36, "data"(36..40) data-size(40..44), payload(44..).
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(
            &bytes[40..44],
            &2u32.to_le_bytes(),
            "payload is one s16 sample"
        );
        assert_eq!(&bytes[44..46], [0u8, 0u8], "one silent sample");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
