# Offline synthesis spike — measured 2026-09-03

Answers one question: **can this project actually produce speech on a device, offline, with
the network cable physically unplugged — and at what cost?** Not from a vendor blog this
time. Every number below came out of a run in the sandbox this repo was audited in, and the
scripts that produced it are in the tree.

What is *not* true afterwards, and is worth saying first: **nothing in `crates/` can speak
yet.** `REAL_SYNTHESIS_AVAILABLE` is still `false` and `PiperEngine` still refuses. What
changed is that Step 1 stopped being speculative — the model I/O contract, the tokenizer,
the voice-vector layout and a measured reference are now pinned in
`crates/vocal-core/tests/fixtures/kokoro/`, so the Rust engine has something to be graded
against instead of a prose description.

## What was run

```bash
python3 scripts/fetch-offline-model.py --accept-licence      # -> models/  (88 MB, gitignored)
python3 scripts/extract-open-phonemizer.py --accept-licence  # -> models/phonemizer/
python3 scripts/spike-kokoro-offline.py --model-dir models --controls \
    --text "This sentence was synthesised on a single board, with the network cable unplugged."
```

Why those fetch scripts exist at all: **Hugging Face is unreachable from here** — as are
`cdn-lfs.huggingface.co` and `objects.githubusercontent.com`, so even `gh release download`
fails. The npm registry is an ordinary package mirror, and `expo-kokoro@1.1.9` ships the
quantized Kokoro graph and its voice vectors *inside its tarball*. That makes the weights
obtainable in exactly the environments where this product has to build.

Runtime: `python3 -m venv` + `pip install onnxruntime numpy` (both reachable), espeak-ng via
the `piper-tts` wheel's bundled `espeakbridge.so` + `espeak-ng-data`.

## Measured

| Item | Value |
|---|---|
| Model file | `kokoro-quantized.onnx`, 92,361,116 B = **88.1 MiB**, int8 |
| Graph I/O | `input_ids` int64[1,L], `style` float32[1,256], `speed` float32[1] → **`waveform` float32[1,N]** |
| Vocoder | **inside the graph** — no second session, no mel stage to port |
| Sample rate / channels | 24,000 Hz mono (matches the persona audition clips) |
| Session load | 0.57–0.75 s (1 thread) |
| English, 86 tokens | 5.33 s audio, 9.55 s inference → **RTF 1.79** |
| English (open-phonemizer path), 98 tokens | 5.88 s audio, 10.91 s → **RTF 1.86** |
| Hindi (`hf_alpha`, 72 tokens) | 4.85 s audio in 8.43 s → **RTF 1.74**; same graph, no special path |
| Hindi, OOV-short variant (48 tokens) | 3.58 s in 8.43 s → RTF 2.35 in the F0 control loop (different sentence; kept to show the spread) |
| Peak RSS, Python process | 284 MiB (one model) / 334 MiB (model + G2P) |
| Output audio | rms 0.0685–0.1535, **peak 0.50–0.987 depending on voice**, 27 % near-silent samples |
| Per-voice output level | af_heart 0.500 · bf_emma 0.553 · open-phonemizer run 0.559 · hf_alpha **0.987** — same graph, same speed: a device needs loudness normalisation, or one hot voice clips |
| Voice asset | 54 files × **522,240 B** = 510 rows × 256 × f32 |
| Tokenizer | 115 symbols, **ids sparse in 0..=177**, `model_max_length` 512 |
| CPU | container with **nproc=2**; `intra_op_num_threads=1` unless stated |

`RTF > 1` means this export does **not** run in real time on this machine, single-threaded.
Do not read that as "Kokoro is too slow for a Pi": it reads as "one thread of an unknown
shared cloud core is roughly a Pi-4-class core, and fp32→int8 Kokoro at this sentence length
is a *near*-real-time, sentence-at-a-time engine". The follow-up measurement that matters is
on the actual board with `ort` (Rust), per-thread, with first-audio latency for the *first
sentence*, not the whole paragraph.

## Controls: why the audio counts as evidence

RMS > 0 proves nothing on its own — a stuttering hiss has RMS. The spike therefore runs
falsification checks (`--controls`), and the reasoning is worth keeping:

| Check | Result |
|---|---|
| `speed` 0.5 / 1.0 / 1.5 / 2.0 | 10.60 / 5.33 / 3.35 / 2.88 s — duration tracks 1/speed ✓ so `speed` is wired |
| `style` zeroed | output changes (rms 0.26) ✓ so the voice vector is read |
| `input_ids` = pads only | collapses to 1.88 s ✓ so the text is read |
| different voice `.bin`, same text | F0 207 / 141 / 216 / 242 Hz for af_heart / af_nicole / bf_emma / hf_alpha ✓ |
| row-selection sanity | af_nicole is both lower-pitched **and** 8.35 s vs 5.33 s for the same sentence, matching that voice's known slow delivery — a wrong style row would not reproduce that pairing |

The F0 + duration agreement across voices is the strongest cheap signal that
`style_data[n_tokens * 256 : n_tokens * 256 + 256]` — the single most mysterious line in the
reference implementation — is being indexed the right way.

## Four findings that change the plan

**1. A persona is a 522 KB float matrix, not a model.** Kokoro's per-speaker asset is 510
style rows × 256 f32; the 88 MB graph is shared. So "generate me a voice" (ROADMAP §5) is
not a 300 MB per-voice training problem: the deliverable of path B or C is a small vector
plus a manifest, and **a device can host the whole persona roster for a few MB**. It also
explains a behaviour the reference code hides: the row is selected by *utterance length*, so
one long sentence and the same sentence split into three pick different rows — prosody
follows phrasing length. That is a real constraint on how `vocal-local` should chunk text.

