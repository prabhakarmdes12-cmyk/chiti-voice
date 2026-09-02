# Offline synthesis spike — audio

Evidence, not product. These are the four utterances recorded in
[`docs/research/KOKORO_OFFLINE_SPIKE.md`](../../docs/research/KOKORO_OFFLINE_SPIKE.md),
synthesised locally: the weights were fetched once and every run below read files on disk.
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

## Provenance

| Field | Value |
|---|---|
| Acoustic graph | `kokoro-quantized.onnx`, 92,361,116 B, sha256 `fbae9257…` — int8 Kokoro-82M export |
| Channel | npm `expo-kokoro@1.1.9` tarball, sha256 `d4a8290008…` (HF and the GitHub release CDN are blocked where this ran) |
| Voices | `af_heart`, `bf_emma`, `hf_alpha` — 522,240 B each = 510 rows x 256 f32 style vectors |
| Phonemiser, two English clips + Hindi | espeak-ng (`en-us`, `en`, `hi`) via the `piper-tts` wheel — **GPL-3 data; do not link it from a non-GPL app** |
| Phonemiser, `open-phonemizer-en_us.wav` | 274,927-entry lexicon + 61 MB G2P graph from `expo-open-phonemizer@1.0.1` (MIT package) — no GPL component in that run |
| Reproduce | `scripts/fetch-offline-model.py`, `scripts/extract-open-phonemizer.py`, `scripts/spike-kokoro-offline.py` |

## What these clips are not

* **Not licensed for redistribution as a product voice.** The npm package's MIT licence covers
  its own TypeScript. The weights' terms are stated where Kokoro-82M is published, and the
  lexicon is 274,927 words of someone else's data. This repo's own rule
  (`VOICE_INV_008` / `PackValidator::require_provenance`) refuses to ship a `real` pack
  without that chain recorded — these bytes included, by design.
* **Not the personas.** `af_heart` / `bf_emma` / `hf_alpha` are the model's stock voices, kept
  as the baseline to beat. Direction for Tara/Kashi/Bobo stays in `assets/persona-auditions/`;
  a persona in this architecture is a 522 KB style vector, not a new model.
* **Not real-time.** RTF measured 1.79–2.35 on one thread of a 2-vCPU container. Nothing here
  demonstrates a Pi-class budget — that is still ROADMAP Step 2, to be measured on the board.
* **Not verified for pronunciation.** `open-phonemizer-en_us.wav` renders "Chiti" as
  `tʃˈaːɾi`: the G2P fallback guessed the product's own name. Recorded here because that is
  the finding — proper nouns need the pack-level override table the spec does not have yet.
