//! Level metering, loudness normalisation, and the one float → int16 rule.
//!
//! ## Why this module exists in an "offline voice" crate
//!
//! The 54-voice survey in `docs/research/PERSONA_STYLE_VECTORS.md` measured peaks on one plain
//! sentence: **eight voices exceeded 0.9 and two reached 1.000, i.e. clipped.** Neither the
//! reference implementation nor piper normalises loudness, so on a small speaker or a Pi-class DAC
//! those voices are not merely loud — they are distortion on every utterance, for every user. That
//! makes this a safety component, not mixing cosmetics.
//!
//! ## The two rules, pinned by parity tests
//!
//! 1. **Conversion is `clamp(floor(x * 32767), -32768, 32767)`,** in float64. It floors because
//!    the reference export floors; `wav.rs` used to `.round()`, which is a permanent ≤ 1 LSB
//!    disagreement with every piece of reference audio in this repo. `tests/dsp_parity.rs` grades
//!    this rule against the *graph's own float output* rather than a synthetic vector, so the
//!    fixture cannot agree with a wrong implementation by construction.
//! 2. **Loudness gain is `min(target_linear / rms, ceiling / peak)`, capped by `max_gain_db`.**
//!    All decisions accumulate in float64 (like numpy in the reference path) even though samples are
//!    float32 — keeping the gain as `f64` and folding it in only at conversion time is what makes the
//!    two implementations produce identical PCM instead of near-identical PCM.
//!
//! `max_gain_db` is not decoration. When this module's parity fixtures were first generated, the
//! window search picked a *silent* 512-sample stretch and reported a perfectly well-formed gain of
//! **+147.94 dB** — "normalise silence to −21 dBFS" means "raise the noise floor by twelve orders of
//! magnitude", which on a device means a burst of quantisation mush at the start of playback. A
//! silent buffer is therefore left alone (guard in [`plan`]) and amplification is capped.

use crate::error::{VoiceError, VoiceErrorCode, VoiceResult};
use crate::phoneme_tokens::PCM_SCALE;

/// int16 full scale for a float in `[-1, 1]`. Derived from `phoneme_tokens::PCM_SCALE` so the
/// scaling constant has exactly one definition site in this crate.
pub const SAMPLE_FULL_SCALE: f64 = PCM_SCALE as f64;
/// Never let a peak get closer than this to full scale. 0.98 is what the persona pipeline measured
/// with: Bobo's cast asked for +1.7 dB and was held at 0.980, which is the ceiling doing its job.
pub const DEFAULT_PEAK_CEILING: f32 = 0.98;
/// The most amplification the runtime will apply, in dB. Attenuation is unbounded and harmless.
pub const DEFAULT_MAX_GAIN_DB: f32 = 12.0;

/// A loudness decision: what to reach, and what may stand in the way.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessSpec {
    /// Target RMS level, dBFS (negative; 0.0 is full-scale RMS).
    pub target_dbfs: f32,
    /// Hard ceiling on `|sample|` after gain, in float PCM units.
    pub peak_ceiling: f32,
    /// Cap on *amplification* only, in dB, so silence cannot be raised out of the noise floor.
    pub max_gain_db: f32,
}

impl Default for LoudnessSpec {
    fn default() -> Self {
        Self {
            target_dbfs: -20.0,
            peak_ceiling: DEFAULT_PEAK_CEILING,
            max_gain_db: DEFAULT_MAX_GAIN_DB,
        }
    }
}

/// What [`plan`] decided for a specific buffer. Kept separate from the samples so a caller can log
/// it — a voice that needs +11 dB of gain every utterance is a pack problem worth seeing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessApplied {
    /// The gain actually applied (already respecting the ceiling and the cap).
    pub gain: f64,
    /// RMS before the gain, so the caller can tell "quiet" from "normalised".
    pub rms_before: f64,
    /// Peak before the gain.
    pub peak_before: f64,
    /// True when the peak ceiling, not the target, set the gain: the clip is already too hot.
    pub ceiling_limited: bool,
    /// True when `max_gain_db` set the gain: the buffer was too quiet to bring up honestly.
    pub gain_limited: bool,
}

impl LoudnessApplied {
    /// The identity decision: no scaling, no limits hit.
    fn untouched(rms_before: f64, peak_before: f64) -> Self {
        Self {
            gain: 1.0,
            rms_before,
            peak_before,
            ceiling_limited: false,
            gain_limited: false,
        }
    }
}

