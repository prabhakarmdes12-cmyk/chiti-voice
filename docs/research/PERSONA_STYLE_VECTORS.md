# Persona style vectors — what the 54 stock voices are, and what "voice personality" can mean here

**Date:** 2026-09-03 · **Engine:** Kokoro-82M int8 (the same graph as `KOKORO_OFFLINE_SPIKE.md`) · **All numbers measured in this repo, nothing quoted.**

Three things happened here: every stock voice was measured, the persona specs were checked against
what the model actually accepts, and three personas were **derived** (style-vector blends) and
measured back. `docs/research/KOKORO_OFFLINE_SPIKE.md` explains why derivation is the only
"generate my voice" route that costs megabytes; this file is what that produces, and where it stops.

```bash
pip install piper-tts onnxruntime numpy                       # then, per model:
python3 scripts/fetch-offline-model.py --accept-licence --all-voices --dest models
python3 scripts/survey-persona-voices.py --emit /tmp/survey.json   # ~450 s single-threaded
python3 scripts/derive-persona-style.py --persona tara \
    --sources af_bella:0.40,af_heart:0.35,af_aoede:0.25 \
    --speed 1.0 --target-dbfs -21.0 --report --wav-out out.wav
```

The raw measurements are committed as
[`persona-survey.json`](./persona-survey.json) — 58 rows with the engine's sha256, the exact
sentences, and the metric method — so §2 and §3 can be re-derived without re-running the survey, and
any disagreement with this document is checkable against bytes.

## 1. Four of the five parameters in the persona specs do not exist

`docs/personas/*.md` (from the PRD) give each voice a parameter table. This is what the engine
actually offers — verified by listing the ONNX graph's inputs, not by reading docs about it:

```
graph inputs: ['input_ids', 'speed', 'style']
```

| Spec parameter | Range in the specs | Kokoro input | Status |
|---|---|---|---|
| `Speed` | 0.7 – 1.4 (Tara 1.0, Kashi 0.92, Bobo 1.15) | `speed` float32[1] | ✅ real input |
| `Pitch` | ±0.5 (Tara 0.0, Kashi −0.10, Bobo +0.30) | — | ❌ **no input exists** |
| `Energy` | 0–1 (0.55 / 0.48 / 0.80) | — | ❌ **no input exists** |
| `Warmth` | 0–1 (0.72 / 0.60 / 0.75) | — | ❌ **no input exists** |
| `Expressiveness` | 0–1 (0.58 / 0.42 / 0.88) | — | ❌ **no input exists** |

Register, loudness and pitch movement are properties of the **voice vector** (`style`, 256 floats)
and of the voice it came from; they are not dials. So a persona can only be built by *casting and
blending vectors*, plus post-processing where post-processing is honest:

| Spec parameter | Honest approximation available today |
|---|---|
| `Speed` | the `speed` input ✅ (measured: non-linear at the edges, and not exactly 1/s) |
| `Pitch` | choose/blend voices by measured median F0 — the survey's first column. Not "±0.5" of anything |
| `Energy` | loudness normalisation to a target dBFS (`--target-dbfs`, with a peak ceiling). It is a gain, not a performance change |
| `Expressiveness` | choose a voice by measured pitch range (`f0_range_hz`). **Do not** reach for it by blending — see §4 |
| `Warmth` | nothing honest. Spectral tilt is a DSP decision we have not made |

**Consequence for the product:** either those parameters become *pack-level* fields that drive
casting and post-processing (this document is the evidence that they can), or they come out of the
specs. Leaving them written as engine parameters is the worst option: `docs/personas/*.md` currently
read like an API that does not exist.

## 2. Every stock voice, measured

One sentence per voice — identical text, identical `speed`, one thread. Metrics from
`scripts/voice_metrics.py` (autocorrelation F0 track, RMS/peak, phoneme rate), so they are comparable
across voices.

### English pass — 54 voices, sorted by register