**2. The copyleft boundary is the phonemiser, not the engine.** `piper-tts`'s own wheel
metadata says `License: GPL-3.0-or-later`, because it bundles espeak-ng's data, and the
Kokoro reference implementation likewise leans on an espeak-ng port. The ONNX graphs and
voice vectors are not what forces a licence. A measured permissive alternative exists:
`expo-open-phonemizer@1.0.1` (MIT) = 274,927-entry `en_us` lexicon + a 61 MB char-level G2P
graph — and the committed script drives it end to end (`--phonemizer open` produced the Hindi
sentence's English-side sibling above with no GPL component in the path). Caveats recorded in
§3 of the roadmap: the tarball states no licence for the *weights* or the lexicon's terms, and
the graph covers `en_us` **only** — Hindi/Tamil/etc. still need espeak-ng or our own G2P.

**3. Do not copy the reference implementation blindly — one of its paths is broken.**
Its `_phonemize` reads `results["output"]`, but the graph exports `logits`, so any word
missing from the lexicon throws. And its `decode()` returns the raw argmax, which for a
`char_repeats = 3` model looks like `tt_ʃʃʃˈaː__ɾ__i`: the blanks and the per-char repeats
have to be collapsed (`tʃaːɾi`) before the synthesiser sees them. A port that skips the
collapse gets audio that is fluent, mispronounced and **not** detectable by any RMS check —
so both rules are written into `scripts/extract-open-phonemizer.py`'s output
(`decode_rules`) rather than left in a comment.

**4. Proper nouns need an override table, which the pack spec does not have.** "Chiti" is
out-of-vocabulary, so the G2P graph guessed: `tʃˈaːɾi` — a plausible-sounding mangling of
the product's own name. Any deployment will hit this on the first brand name, city or
customer name. The fix is cheap (a `pronunciation_overrides` map in the pack, consulted before
both the lexicon and the graph), but it must exist *before* persona work, or every voice will
mispronounce the same words in the same way.

**5. `speed` is not a linear time control at the edges.** 0.5 → 10.60 s (1.99× ✓), 1.5 →
3.35 s (0.63×, expected 0.67×), 2.0 → 2.88 s (0.54×, expected 0.50×). The length regulator
quantises, so a UI that promises "2× speed" will be off by ~8 %. Minor, but the sort of thing
that becomes a bug report.

**6. A permissive path exists for English only — and it was measured, not proposed.** Two
English clips in `assets/offline-spike/` came from espeak-ng; one came from
`scripts/extract-open-phonemizer.py`'s lexicon + G2P graph, with no GPL component in the run.
`hi`, `ta`, `bn`, `mr`, `pa` were verified to work through espeak-ng's data and through
**nothing else** — so the licence decision in §3 has a clean answer for the personas that
speak English and an open question for KASHI.

**7. Characters outside the 115-symbol vocab are silently dropped — and that is upstream
behaviour, not a bug I introduced.** `encode` applies the whitelist, so an espeak IPA symbol the
table lacks (e.g. `ɫ`, dark *l*, which `en-us` does emit) disappears before synthesis rather than
raising an error. The reference fixture happens to need no stripping — all 84 characters of its
phoneme string are in the vocabulary, which the `reference_phonemes_are_already_canonical` test
asserts — but nothing in the pipeline *guarantees* that for the next sentence. A real
`hi`/`en-IN` G2P mapping (ROADMAP §2 Step 4) is therefore not a polish item: it is the component
that decides whether phonemes arrive complete.

## What this does not prove

* **Not that it is good.** No human listened for quality; MOS is unmeasured. The clips in
  `assets/offline-spike/` are for you to judge — that is what they are for.
* **Not that Rust/`ort` reproduces it.** No Rust inference code has been compiled anywhere;
  this sandbox has no toolchain. The fixture is the grading target, deliberately tight
  (`samples ±2 %`, `rms ±40 %`, not bit-exact — quantized weights across ORT builds/threads
  don't reproduce float accumulation order, and asserting equality there would test one
  runtime, not the engine).
* **Not device fit.** 284–334 MiB is *Python*'s peak with numpy + ORT loaded; an `ort` build
  will be lower, and a Pi 4's number has to be measured on a Pi 4.
* **Not a `.cvpack`.** The pack format has no slot for a 115-symbol tokenizer table, a
  per-length style matrix or pronunciation overrides yet, and its provenance rule (real
  packs need a complete `provenance` block) correctly **refuses** this model: the tarball it
  came in states no licence for the weights. So the honest use of this spike is measurement —
  shipping these bytes as a voice pack would fail our own gate, by design.
* **Not Hindi quality.** Phonemes came from espeak-ng `hi`, unread by a Hindi speaker;
  `hf_alpha` produced a real, audible, unreviewed 3.58 s utterance and nothing more.

## Files this produced

| Path | What |
|---|---|
| `scripts/fetch-offline-model.py` | npm-tarball fetch, pinned sha256, size/hash checks, writes `SOURCE.json`, refuses to write without `--accept-licence` |
| `scripts/extract-open-phonemizer.py` | the permissive G2P path, with the two upstream defects documented and `decode_rules` emitted as data |
| `scripts/spike-kokoro-offline.py` | synthesis + `--controls` + `--emit-fixture` |
| `crates/vocal-core/tests/fixtures/kokoro/` | `reference.json`, `reference_af_heart.wav`, `tokenizer.json` |
| `crates/vocal-core/tests/kokoro_reference.rs` | CI-runnable guard: the fixture's numbers must match the committed audio; the reference must not be silence; the graph contract must stay pinned |
| `assets/offline-spike/*.wav` | four clips you can actually listen to |