/// dBFS → linear amplitude, in float64 to match the reference path's arithmetic.
pub fn linear_from_dbfs(dbfs: f32) -> f64 {
    10f64.powf(f64::from(dbfs) / 20.0)
}

/// Root-mean-square level in float64. Returns 0.0 for an empty buffer, which every caller here
/// treats as "nothing to scale" rather than dividing by it.
pub fn rms(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples
        .iter()
        .map(|s| f64::from(*s) * f64::from(*s))
        .sum();
    (sum / samples.len() as f64).sqrt()
}

/// Largest absolute sample, in float64. Non-finite samples are ignored by `f64::max` semantics
/// rather than poisoning the peak; [`encode_strict`] is what refuses them.
pub fn peak(samples: &[f32]) -> f64 {
    samples
        .iter()
        .fold(0.0f64, |acc, s| acc.max(f64::from(*s).abs()))
}

/// RMS level in dBFS, or `None` for silence — `log10(0)` is not a level.
pub fn level_dbfs(samples: &[f32]) -> Option<f32> {
    let r = rms(samples);
    if r > 0.0 && r.is_finite() {
        Some((20.0 * r.log10()) as f32)
    } else {
        None
    }
}

/// Decide the gain for `samples` without touching them, so the decision can be logged or asserted.
#[must_use]
pub fn plan(samples: &[f32], spec: &LoudnessSpec) -> LoudnessApplied {
    let rms_before = rms(samples);
    let peak_before = peak(samples);
    if rms_before == 0.0 || peak_before == 0.0 {
        return LoudnessApplied::untouched(rms_before, peak_before);
    }
    if !spec.target_dbfs.is_finite() || !spec.peak_ceiling.is_finite() {
        return LoudnessApplied::untouched(rms_before, peak_before);
    }

    let by_target = linear_from_dbfs(spec.target_dbfs) / rms_before;
    let by_ceiling = f64::from(spec.peak_ceiling) / peak_before;
    let ceiling_limited = by_ceiling < by_target;
    let mut applied = LoudnessApplied {
        gain: by_target.min(by_ceiling),
        rms_before,
        peak_before,
        ceiling_limited,
        gain_limited: false,
    };

    if spec.max_gain_db.is_finite() {
        let cap = linear_from_dbfs(spec.max_gain_db);
        if applied.gain > cap {
            applied.gain = cap;
            applied.gain_limited = true;
        }
    }
    applied
}

/// float PCM → int16 PCM: the single conversion rule in this crate.
///
/// `gain` is float64 and applied *before* the floor, in this order, because the reference path
/// (numpy, float64) does the same and the parity fixtures compare with equality, not tolerance.
#[must_use]
pub fn scale_to_i16(sample: f32, gain: f64) -> i16 {
    // Rust's float -> integer cast saturates, so +/-inf land on the rails; NaN lands on 0 after
    // `clamp` leaves it alone. A model that emits non-finite samples should be caught by
    // `encode_strict`, not by this line.
    let scaled = (f64::from(sample) * gain * SAMPLE_FULL_SCALE).floor();
    scaled.clamp(-32768.0, 32767.0) as i16
}

/// [`scale_to_i16`] over a buffer, with unit gain.
#[must_use]
pub fn to_pcm16(samples: &[f32]) -> Vec<i16> {
    encode(samples, 1.0)
}

/// [`scale_to_i16`] over a buffer with a gain applied first.
#[must_use]
pub fn encode(samples: &[f32], gain: f64) -> Vec<i16> {
    samples.iter().map(|s| scale_to_i16(*s, gain)).collect()
}

/// Like [`encode`], but refuses non-finite samples instead of saturating them away.
///
/// A broken or truncated model run typically shows up as NaN/inf in the waveform. Turning that into
/// rails or zeros would produce *plausible-looking* audio; the invariant this crate cares about is
/// that a failure is visible, so it is an error here.
pub fn encode_strict(samples: &[f32], gain: f64) -> VoiceResult<Vec<i16>> {
    if !gain.is_finite() {
        return Err(VoiceError::new(
            VoiceErrorCode::SynthesisFailed,
            format!("loudness gain {gain} is not finite; refusing to encode"),
        ));
    }
    let mut out = Vec::with_capacity(samples.len());
    for (idx, s) in samples.iter().enumerate() {
        if !s.is_finite() {
            return Err(VoiceError::new(
                VoiceErrorCode::SynthesisFailed,
                format!(
                    "sample {idx} of {} is {s}, not a finite level; \
                     engine output looks corrupt, not quiet",
                    samples.len()
                ),
            ));
        }
        out.push(scale_to_i16(*s, gain));
    }
    Ok(out)
}