| voice | F0 (Hz) | pitch range (Hz) | phonemes/s | voiced | level (dBFS) | peak | dur (s) | infer (s) |
|---|---|---|---|---|---|---|---|---|
| `am_onyx` | 82.8 | 49.3 | 15.60 | 0.57 | -23.1 | 0.671 | 5.45 | 8.2 |
| `bm_lewis` | 90.9 | 77.5 | 14.11 | 0.47 | -24.6 | 0.634 | 6.03 | 8.3 |
| `im_nicola` | 91.3 | 329.8 | 16.50 | 0.72 | -19.4 | 0.715 | 5.15 | 7.4 |
| `jm_kumo` | 92.7 | 46.0 | 13.71 | 0.54 | -20.7 | 1.000 | 6.20 | 8.7 |
| `am_echo` | 104.3 | 89.0 | 15.45 | 0.56 | -23.2 | 0.547 | 5.50 | 8.0 |
| `am_puck` | 108.1 | 99.0 | 15.67 | 0.60 | -19.3 | 0.940 | 5.42 | 7.9 |
| `am_michael` | 110.6 | 75.1 | 14.05 | 0.58 | -24.9 | 0.629 | 6.05 | 8.7 |
| `zm_yunyang` | 110.6 | 103.6 | 16.67 | 0.47 | -25.2 | 0.882 | 5.10 | 7.3 |
| `zm_yunjian` | 112.7 | 97.8 | 15.81 | 0.52 | -21.9 | 0.603 | 5.38 | 7.6 |
| `am_adam` | 114.8 | 74.3 | 15.74 | 0.56 | -19.3 | 0.855 | 5.40 | 7.7 |
| `bm_daniel` | 120.0 | 70.4 | 16.19 | 0.66 | -21.7 | 0.603 | 5.25 | 7.2 |
| `bm_fable` | 123.1 | 319.0 | 15.96 | 0.70 | -23.9 | 1.000 | 5.33 | 7.7 |
| `am_liam` | 124.0 | 142.6 | 16.75 | 0.61 | -22.2 | 0.574 | 5.08 | 7.8 |
| `em_alex` | 127.7 | 80.8 | 18.89 | 0.67 | -23.0 | 0.600 | 4.50 | 6.3 |
| `hm_psi` | 129.0 | 98.8 | 13.88 | 0.56 | -18.7 | 0.958 | 6.12 | 8.8 |
| `hm_omega` | 130.1 | 101.1 | 14.05 | 0.60 | -17.1 | 0.900 | 6.05 | 8.8 |
| `am_fenrir` | 131.1 | 132.3 | 15.60 | 0.61 | -20.0 | 0.982 | 5.45 | 8.1 |
| `em_santa` | 131.9 | 113.4 | 18.68 | 0.65 | -23.0 | 0.740 | 4.55 | 6.3 |
| `pm_alex` | 134.1 | 77.1 | 18.38 | 0.64 | -23.1 | 0.636 | 4.62 | 6.4 |
| `pm_santa` | 136.0 | 99.8 | 18.38 | 0.63 | -23.3 | 0.699 | 4.62 | 6.6 |
| `bm_george` | 137.9 | 101.3 | 14.35 | 0.65 | -21.7 | 0.516 | 5.92 | 8.7 |
| `af_alloy` | 139.5 | 96.1 | 15.25 | 0.57 | -22.9 | 0.599 | 5.58 | 8.1 |
| `af_kore` | 148.1 | 136.9 | 15.18 | 0.62 | -18.9 | 0.609 | 5.60 | 8.2 |
| `af_nicole` | 148.1 | 270.8 | 10.43 | 0.35 | -23.1 | 0.747 | 8.15 | 12.4 |
| `am_eric` | 150.9 | 122.0 | 17.44 | 0.69 | -21.7 | 0.472 | 4.88 | 7.3 |
| `zm_yunxi` | 152.4 | 104.0 | 15.74 | 0.52 | -22.0 | 0.806 | 5.40 | 7.6 |
| `af_nova` | 156.9 | 111.3 | 15.96 | 0.55 | -27.8 | 0.321 | 5.33 | 7.7 |
| `af_sky` | 158.9 | 122.4 | 15.74 | 0.59 | -24.5 | 0.448 | 5.40 | 7.6 |
| `am_santa` | 162.2 | 220.0 | 15.89 | 0.60 | -23.3 | 0.548 | 5.35 | 7.7 |
| `ef_dora` | 171.4 | 101.4 | 19.10 | 0.68 | -23.6 | 0.613 | 4.45 | 6.4 |
| `af_river` | 176.5 | 91.5 | 16.92 | 0.64 | -22.2 | 0.665 | 5.03 | 7.0 |
| `bf_emma` | 176.5 | 74.2 | 15.89 | 0.58 | -21.4 | 0.611 | 5.35 | 7.4 |
| `pf_dora` | 180.5 | 93.3 | 18.78 | 0.68 | -23.8 | 0.676 | 4.53 | 6.3 |
| `af_aoede` | 181.8 | 119.9 | 15.60 | 0.62 | -18.4 | 0.702 | 5.45 | 8.0 |
| `hf_beta` | 181.8 | 109.7 | 15.18 | 0.65 | -18.2 | 0.863 | 5.60 | 8.1 |
| `af_sarah` | 183.2 | 130.9 | 15.18 | 0.54 | -21.9 | 0.830 | 5.60 | 8.1 |
| `bf_lily` | 184.6 | 144.0 | 16.19 | 0.65 | -22.0 | 0.612 | 5.25 | 7.5 |
| `af_bella` | 190.5 | 123.8 | 14.85 | 0.61 | -22.1 | 0.660 | 5.72 | 8.4 |
| `af_heart` | 190.5 | 105.3 | 16.04 | 0.57 | -23.5 | 0.538 | 5.30 | 7.6 |
| `bf_isabella` | 200.0 | 89.4 | 15.60 | 0.59 | -18.6 | 0.960 | 5.45 | 7.6 |
| `bf_alice` | 201.7 | 154.8 | 15.74 | 0.64 | -21.3 | 0.728 | 5.40 | 7.9 |
| `af_jessica` | 206.0 | 154.8 | 17.44 | 0.67 | -22.9 | 0.588 | 4.88 | 7.1 |
| `ff_siwis` | 206.9 | 105.7 | 15.96 | 0.68 | -20.3 | 0.947 | 5.33 | 7.7 |
| `hf_alpha` | 208.7 | 125.1 | 14.17 | 0.62 | -16.2 | 0.805 | 6.00 | 8.7 |
| `if_sara` | 212.4 | 114.6 | 17.71 | 0.68 | -15.0 | 0.966 | 4.80 | 6.8 |
| `jf_gongitsune` | 216.2 | 137.8 | 12.64 | 0.54 | -21.7 | 0.532 | 6.72 | 9.5 |
| `zf_xiaoxiao` | 229.7 | 134.5 | 15.74 | 0.62 | -17.6 | 0.721 | 5.40 | 7.8 |
| `zf_xiaobei` | 230.8 | 145.3 | 13.18 | 0.63 | -21.5 | 0.527 | 6.45 | 9.4 |
| `zf_xiaoni` | 230.8 | 105.0 | 15.25 | 0.57 | -20.0 | 0.722 | 5.58 | 8.2 |
| `jf_nezumi` | 240.0 | 99.6 | 13.08 | 0.56 | -21.4 | 0.602 | 6.50 | 9.2 |
| `jf_alpha` | 266.7 | 132.4 | 14.72 | 0.66 | -20.4 | 0.599 | 5.78 | 8.3 |
| `zm_yunxia` | 289.2 | 126.7 | 15.25 | 0.58 | -21.6 | 0.557 | 5.58 | 7.9 |
| `zf_xiaoyi` | 292.7 | 110.3 | 15.74 | 0.58 | -20.6 | 0.522 | 5.40 | 7.6 |
| `jf_tebukuro` | 324.3 | 144.7 | 13.65 | 0.57 | -22.0 | 0.563 | 6.22 | 8.9 |

