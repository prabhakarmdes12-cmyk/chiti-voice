# Offline synthesis spike — audio

Evidence, not product. The first four rows are the spike utterances recorded in
[`docs/research/KOKORO_OFFLINE_SPIKE.md`](../../docs/research/KOKORO_OFFLINE_SPIKE.md); the five
`persona-*` rows are measured casts recorded in
[`docs/research/PERSONA_STYLE_VECTORS.md`](../../docs/research/PERSONA_STYLE_VECTORS.md), synthesised locally: the weights were fetched once and every run below read files on disk.
Nothing in `crates/` can produce them yet — `REAL_SYNTHESIS_AVAILABLE` is still `false`.

They exist for two reasons: so a human can judge the quality the roadmap aims at (nobody had
listened to anything in this repo before), and so the numbers in that document are checkable
against bytes rather than against a claim.

| File | Length | Rate | RMS | Peak | sha256 |
|---|---|---|---|---|---|
| `af_heart-en_us.wav` | 5.33 s | 24000 Hz | 0.0685 | 0.500 | `6eb2b4f29d2fe27e…` |
| `bf_emma-en.wav` | 3.80 s | 24000 Hz | 0.0866 | 0.553 | `cb7b6d17e9723b46…` |
| `hf_alpha-hi.wav` | 4.85 s | 24000 Hz | 0.1535 | 0.987 | `e3d8edf3c67d46e7…` |
| `open-phonemizer-en_us.wav` | 5.88 s | 24000 Hz | 0.0688 | 0.559 | `f5cad7f0afc3fae8…` |
| `persona-tara.wav` | 5.50 s | 24000 Hz | 0.0891 | 0.698 | `7eeb9b57df380e96…` |
| `persona-tara-indic.wav` | 5.60 s | 24000 Hz | 0.0891 | 0.673 | `6b488844fc12d9a0…` |
| `persona-kashi.wav` | 3.98 s | 24000 Hz | 0.0750 | 0.745 | `e3bf6c0a24920eeb…` |
| `persona-bobo.wav` | 3.42 s | 24000 Hz | 0.0960 | 0.980 | `eed82f51e5a04d40…` |
| `persona-bobo-solo.wav` | 3.60 s | 24000 Hz | 0.1333 | 0.937 | `0adb5952e70beab9…` |

The `persona-*` peaks are the output of a deliberate loudness stage: RMS is normalised to each
persona's target and the peak is held at 0.98, because the 54-voice survey found eight voices above
0.9 on plain sentences and two clipping at 1.000.

## Provenance

| Field | Value |
|---|---|
| Acoustic graph | `kokoro-quantized.onnx`, 92,361,116 B, sha256 `fbae9257…` — int8 Kokoro-82M export |
| Channel | npm `expo-kokoro@1.1.9` tarball, sha256 `d4a8290008…` (HF and the GitHub release CDN are blocked where this ran) |
| Voices | `af_heart`, `bf_emma`, `hf_alpha`, plus 54 surveyed vectors — 522,240 B each = 510 rows x 256 f32 style vectors |
| Derived vectors | `persona-*.bin` are **not committed** (gitignored under `models/`): each is a weighted sum of stock vectors, i.e. a derivative work of weights whose licence is unverified. The recipes are committed in `docs/research/persona-recipes/` |
| Phonemiser, two English clips + Hindi | espeak-ng (`en-us`, `en`, `hi`) via the `piper-tts` wheel — **GPL-3 data; do not link it from a non-GPL app** |
| Phonemiser, `open-phonemizer-en_us.wav` | 274,927-entry lexicon + 61 MB G2P graph from `expo-open-phonemizer@1.0.1` (MIT package) — no GPL component in that run |
| Reproduce | `scripts/fetch-offline-model.py`, `scripts/extract-open-phonemizer.py`, `scripts/spike-kokoro-offline.py`, `scripts/survey-persona-voices.py`, `scripts/derive-persona-style.py` (metrics shared in `scripts/voice_metrics.py`) |
| Parity fixtures | `scripts/make-dsp-parity-fixtures.py` turns the float input of `af_heart-en_us.wav` into `crates/vocal-core/tests/fixtures/kokoro/dsp_parity.json`, which is how `vocal-core`'s floor and loudness rules get graded in CI with no model present |

## What these clips are not

* **Not licensed for redistribution as a product voice.** The npm package's MIT licence covers
  its own TypeScript. The weights' terms are stated where Kokoro-82M is published, and the
  lexicon is 274,927 words of someone else's data. This repo's own rule
  (`VOICE_INV_008` / `PackValidator::require_provenance`) refuses to ship a `real` pack
  without that chain recorded — these bytes included, by design.
* **Not the personas, even the ones named after them.** `af_heart` / `bf_emma` / `hf_alpha` are
  stock voices kept as the baseline to beat. The `persona-*` casts are measured approximations built
  from those stock vectors: only `speed` is a real engine input, four of the five documented
  persona parameters have no input at all, and a blend is not a speaker. Commissioned direction stays
  in `assets/persona-auditions/`; a persona in this architecture is a 522 KB style vector, not a new
  model, which is why path C is the route to one we can own.
* **Not real-time.** RTF measured 1.79–2.35 on one thread of a 2-vCPU container. Nothing here
  demonstrates a Pi-class budget — that is still ROADMAP Step 2, to be measured on the board.
* **Not verified for pronunciation.** `open-phonemizer-en_us.wav` renders "Chiti" as
  `tʃˈaːɾi`: the G2P fallback guessed the product's own name. Recorded here because that is
  the finding — proper nouns need the pack-level override table the spec does not have yet.