/// Normalise and encode in one step: what a file/HTTP response path wants.
#[must_use]
pub fn normalise(samples: &[f32], spec: &LoudnessSpec) -> (Vec<i16>, LoudnessApplied) {
    let applied = plan(samples, spec);
    (encode(samples, applied.gain), applied)
}

/// [`normalise`] with the non-finite refusal of [`encode_strict`].
pub fn normalise_strict(samples: &[f32], spec: &LoudnessSpec) -> VoiceResult<(Vec<i16>, LoudnessApplied)> {
    let applied = plan(samples, spec);
    Ok((encode_strict(samples, applied.gain)?, applied))
}

/// Normalise in place for a playback path that consumes float32 (a DAC callback, an in-memory
/// stream). Encoding for a file should go through [`normalise`] instead: rounding back to float32
/// here is fine for listening and would break bit-parity with the reference.
#[must_use]
pub fn apply_f32(samples: &mut [f32], spec: &LoudnessSpec) -> LoudnessApplied {
    let applied = plan(samples, spec);
    for s in samples.iter_mut() {
        *s = (f64::from(*s) * applied.gain) as f32;
    }
    applied
}

/// Scale already-encoded int16 samples by `gain`, flooring like the reference. Used by the pack
/// builder to re-level a recorded clip without a float round-trip.
#[must_use]
pub fn rescale_pcm16(samples: &[i16], gain: f64) -> Vec<i16> {
    samples
        .iter()
        .map(|s| {
            let scaled = (f64::from(*s) * gain).floor();
            scaled.clamp(-32768.0, 32767.0) as i16
        })
        .collect()
}

