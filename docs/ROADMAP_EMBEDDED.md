# Roadmap Revision — Real Voice, Offline, On Device

Classification: PLAN (supersedes the phase order in `PRD.md` for the goals below)
Date: 2026-09-03
Owner decision required: backend selection (`ADR-001` is still `PROPOSED`)

---

## 0. The constraints this plan is built against

Stated goals for Chiti Vocal Runtime:

1. **A voice you can connect to your own projects** — a callable surface (library, daemon,
   SDK), not a demo.
2. **Very good voice quality.**
3. **Runs on any device**, with **offline mandatory** (no network call in the synthesis
   path, ever — `VOICE_INV_001`).
4. **Embedded targets**: Raspberry Pi-class, robot, toy.

These four cannot all be maximized. Not because of this codebase, but because of
arithmetic. The rest of this document is about choosing which constraint to bend, per
device tier, and getting to *audible* as fast as possible.

**Nothing in this repository currently produces sound.** `MockEngine` emits silence,
`PiperEngine` is a voice registry with a `TODO`, no `.cvpack` contains a model, and
`vocal_core::REAL_SYNTHESIS_AVAILABLE == false`. Treat everything below as "the work that
has to happen", not "the work that mostly happened".

---

## 1. The numbers that decide the design

Third-party on-device benchmark (Picovoice, 2026 — desktop CPU, single core; measure on
your real hardware before committing, but the relative ordering is what matters)
[2](https://picovoice.ai/blog/on-device-tts/):

| Engine | Model size | Peak memory | Time to first sample | Streaming out | License |
|---|---|---|---|---|---|
| Piper TTS | 61 MB | 2.6 GB | 1,720 ms | yes | MIT (engine), **per-voice** (models) |
| Kokoro-82M | 341 MB | 2.0 GB | 3,658 ms | yes | Apache-2.0 |
| Kyutai Pocket TTS | 242 MB | 610 MB | 1,713 ms | yes | MIT |
| KittenTTS Nano (int8) | 42 MB | 320 MB | **10,483 ms** | **no** | Apache-2.0 (see §3) |
| Picovoice Orca | 7 MB | 41 MB | 106 ms | yes | **proprietary/commercial** |

### Measured in this repo, not quoted (2026-09-03)

[`docs/research/KOKORO_OFFLINE_SPIKE.md`](./research/KOKORO_OFFLINE_SPIKE.md) synthesised real speech
with the actual graph, in the sandbox, from weights pulled off npm because Hugging Face is
blocked here. Against a model *in this family* (int8 Kokoro-82M, 88.1 MiB, vocoder fused into
the graph, 24 kHz mono):

| Item | Measured | Against the table above |
|---|---|---|
| Model size | 88.1 MiB (int8 export) | the blog's 341 MB is the fp32 Kokoro export — quantisation is ~4×, and it is *free* to adopt |
| Peak RSS | 284 MiB (Python, one thread) / 334 MiB (with the G2P model too) | "2.0 GB" is the fp32 Python story; a Rust/`ort` build should be nearer the model size, but that number is still unmeasured on a board |
| RTF, 1 thread, nproc=2 | 1.79 (en) · 1.86 (en, permissive G2P) · 2.35 (hi) | >1 ⇒ **not** real-time on a single modest core; sentence-at-a-time, not stream-and-play |
| Session load | 0.57–0.75 s | matters for a robot that boots cold |
| Graph contract | `input_ids` i64[1,L] + `style` f32[1,256] + `speed` f32[1] → `waveform` f32[1,N] | **no separate vocoder session** — Step 1's two-session plan collapses to one for this export |
| Voice asset | 522,240 B = 510 × 256 f32 per speaker | a *persona* is 0.5 MB; the model is shared. See §5 |
| Output level varies by voice | peak 0.50 (af_heart) → 0.99 (hf_alpha), same graph | a device player needs loudness normalisation; 0.99 is one hot voice from clipping |

Two of those change the design immediately: the fused vocoder (less to port) and the per-voice
peak spread (a required post-stage, not an optional one).

Quality context from the same survey of public comparisons
[5](https://www.promptquorum.com/power-local-llm/local-tts-voice-cloning-piper-coqui-xtts),
[2](https://picovoice.ai/blog/on-device-tts/):

| Model | Approx. naturalness (MOS, English) | Note |
|---|---|---|
| Human reference | ~4.5 | ceiling |
| Kokoro-82M | ~4.0–4.2 | "very good per parameter", big |
| Piper VITS | ~3.5 | "good", intelligible, flat, tiny-ish |
| KittenTTS | small-but-surprisingly-natural for its size | English-focused, no streaming |
| Neu-TTS-Nano / XTTS-v2 / F5-TTS | varies | XTTS v2 (CPML) and F5-TTS (CC-BY-NC-4.0) are **non-commercial** — unusable for you |

Read those two tables together and the design conclusions are forced:

- **"Very good" and "20 MB, fully offline, on a toy" is not currently available for free
  from anyone.** The 20 MB target in `docs/research/20MB_CHALLENGE.md` with MOS > 4.0 is a
  research program (quantization, distillation, shared backbone + speaker adapters), not a
  phase. It is a 2027-sized goal, not a 2026-sized one.
- **Real-time interactive voice needs streaming synthesis.** Kitten Nano's 10 s time-to-first-sample
  and no-streaming behaviour makes it fine for "read this notification" and wrong for
  "conversation with a robot". That is a product-fit decision, not a quality one.
- **Peak memory is the real embedded constraint, not model size.** Piper's 61 MB model
  wanting 2.6 GB peak (because of ONNX Runtime arenas + espeak-ng data + phoneme caches) is
  the single most important number in this table for a Pi Zero/toy target. It must be
  measured and *configured*, not assumed.
- **A Raspberry Pi 4/5 (2–4 GB) can run this. A microcontroller (ESP32-S3 class) cannot run
  any of these models.** For an ESP32-class toy, the honest architecture is: pre-generated
  clips + concatenative/unit-selection playback for a fixed phrase set, or a small
  vocoder-driven approach, with neural TTS on a companion device (phone/Pi/edge box). If a
  battery toy must speak arbitrary text offline, that is the actual research project here.

### Recommended target tiers

| Tier | Hardware | Engine choice | Quality | Why |
|---|---|---|---|---|
| **T0 (ship first)** | Desktop/server, Pi 4/5 (2 GB+) | **Piper** (VITS, 22k/medium) | ~3.5, intelligible | MIT engine, tiny models, streams, espeak-ng path proven on Pi; the fastest route to *audible* |
| **T1 (quality)** | 4 GB+ RAM, modern CPU | **Kokoro** or **Pocket TTS** | ~4.0+ | Apache/MIT, much better prosody; Kokoro too heavy for small boards |
| **T2 (clone/persona)** | Same as T1 | **Pocket TTS voice cloning** from a short reference | inherits model | MIT, 242 MB, ~5 s reference → the cheapest legitimate route to "our own voice" without training a model from scratch |
| **T3 (toy/MCU)** | ESP32-S3 etc. | **No neural TTS.** Clip set + concatenative, or delegate to a paired device | n/a | Do not promise offline arbitrary-text speech here |

`ADR-001` recommends Piper; that recommendation still holds **for T0**, and should be
accepted with the licensing conditions in §3 written into it.

---

## 2. Re-sequenced plan (each step has a binary exit test)

The PRD ordered this as *backend → daemon → SDK → size research*. For your goals the
ordering above buries the two hardest things (audible quality, device fit) behind a year of
surface area. Re-sequence:

**Step 0 — truth + build (done 2026-09-03, this commit).**
Compilable workspace, honest docs, `REAL_SYNTHESIS_AVAILABLE`, enforced pack limits,
`docs-truth` CI gate. Exit: `cargo build --workspace --all-targets` green; CI green.

**Step 1 — ONE real sentence of audio (days, not quarters).**
*Status 2026-09-03 (later the same day): the deterministic half has landed. Everything the engine
must do around the graph is now Rust and CI-tested — `style_matrix.rs` (510×256 LE decode, the
row-index rule, exact-size rejection), `audio_levels.rs` (floor conversion + loudness with a peak
ceiling and a max-gain cap), and `tests/dsp_parity.rs`, which grades both rules against
`tests/fixtures/kokoro/dsp_parity.json`: the raw float32 output of the very run that produced
`assets/offline-spike/af_heart-en_us.wav`, so passing means the crate reproduces committed reference
audio sample-for-sample with no tolerance anywhere. That also fixed a live bug: `wav.rs` scaled with
`.round()` where the reference floors, i.e. the "already implemented and tested" item 4 below was
**not** bit-compatible with our own evidence. What remains is the `ort` session call itself, which
needs a machine with crates.io; CI compiles them under `--all-features` and *runs* them: `Unit Tests` on
all three OSes is green at `56f13b3`, with the parity assertions among them. One test failed on the
first push and it was my hand-written expectation, not the rule (`-1.0` floors to -32767, so the
bottom int16 rail is unreachable from in-range audio) — which is what grading against data is for.*
Accept `ADR-001` (Piper for T0). Then, in `PiperEngine` behind `--features piper`:
1. `ort::Environment` + `Session` load from bytes already in the `.cvpack` (never a path
   fetch — `fetch-models` stays off; it is now removed from the workspace manifest).
2. Piper's input contract: phoneme ID sequence + speaker ID + length/scale/noise tensors →
   mel → **vocoder** session → PCM. Two sessions (acoustic + vocoder) or one fused export;
   decide and record it in an ADR.
3. Phonemization: see §3. This is the hard part, not the ONNX part.
4. Write PCM through `vocal_core::wav` (already implemented and tested).
- **Exit test:** a CI job runs `chiti-voice speak --voice tara "Hello" --engine piper` on a
  *real* model and asserts (a) exit 0, (b) PCM RMS > threshold, (c) duration within ±20% of
  expected. Then flip `REAL_SYNTHESIS_AVAILABLE = true`. `docs-truth` then *requires* the
  docs to stop saying "No audible voice" — the flag keeps prose and code locked together.
- Also add: `cargo test --benches` RTF measurement vs. wall-clock playback length.

**Step 2 — device fit, before any API surface.**
On the actual hardware: RSS, time-to-first-sample, RTF, cold-start model load, pack size.
Then set `PackLimits` per tier from measurements (the `embedded()`/`tiny()` profiles exist
now and are guesses to be replaced by data). Decide the memory story for ONNX Runtime
arenas, and whether `mmap` loading of `model.onnx` is acceptable (it changes the pack format:
external-data files must then be declared in the manifest, which the allowlist already supports).
- **Exit test:** published table of measured numbers per device; a documented `MIN_HW` and a
  startup budget (< 300 ms warm, < 2 s cold is a reasonable target to argue about).

**Step 3 — the surface you actually wanted: daemon + SDK.**
`vocal-local` Axum daemon on `127.0.0.1:7731` implementing `docs/api/HTTP_API.md`
(`/v1/speak`, `/v1/health`, `/v1/stop`), chunked PCM streaming, cancellation within 100 ms
(FR-025/034), and `@chiti/voice-web` on top. This is where "connect it to my projects"
becomes true. Keep `SynthesisResponse` as the wire contract so the CLI and daemon share one
engine path.
- **Exit test:** a browser page and a Node script speak the same sentence with the network
  disconnected (DevTools offline + no outbound route), and `stop()` silences in < 100 ms.

**Step 4 — the two invariants that make it a product, not a toy library.**
- Text normalization for Indian English and Hindi (₹ lakh/crore, DD/MM/YYYY, phone numbers,
  "Dr./Shri./IAS/ISRO", numerals → words). This is where TTS products actually fail, and it
  is currently a `Ok(text.to_string())` stub. Own it deliberately (a small rule engine + a
  pronunciation dictionary), not as Phase 6 filler.
- Provenance/licensing gate per pack (already started: the builder refuses to emit a
  non-placeholder pack with incomplete provenance, and CI checks it).

**Step 5 — persona runtime wiring, then quality research.**
Intent → prosody applied by the engine, not just parsed — and the manifest now says only what can be
applied: `rate` goes to the graph's `speed` input, `energy` becomes a dBFS offset on the loudness
stage, `pause_factor` becomes inserted silence, and `pitch` is realised by **cast selection** (a pack
must set `pitch_baked_into_style` or the value is rejected at load). `warmth`/`expressiveness` stay out
of the manifest entirely; they are documented, not dialled (§6).
Then, and only then, the `20MB_CHALLENGE` research track (quantization/distillation, shared
backbone + adapters), and the browser-native WASM/WebGPU question — which is a *separate
engineering problem* (§4), not a later phase of this one.

Steps 1–2 are where the product either exists or doesn't. Everything after that is surface.

---

## 3. The licensing trap, stated plainly

`LICENSE` now carries the table; this is the short version, because it can invalidate the
whole plan rather than a line item.

- **Piper (engine) = MIT** → fine to embed commercially.
- **Piper voices are licensed per model.** The engine being MIT says nothing about the
  `.onnx` weights; individual model cards differ (CC0, CC-BY, CC-BY-SA, and some
  non-commercial terms) [1](https://ithub.global.ssl.fastly.net/estebanstifli/LocalText2Voice/blob/main/THIRD_PARTY_NOTICES.md),
  [3](https://huggingface.co/agentvibes/piper-custom-voices). "Apache 2.0", which the
  placeholder manifests in this repo previously asserted for a file that did not exist, is
  not a safe default and has been removed.
- **The wheel, not just the tool.** Measured: `piper-tts` 1.7.0's own packaging metadata says
  `License: GPL-3.0-or-later` and ships `espeak-ng-data` (125 entries) inside the wheel — so
  "Piper is MIT" is true of the engine and false of the artifact you would install. Same for
  the phonemiser in any stack that bundles espeak's data. **The phonemiser is the copyleft
  boundary in this product, not the ONNX graph.**
- **espeak-ng = GPL-3.0.** Piper uses it for phonemization/G2P. Shipping it *inside* a
  proprietary distributed binary is the classic way to acquire a source-disclosure
  obligation for your own binary. Options, in order of cleanliness: (a) use a non-GPL G2P
  path — **now measured, not speculated:** `expo-open-phonemizer@1.0.1` (MIT) pairs a
  274,927-entry `en_us` lexicon with a 61 MB char-level G2P graph, and
  `scripts/extract-open-phonemizer.py` + `scripts/spike-kokoro-offline.py --phonemizer open`
  synthesise a sentence through it with no GPL component in the path. Its limits are real and
  stated: `en_us` only (so `hi` still needs espeak-ng or our own G2P), no licence stated for
  the weights inside the tarball, and it mispronounces out-of-lexicon proper nouns —
  "Chiti" came out `tʃˈaːɾi`. KittenTTS 0.8's learned G2P and Kokoro's `misaki` route still
  need the same independent verification,), (b) ship espeak-ng as a separate process/package with its
  own license boundary, (c) accept GPL for that component and structure accordingly. **Make
  this a written decision in `ADR-002` before Step 1 finishes.** Note the same trap exists
  for KittenTTS's *reference Python package*, which has been reported to pull GPL-3.0
  `phonemizer` even though the project is Apache-2.0
  [4](https://news.ycombinator.com/item?id=44807868) — i.e. *engine license ≠ artifact
  license ≠ frontend license*. Audit the whole path, in the language you will ship.
- **Non-commercial-only models to avoid**: XTTS v2 (CPML), F5-TTS (CC-BY-NC-4.0)
  [5](https://www.promptquorum.com/power-local-llm/local-tts-voice-cloning-piper-coqui-xtts).
  Both are popular in "clone a voice" tutorials. Both are unusable for a paid product.
- **Hindi.** Don't assume parity with English. Check what exists for `hi-IN` in Piper's
  catalog and in AI4Bharat's IndicTTS line, and evaluate MOS before promising KASHI to
  anyone.

A 30-minute license audit per candidate now is cheap. After Step 1 it becomes either a
legal conversation or a re-platform.

---

## 4. "Any device" — where the current architecture silently fails

`Rust + ort` gets you: Linux x86_64/ARM64/RISC-V, Windows, macOS. **That is not "any
device."** It does not get you browsers, and it does not practically get you Android/iOS,
because ONNX Runtime's distribution story, WASM size, and mobile toolchains are different
projects, not build flags. The PRD pushes browser-native to Phase 12, which is backwards if
a browser or phone is a real target.

Decide now, per device you care about:

| Target | Feasible path | Cost |
|---|---|---|
| Desktop app / server | link `vocal-core`, or loopback daemon | low (Steps 1–3 as planned) |
| Raspberry Pi / robot | same binary + `PackLimits::embedded()` + measured budgets | low–medium |
| Web page, no install | `onnxruntime-web` WASM/WebGPU, model in a service-worker cache; **or** a hosted daemon you do not control | high; a separate engine path, own quantization, own 20 MB-plus-size problem |
| Android / iOS | native engine (ONNX Runtime mobile / platform TTS / a C++ core with FFI), plus per-OS audio and packaging | high; effectively another project |
| ESP32 / MCU | no neural TTS; clip set or delegate to a paired device | medium, but a different product |

The `.cvpack` idea still holds across these (it is a container + manifest + provenance), and
`ort`-vs-`onnxruntime-web` is exactly what the `VoiceEngine` abstraction was for. But the
plan must say **which** of those rows is in scope for v1. My recommendation: v1 =
desktop + Pi via the daemon, and treat browser as a parallel spike (one page, one
quantized model, WASM) *before* investing in Voice Lab or signing, because it is the row
most likely to change the pack format and the size budget.

---

## 5. "Generate the voice for me" — what that can actually mean

A `.cvpack` voice is not an audio file; it is a model plus metadata. So "generate a voice"
resolves to one of three things, with very different costs:

| Path | What you get | Cost | Notes |
|---|---|---|---|
| **A. Adopt an existing open voice** | A real, offline, working persona today (e.g. a Piper `en-IN`/`en-GB` voice; Kokoro/Pocket voices for better quality) | days | Fastest. But it is *their* voice, not your brand voice, and "Tara" becomes a label on someone else's timbre. Verify each model card (§3). |
| **B′. Derive a style vector** (Kokoro-class engines only) | A new persona as a **522 KB** style matrix (510 × 256 f32) against the shared 88 MB graph — which is also why one device can carry the whole roster | days–weeks, one GPU session | Added after the spike, because it changes this table's premise: for this engine family a *voice* is not a model. Caveat found while reading the reference code: the style row is selected by utterance length, so prosody depends on how the daemon chunks sentences (§2 Step 3). Interpolating two existing vectors is cheap and yields a "new" timbre, but it is a blend, not a speaker — say so in any manifest. **Measured 2026-09-03** ([`PERSONA_STYLE_VECTORS.md`](./research/PERSONA_STYLE_VECTORS.md)): all 54 stock vectors surveyed; blending interpolates register (0.4–3.7 % off the weighted mean) but *attenuates pitch range below every source*, so it suits restrained personas and damages expressive ones — a cast is therefore `selection + speed + gain`, and `scripts/derive-persona-style.py` implements it. |
| **B. Clone/fine-tune toward a target** | A persona that sounds like a reference speaker (**reference clips exist now:** `assets/persona-auditions/*.wav`, 22.9–24.2 s mono 24 kHz — see §5.1) | weeks + GPU | Pocket TTS clones from a very short reference under MIT [8](https://getstream.io/blog/best-on-device-tts-models/) — the cheapest legitimate route to a distinctive voice. Kokoro fine-tuning is Apache-2.0-friendly. Model size/latency then follow the base model, i.e. T1 tier, not a toy. |
| **C. Commission a speaker + train** | Ownable, consistent, licensable brand voice; satisfies INV_008 properly (consent contract, terms of use, term length, territory) | months + money + a dataset pipeline | The only path that yields a *product asset* you can license onward — which is what a `.cvpack` business model presumes. |

**The constraint on this workspace** (updated by the spike): `crates.io` is still
unreachable, so **there is still no Rust build verification from here** — that part is
unchanged and is why `crates/` cannot speak yet. But *"no model download"* was false: the
npm registry mirrors the weights, so a measured contract, a reference fixture and four
listen-test clips could be produced here after all. Re-read that as a lesson about this
repo's habit of accepting a blocker as given. What is possible from here:
writing and statically reviewing the ONNX/phonemization code, all the pack/CI/docs/tooling
work (done), and producing **audition audio to fix the persona direction** — which is the
prerequisite for both B and C, and is the part of "generate the voice" that can actually be
decided today. Choose the direction, then A gets you speaking this week while B/C is
commissioned in parallel.

---

### 5.1 Reference audio that already exists (2026-09-03)

`assets/persona-auditions/` holds one ~23 s clip per persona (tara, kashi, bobo), with
measured properties and checksums in its README. They were produced by a third-party
text-to-speech service, so they are:

* **usable as path B input** — zero-shot cloners take a few seconds of reference and imitate
  the timbre; 23 s is more than enough, and each clip already covers that persona's intent
  profiles (warm/greeting/alert/calm for Tara, calm/guidance/knowledge for Kashi,
  excited/playful/encouraging/calm for Bobo);
* **usable as path C's brief** — the thing to hand an actor and say "this, but yours";
* **not** a `.cvpack`, not a model, not loadable by anything in `crates/`, and not evidence
  that `REAL_SYNTHESIS_AVAILABLE` should be true;
* **not a licensable voice asset.** A machine-generated performance gives no actor contract to
  point at and, in most jurisdictions, weak grounds for an exclusive right in the *timbre*
  itself; the rendering model's own terms also govern the output. If the product needs a voice
  it can license to others, path C is the only one of the three that produces that.

Re-render rather than overwrite if a different take is wanted, so the checksums stay
meaningful.


## 6. Known gaps deliberately left open by this commit

- Four of the five parameters in `docs/personas/*.md` (Pitch, Energy, Warmth, Expressiveness) are
  **not engine inputs** — verified against the ONNX graph, which accepts `input_ids`, `style`, `speed`
  and nothing else. They are now documented as casting/post-processing targets rather than dials
  (`PERSONA_STYLE_VECTORS.md` §1), and the manifest half of that decision is *closed*: `.cvpack`
  `persona` blocks state only what can be honoured, and `PackValidator` rejects the rest at load
  (rate band 0.5–1.6, energy as 0–1 mapped to a dBFS offset, `default_pitch` only with
  `pitch_baked_into_style`, no per-intent pitch). Still open, and a product call rather than an
  engineering one: whether `warmth` and `expressiveness` stay in the specs at all, given that they
  have no implementation and no proposed one — they are recorded in the persona docs and the recipe
  JSONs in the meantime.
- `zip = "0.6"` — two majors behind (0.6 → 2.x renamed the write/read APIs). Not upgraded
  blind because nothing here can compile-check it; do it in a PR with `cargo test` running.
- `security.rs` returns `Result<(), String>`; `error.rs` therefore maps pack failures to
  `VoiceErrorCode` by keyword. Replace with a typed `ValidationError` for exact mapping.
- `VoiceCapabilities.supported_formats` still advertises `ogg` with no encoder anywhere; the
  CLI errors honestly. Either implement a container (opus) or narrow the capability list.
- `docs/api/*.md` describe unimplemented surfaces. They are specifications, marked as such in
  the README table; they are not evidence of anything.
- No audio-device playback in the CLI (`rodio` etc. not added: dependency bar in `AGENTS.md`).
- `Cargo.lock` is still absent — it must be generated on a machine with crates.io access and
  committed. CI's `supply-chain` job now fails if it is missing.
- `.cvpack` can now express three of the four things the measured engines need: where the
  510 × 256 style matrix comes from (`persona.style`: one stock voice, a bounded blend, or a packed
  file of exactly 522,240 bytes), a bounded output-loudness stage (`persona.loudness`), and a
  `pronunciation_overrides` map for proper nouns (`chiti` → `ˈtʃɪti`, which is what the permissive
  graph otherwise mangles). Packs are generated from the spec docs by
  `scripts/sync-persona-manifests.py` (`--check` is wired into `ops/ci/`; the live Phase 1 workflow's
  job list is fixed). Of the two gaps recorded there, the more dangerous one is now closed at the
  engine level: `vocal_core::utterance_plan` owns the chunking policy, because the style row is selected
  by utterance token count and so how the daemon splits sentences *is* the prosody. Its default ceiling
  is 509 tokens — one below what `encode` truncates at — which makes truncation of a long request
  structurally impossible instead of silent, and `PlanPolicy` travels with every `Plan` so a rendering
  can be reproduced. What remains open is the manifest half: no pack yet says which policy it was tuned
  under, and `persona.chunking` is not a field, so two builds with different defaults would still sound
  different from the same pack. The other gap also stands: **no tokenizer slot** (the 115-symbol table is
  still an out-of-band file).
- `encode` and the upstream tokenizer disagree about characters outside the 115-entry vocab, and the
  disagreement is audible. Upstream maps an unmapped character to a `PAD` *token* (`vocab.get(c, pad)`),
  keeping the sequence's length; `phoneme_tokens::strip_to_vocab` deletes it, shortening both the id list
  and — because the row index *is* the token count — the style row. The concrete exposure is this
  vocabulary's shape: it carries U+0261 (script-g) and not ASCII `g`, which is what espeak-style IPA
  emits for /g/, so a phonemiser without that mapping loses every 'g' silently and shifts prosody by the
  same amount. `tests/kokoro_tokens.rs` already guarantees the *fixtures* never exercise the difference
  (they assert no stripping is needed at all), which is a fence, not an answer. The engine commit picks
  one: be faithful to upstream, or refuse unknown input loudly. Doing neither is the status quo, and the
  doc comment that described the first while the code did the second has now been corrected.

- Loudness normalisation exists and is measured, but nothing that produces audio uses it yet:
  `vocal_core::audio_levels` implements the `target_dbfs` / `peak_ceiling` / `max_gain_db` stage the
  manifest declares, `tests/dsp_parity.rs` grades the i16 scaling against real graph output, and the
  persona runtime maps intent energy onto it. The gap is that only the mock path runs it — which is
  Step 2's job, not a schema job. The spike's reason for the stage is unchanged: peak ranged 0.50–0.99
  across voices on one sentence, and 0.99 is one hot voice from clipping.
- `MOCK` engine's silence-as-valid-audio design invites exactly the false-completeness this
  repo had. Consider making `MockEngine` refuse unless `#[cfg(test)]` or an explicit
  `--allow-silence`, and deleting the "produces audio" framing from all future docs.