### Hindi pass — the four Hindi vectors, sorted by register

| voice | F0 (Hz) | pitch range (Hz) | phonemes/s | voiced | level (dBFS) | peak | dur (s) | infer (s) |
|---|---|---|---|---|---|---|---|---|
| `hm_omega` | 129.4 | 124.3 | 13.07 | 0.57 | -17.1 | 0.820 | 3.83 | 5.4 |
| `hm_psi` | 133.7 | 113.2 | 13.51 | 0.54 | -18.7 | 0.819 | 3.70 | 5.2 |
| `hf_beta` | 179.1 | 134.2 | 15.15 | 0.60 | -17.9 | 0.767 | 3.30 | 4.5 |
| `hf_alpha` | 222.2 | 154.9 | 13.33 | 0.60 | -16.4 | 0.731 | 3.75 | 5.4 |

What this buys us, stated as decisions rather than taste:

- **Casting is now arguable.** Tara (mid-high register, moderate pace, restrained) sits near
  `af_heart` / `af_bella` / `af_aoede` at 182–191 Hz and 14.9–16.0 phon/s. Kashi (lower, unhurried,
  "restrained") wants 95–130 Hz and the *narrow* ranges (`am_michael` 64.5 Hz, `bm_lewis` 83.2 Hz).
  Bobo (high, quick, animated) wants the extremes: `am_santa` 206.9 Hz, 254.1 Hz range, 17.5 phon/s.