/// The int16 value corresponding to a float peak ceiling, for "did I stay under it?" assertions.
#[must_use]
pub fn ceiling_in_samples(peak_ceiling: f32) -> i16 {
    (f64::from(peak_ceiling) * SAMPLE_FULL_SCALE).floor().clamp(-32768.0, 32767.0) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(v: &[f32]) -> Vec<f32> {
        v.to_vec()
    }

    #[test]
    fn conversion_floors_and_clamps() {
        assert_eq!(scale_to_i16(0.0, 1.0), 0);
        // One LSB at full scale is *not* representable in float32: 1/32767 rounds to a value whose
        // product with 32767 is 0.9999999990686774, so flooring yields silence, not 1. Measured,
        // not assumed — and the reason no code here may "just add 1" to undo a floor.
        assert_eq!(scale_to_i16(1.0 / 32767.0, 1.0), 0);
        // 0.5 * 32767 = 16383.5 -> floors to 16383. This is the half-LSB the old `.round()`
        // differed on, and precisely why the rule is pinned against real graph output.
        assert_eq!(scale_to_i16(0.5, 1.0), 16383);
        assert_eq!(scale_to_i16(-0.5, 1.0), -16384);
        assert_eq!(scale_to_i16(1.0, 1.0), 32767);
        assert_eq!(scale_to_i16(-1.0, 1.0), -32768);
        assert_eq!(scale_to_i16(8.0, 1.0), 32767);
        assert_eq!(scale_to_i16(-8.0, 1.0), -32768);
        // Below one LSB there is nothing to encode; flooring must not invent a sample.
        assert_eq!(scale_to_i16(3.0e-8, 1.0), 0);
        assert_eq!(scale_to_i16(-3.0e-8, 1.0), -1);
    }

    #[test]
    fn peak_and_rms_are_float64_and_empty_safe() {
        assert_eq!(rms(&[]), 0.0);
        assert_eq!(peak(&[]), 0.0);
        assert_eq!(level_dbfs(&[]), None);
        assert_eq!(level_dbfs(&samples(&[0.0, 0.0, 0.0])), None);
        let s = samples(&[0.5, -0.5]);
        assert!(level_dbfs(&s).is_some());
        assert_eq!(peak(&s), 0.5);
    }

    #[test]
    fn ceiling_wins_over_target_when_the_clip_is_already_hot() {
        let mut s = vec![0.0f32; 8];
        for (i, slot) in s.iter_mut().enumerate() {
            *slot = if i % 2 == 0 { 0.9 } else { -0.9 };
        }
        let spec = LoudnessSpec {
            target_dbfs: 0.0,
            peak_ceiling: 0.5,
            max_gain_db: 96.0,
        };
        let applied = plan(&s, &spec);
        assert!(applied.ceiling_limited, "peak 0.9 over ceiling 0.5 must be the binding limit");
        assert!(!applied.gain_limited);
        let gain = applied.gain;
        assert!(gain < 1.0, "ceiling-limited must attenuate, saw {gain}");
        assert_eq!(ceiling_in_samples(0.5), 16383);
        let (out, _) = normalise(&s, &spec);
        // The ceiling is enforced through the *gain*, and flooring a negative sample can then land
        // one LSB past it (-0.9 * 0.5/0.9 * 32767 floors to -16384 where +0.9 floors to +16383).
        // The reference floors too, so this asymmetry is matched deliberately; asserting `<= ceiling`
        // exactly would be asserting a rule neither implementation follows.
        let bound = ceiling_in_samples(0.5);
        assert!(bound >= 0);
        let bound = i64::from(bound) + 1;
        assert!(out.iter().all(|v| i64::from(v.abs()) <= bound), "peaks escaped the ceiling by more than the floor asymmetry: {out:?}");
    }

    #[test]
    fn silence_is_left_alone_and_amplification_is_capped() {
        let silent = vec![0.0f32; 64];
        let spec = LoudnessSpec {
            target_dbfs: 0.0,
            peak_ceiling: 0.98,
            max_gain_db: 12.0,
        };
        let applied = plan(&silent, &spec);
        assert_eq!(applied.gain, 1.0, "silence must not be amplified");
        assert!(!applied.gain_limited);
        let (out, _) = normalise(&silent, &spec);
        assert!(out.iter().all(|v| *v == 0));

        // Quiet but not silent: the cap binds, and says so, instead of raising the noise floor.
        let quiet: Vec<f32> = (0..256).map(|i| ((i % 7) as f32 - 3.0) * 1.0e-4).collect();
        let applied = plan(&quiet, &spec);
        assert!(applied.gain_limited, "a +60 dB request must be refused");
        assert_eq!(applied.gain, linear_from_dbfs(12.0));
    }

    #[test]
    fn strict_encoding_refuses_non_finite_output_but_lenient_saturates() {
        let mut s = vec![0.25f32; 4];
        s[2] = f32::NAN;
        assert!(encode_strict(&s, 1.0).is_err());
        assert_eq!(to_pcm16(&s)[2], 0, "documented lenient behaviour, for callers that opt in");
        s[2] = f32::INFINITY;
        assert!(encode_strict(&s, 1.0).is_err());
        assert_eq!(to_pcm16(&s)[2], i16::MAX, "saturating cast puts +inf on the rail");
        assert!(encode_strict(&s, f64::NAN).is_err());
    }

    #[test]
    fn rescale_matches_a_float_round_trip() {
        let s = samples(&[0.9, -0.4, 0.0, 0.25]);
        let pcm = to_pcm16(&s);
        assert_eq!(rescale_pcm16(&pcm, 1.0), pcm);
        let half = rescale_pcm16(&pcm, 0.5);
        assert!(half.iter().all(|v| v.abs() <= pcm[0].abs()));
    }

    #[test]
    fn apply_f32_and_encode_agree_within_one_lsb() {
        let s: Vec<f32> = (0..512).map(|i| (i as f32 * 0.001).sin() * 0.4).collect();
        let spec = LoudnessSpec {
            target_dbfs: -18.0,
            peak_ceiling: 0.98,
            max_gain_db: 12.0,
        };
        let (exact, _) = normalise(&s, &spec);
        let mut copy = s.clone();
        let via_playback = apply_f32(&mut copy, &spec);
        assert!(
            !via_playback.gain_limited && !via_playback.ceiling_limited,
            "the two paths must be comparing the same decision, got {via_playback:?}"
        );
        let via_float = to_pcm16(&copy);
        for (a, b) in exact.iter().zip(via_float.iter()) {
            assert!(i16::abs(*a - *b) <= 1, "{a} vs {b}: rounding paths diverged");
        }
    }
}
