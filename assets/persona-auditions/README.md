# Persona audition clips — reference audio, **not** a voice pack

Three ~23-second clips that establish what Tara, Kashi and Bobo are supposed to
*sound* like. They are the input to a training step, not the output of one.

| file | length | rate | ch | depth | size | RMS | sha256 |
|---|---|---|---|---|---|---|---|
| bobo-audition.wav | 24.2 s | 24000 Hz | mono | 16-bit | 1133 KiB | 0.109 | `91bed35cccb92b18011480351a4f3decd60f60092ccf1f5f963f8ef79c8fdd09` |
| kashi-audition.wav | 22.9 s | 24000 Hz | mono | 16-bit | 1073 KiB | 0.099 | `f3e6b3c32e95ab2bd49f1d88e5a24cd4bc2acf4cce7605ccf2213bcb7c435aa0` |
| tara-audition.wav | 23.6 s | 24000 Hz | mono | 16-bit | 1106 KiB | 0.113 | `7ee8c85be0b9745a463fa11267e60d7170000dfb0e6cb9de056a98349d19a031` |

PCM was measured from the files (not taken from the generator's description): mono
24 kHz 16-bit, RMS 0.099–0.113 with peaks 0.60–0.84, i.e. audible and not clipping.

## What these are for

`docs/ROADMAP_EMBEDDED.md` §5 lists three ways to obtain a voice:

* **Path B — clone from a short reference.** Zero-shot cloners (XTTS-class, and
  Kyutai Pocket TTS, MIT) accept a few seconds of reference audio and imitate its
  timbre. These clips are long enough (22.9–24.2 s) to serve as that reference. This is
  the cheapest legitimate route to "a voice of our own", and it is the reason these
  files exist.
* **Path C — commission an actor.** The clips double as the audition brief / target
  timbre to hand to a voice actor alongside the persona descriptions in
  `voice-packs/*/manifest.json`.

The scripts are derived from each pack's own persona description and intent profiles
(Tara: warm, professional Indian-English assistant; Kashi: calm, measured Hindi
narration; Bobo: playful, high-energy). Prosody was steered only through the *text*:
nothing in this repo applies `persona.default_rate`, `default_pitch`, or the
`intent_profiles` to audio yet, so these clips do not demonstrate those fields.

## What these are not

* **Not a `.cvpack`.** No `model.onnx`, no `voices.bin`, no inference graph. Nothing in
  `crates/` reads this directory; `chiti-voice verify` and `chiti-voice speak` behave
  exactly the same with these files present or absent.
* **Not proof of a working voice.** `vocal_core::REAL_SYNTHESIS_AVAILABLE` is still
  `false`: no neural inference is wired up, and the engine still refuses to synthesise
  rather than returning silence. See `docs/ROADMAP_EMBEDDED.md` Step 1.
* **Not a licensed asset you can sell.** These are synthetic speech rendered by a
  third-party text-to-speech model. There is no actor contract and no personality-rights
  clearance problem, but a machine-generated performance is also a weak basis for an
  exclusive voice right in most jurisdictions — and the *model's* own terms govern what
  you may do with its output. If the product needs a defensibly proprietary voice,
  Path C (recording + training with a signed contract) is the only one of the three that
  delivers that.
* **Not the shipping audio.** Even after Path B succeeds, the device artifact is an
  ONNX model built from a fine-tuned/conditioned checkpoint, at the pack's sample rate,
  validated by `scripts/build-voice-packs.py --require-real-models`.

## Provenance

* Generated 2026-09-03 with Arena Agent Mode's speech synthesiser (a commercial
  third-party TTS service); voices were chosen from a two-candidate audition per
  persona.
* Renderer output was accepted as-is: no normalisation, trimming, re-sampling or
  post-processing. The WAV containers are PCM with no metadata.
* The scripts are original copy written for this repo (no third-party text), recorded
  here so a re-render is reproducible:
  * **Tara** — greeting → order status → scheduling → reminder → reassurance, matching the
    `warm`/`greeting`/`alert`/`calm` intents.
  * **Kashi** — greeting → blessing → encouragement → grounding advice → availability,
    matching the `calm`/`guidance`/`knowledge` intents.
  * **Bobo** — celebration → urgency → reframing failure → encouragement → settle-down,
    matching the `excited`/`playful`/`encouraging`/`calm` intents.
* To get a different take, add a new file (e.g. `tara-audition-v2.wav`) instead of
  overwriting: the checksums in the table above are the reference others will compare
  against.