- **The pace spread is 1.8× for identical text** (10.4 → 19.1 phon/s). A persona's "slowness" is real
  and measurable in these vectors — and it is mostly *not* the `speed` input, which is a different axis.
- **`hm_*` is the only path to Hindi today**, and `hm_omega` is the closest measured match to Kashi's
  description. That is how Kashi's recipe got picked — not by preference.

8 of 54 voices exceeded peak 0.9 on the English pass: `am_fenrir` (0.982), `am_puck` (0.940), `bf_isabella` (0.960), `bm_fable` (1.000), `ff_siwis` (0.947), `hm_psi` (0.958), `if_sara` (0.966), `jm_kumo` (1.000).

`bm_fable` and `jm_kumo` **reach 1.000, so they clip**, on a plain sentence, and six more sit above
0.9. Neither the reference implementation nor piper normalises loudness. Two consequences, both
binding on the runtime: the `.cvpack` spec needs a loudness slot and a peak ceiling (ROADMAP §6 lists
it; this is the measurement behind it), and `--target-dbfs` belongs in the persona pipeline rather
than being a garnish.

## 3. Three personas derived, and measured back

| persona | recipe (weights sum to 1) | speed | measured F0 | measured range | pace | level after normalise | dev from prediction |
|---|---|---|---|---|---|---|---|
| **bobo-solo** | `am_santa` 1.00 | 1.15 | 206.9 Hz | 261.6 (src min 254.1) | 17.50/s | -17.5 dBFS | 0 % (control) |
| **bobo** | `am_santa` 0.40 + `af_jessica` 0.35 + `bf_alice` 0.25 | 1.15 | 203.4 Hz | 166.5 (src min 181.1) | 18.39/s | -20.4 dBFS | 3.7 % |
| **kashi** | `hm_omega` 0.45 + `am_michael` 0.35 + `bm_lewis` 0.20 | 0.92 | 113.7 Hz | 92.2 (src min 64.5) | 12.58/s | -22.5 dBFS | 2.8 % |
| **tara-indic** | `hf_beta` 0.70 + `af_heart` 0.30 | 1.0 | 186.0 Hz | 111.8 (src min 105.3) | 15.18/s | -21.0 dBFS | 0.9 % |
| **tara** | `af_bella` 0.40 + `af_heart` 0.35 + `af_aoede` 0.25 | 1.0 | 187.5 Hz | 100.9 (src min 105.3) | 15.45/s | -21.0 dBFS | 0.4 % |

Recipes are in `docs/research/persona-recipes/*.json` (weights, sources, measured outcome, source
files, and a `provenance_status` that blocks shipping). Audio in `assets/offline-spike/`:
`persona-tara.wav`, `persona-kashi.wav`, `persona-bobo.wav`, `persona-bobo-solo.wav`,
`persona-tara-indic.wav` — 24 kHz mono, generated while `REAL_SYNTHESIS_AVAILABLE = false`, for the
same reason as the other spike clips.

## 4. The finding that changes the recipe format

Blending behaves well on register and badly on prosody, and those are two different claims:

- **Register interpolates.** Every blend's measured F0 fell inside its sources' span, 0.4 %–3.7 %
  from the weighted-mean prediction. Weights do roughly what you expect — *but* the deviation is
  non-zero and not sign-stable, which is why `--report` states that a recipe predicts **direction,
  never a value**. Measure the result before believing the weights.
- **Prosodic range is attenuated.** Bobo's three sources have pitch ranges of 254.1 / 212.5 /
  181.1 Hz; the blend measured **164.5 Hz — lower than all three**. Averaging style vectors averages
  the contours too, so a mix always lands calmer than its most animated source.
