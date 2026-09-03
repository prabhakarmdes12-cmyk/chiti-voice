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
bad pack path. A sample run looks like (first command, output trimmed to the first input line):

```text
pack=tara.cvpack id=tara placeholder=true
policy max_units=509 min_chunk_units=8 declared=pack
loudness target_dbfs=-21 peak_ceiling=0.98 max_gain_db=12
line 1 chunks=1 units=11 tokens=11 row_matches_units=true
render voice=tara-mock bytes=… file=/tmp/sample.wav silent=true
note: vocal_core::REAL_SYNTHESIS_AVAILABLE=false -- the file above is the mock engine's output, not speech
```

## What each line is doing

- **`policy … declared=pack`** — the chunking policy comes from the pack's `persona.chunking`, resolved
  against the model's real token window by `Persona::chunking_policy()`. `declared=engine default` means
  the pack says nothing and the engine's default applies. Printing it is the point: a render is only
  reproducible under the policy it was planned with, and the numbers cited in `docs/personas/` were
  measured as a single chunk.
- **`row_matches_units=true`** — the style row a chunk reads is its own token count. If that ever
  disagrees, the index into the voice vector has moved, which is a silent change of prosody, so the
  sample treats it as a hard error rather than a detail.
- **`tokens=11`** — `encode` is faithful to the reference vocabulary: an unmapped symbol becomes a
  counted pad token instead of vanishing, so the token count equals the character count. The third line
  of `fixtures/lines.txt` deliberately contains ASCII `g`, which Kokoro's table does not have (it carries
  U+0261 script-g), so it exercises exactly that path.
- **`silent=true`** — `MockEngine` emits digital silence. This is not a bug in the sample; it is the
  honest state of `crates/`, which cannot run the ONNX graph yet. The last line says so out loud, in the
  output, where a reader will see it.

## The boundary nobody should blur

**This build has no grapheme-to-phoneme converter in Rust.** The engine's input is a phoneme string, so
a consumer supplies phonemes — from espeak-ng, or from the permissive `open-phonemizer` path measured in
`docs/research/KOKORO_OFFLINE_SPIKE.md`. A sample that accepted orthography and silently passed it
through would look like a working TTS API while producing garbage; that is the class of mistake this
repository has spent a branch removing, so `--help` says what the input has to be.
