# sample-reader — a consumer's view of Chiti Voice

Not a product surface. This crate exists because every other test in the workspace lives *inside*
`crates/vocal-core` or `crates/voice-pack`, and an in-crate test can see things that a real integrator
cannot: it compiles whether or not an item is `pub`, whether an error type can be named from outside,
and whether the pieces compose without reaching past the crate boundary. This one can't cheat — it
depends on both crates by path and may only use what they export.

CI builds it (`cargo build --workspace --all-targets`), runs its tests
(`cargo test --workspace --all-targets`), and lints it with `-D warnings`, so an API that stops being
usable from outside becomes a red job rather than a someone-figures-it-out event.

## Run it

```bash
cargo run -p chiti-sample-reader -- --lines apps/sample-reader/fixtures/lines.txt --out /tmp/sample.wav
cargo run -p chiti-sample-reader -- --text "hələʊ wɜːld" --voice kashi
```

It loads `voice-packs/dist/tara.cvpack` by default, prints one line per input, and exits non-zero on a
bad pack path. The report shape:

```text
pack=<file> id=<manifest id> placeholder=<is this a stub pack>
policy max_units=509 min_chunk_units=8 declared=pack|engine default
loudness target_dbfs=… peak_ceiling=… max_gain_db=…
line N chunks=… units=… framed=… row_matches_units=… framed_ok=…
render voice=… bytes=… file=… silent=…
note: vocal_core::REAL_SYNTHESIS_AVAILABLE=false -- the file above is the mock engine's output, not speech
```

The `policy` numbers are the ones `voice-packs/tara/manifest.json` declares, and the loudness triple is
its `loudness` block. Everything else is deliberately written as `…`: this README has no Rust toolchain
to run against, and a sample document full of plausible-looking fabricated output is the most reliable
way to teach a number that was never measured. The tests in `tests/integration.rs` are what pin these
fields; they run the built binary, so the values they assert are CI's, not mine.

## What each line is doing

- **`policy … declared=pack`** — the chunking policy comes from the pack's `persona.chunking`, resolved
  against the model's real token window by `Persona::chunking_policy()`. `declared=engine default` means
  the pack says nothing and the engine's default applies. Printing it is the point: a render is only
  reproducible under the policy it was planned with, and the numbers cited in `docs/personas/` were
  measured as a single chunk.
- **`row_matches_units=true`** — the style row a chunk reads is its own token count. If that ever
  disagrees, the index into the voice vector has moved, which is a silent change of prosody, so the
  sample treats it as a hard error rather than a detail.
- **`framed = units + 2`** -- `encode` is reference-faithful in two ways that are easy to conflate. An
  unmapped symbol becomes a counted `PAD` slot rather than vanishing, so the content token count equals
  the character count; and the sequence is framed `PAD … PAD`, so the encoded tensor is two rows wider
  than the content. `framed_ok=false` means that relation broke somewhere other than the framing, which
  is why the sample treats it as a hard error. The third line of `fixtures/lines.txt` contains ASCII `g`,
  which Kokoro's table does not have (it carries U+0261 script-g), so it exercises the pad path.
  Do not "fix" an unmapped symbol by filtering it out: `strip_to_vocab` exists, and its own doc says a
  filter in the synthesis path moves the style row and therefore the prosody, which is why it survives
  only as a reporting helper.
- **`silent=true`** — `MockEngine` emits digital silence. This is not a bug in the sample; it is the
  honest state of `crates/`, which cannot run the ONNX graph yet. The last line says so out loud, in the
  output, where a reader will see it.

## The boundary nobody should blur

**This build has no grapheme-to-phoneme converter in Rust.** The engine's input is a phoneme string, so
a consumer supplies phonemes — from espeak-ng, or from the permissive `open-phonemizer` path measured in
`docs/research/KOKORO_OFFLINE_SPIKE.md`. A sample that accepted orthography and silently passed it
through would look like a working TTS API while producing garbage; that is the class of mistake this
repository has spent a branch removing, so `--help` says what the input has to be.