- **Control, so this isn't a guess:** the same source alone (`bobo-solo`, `am_santa` at weight 1.0)
  measured **254.1 Hz — identical to the source row**, which proves the pipeline does not degrade
  audio and that the compression is caused by mixing.

**So blending is for neutrality, not for personality.** Restrained personas (Tara, Kashi) can be
blended — that is what makes them smooth. An expressive persona must *pick* the widest-range voice
and leave it alone, using `speed` and loudness for the rest. Any "expressiveness" dial in a pack must
therefore be a **selection + gain** parameter, never a blend weight; `persona-recipes/` now encodes
that difference by shipping `bobo` (blended) next to `bobo-solo` (control).

## 5. What this does not prove

1. **Accent.** `hf_beta`'s English row is statistically indistinguishable from `af_heart`'s (181.8 vs
   190.5 Hz, 109.7 vs 105.3 Hz range), yet `persona-tara-indic.wav` sounds different to a human. Our
   metrics do not capture that, so that clip is an *invitation to listen*, not a result. Anyone
   claiming "Indian English achieved" from these numbers would be fabricating.
2. **Quality.** Register, pace, range and loudness describe a voice; none of them say whether it is
   pleasant, and no MOS exists here. The 50-sentence, 3-rater listening test is still Step 1's gate.
3. **Intelligibility of the Hindi clip.** `persona-kashi.wav` was synthesised through espeak-ng's `hi`
   voice and nobody has verified those phonemes — the same caveat `offline-kashi.wav` already carries.
4. **Warmth.** §1: not implementable. If a rater says "warm", we have no knob that answers it.
5. **Licence of what we made.** A blend of Kokoro vectors is a **derivative work** of weights whose
   licence is *unverified* (the carrier's MIT LICENSE covers its code, not its model data — see
   ROADMAP §3). Hence: `models/persona-*.bin` is gitignored, every recipe's `provenance_status` reads
   `incomplete by design — nothing may be shipped from this file`, and `VOICE_INV_008` will keep
   refusing these bytes as a `real` pack. The **recipe JSONs** are committable and useful; the
   **vectors** become shippable only after path C or a cleared upstream licence.

## 6. Follow-up: the loudness rule became Rust, and grew a guard

`--target-dbfs` is no longer only a script feature. `crates/vocal-core/src/audio_levels.rs`
implements the same decision — `gain = min(target_linear / rms, ceiling / peak)` — and
`tests/dsp_parity.rs` grades it against the graph's own float output, with the peak-ceiling invariant
asserted in samples. Two things came out of building that parity:

* **A cap was missing, in both implementations.** The fixture generator first picked a *silent*
  512-sample window and reported a well-formed gain of **+147.94 dB**: "normalise silence to
  −21 dBFS" literally means "raise the noise floor by twelve orders of magnitude", which on a speaker
  is a puff of quantisation mush. So `LoudnessSpec::max_gain_db` (12 dB) caps *amplification* only —
  attenuation is always safe — a zero-RMS buffer is left at unity instead of dividing by it, and
  `derive-persona-style.py --max-gain-db` mirrors it. Re-rendering Tara's cast afterwards produced a
  bit-identical WAV (`7eeb9b57df380e96…`), which is how we know the guard binds on nothing that was
  already sane.
* **Flooring makes the ceiling asymmetric by one LSB.** `floor(-0.9 × (0.5/0.9) × 32767)` is −16384
  where the positive side gives +16383. That is inherent to flooring and the reference has it too, so
  tests bound peaks at `floor(ceiling × 32767) + 1` and say why, instead of asserting a rule no
  implementation follows.

## 7. Next steps this unlocks

1. **Get a verdict on the clips.** They cost nothing and they settle whether this tier of voice is
   what "very good" means — the open product question ahead of all engineering.
2. **Decide the pack's persona language.** `voice.json` gains `style_recipe` (sources + weights, or one
   voice), `speed`, `loudness_target_dbfs`, `peak_ceiling`. A persona then stays *auditable and
   re-derivable* instead of becoming an opaque binary — and it is exactly the format a trained vector
   (path C) must emit to be a drop-in replacement.
3. **Path C, when funded:** a 510×256 float32 matrix, accepted against the metrics above.
