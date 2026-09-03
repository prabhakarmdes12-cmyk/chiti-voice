# Handoff: running this repo with a local agent

Written against `eaee524` on branch `arena/01a06392-chiti-voice` (PR #1). If `git rev-parse HEAD`
disagrees, re-read the two sections this one depends on: `docs/ROADMAP_EMBEDDED.md` §"Resolved
2026-09-03 (tests)" and `apps/sample-reader/README.md`.

Everything in this file is a command to run or a constraint that binds. It deliberately does not
restate the design — that is `docs/architecture/` and `docs/ROADMAP_EMBEDDED.md`.

**If you are an agent, start at `AGENTS.md` in the repository root.** That file is auto-loaded by the
tools that read it, so it carries the same capability line, the commands, and the binding rules in
compressed form; this document is the longer runbook behind it. Both are checked against the tree by
`scripts/verify-doc-claims.py`, including a rule that fails if either one omits the capability flag --
passing by silence is how the last version of this repository got a stale `AGENTS.md` describing
Next.js, an SDK and an HTTP daemon that were never written.

## Why local, and what local unlocks

The work so far was done in a sandbox with **no Rust toolchain at all** and with `crates.io`,
`static.crates.io`, `huggingface.co` and `raw.githubusercontent.com` blocked. That shapes the tree in
ways a fresh reader keeps misreading, so state it plainly:

- Every claim about Rust code compiling is CI's, not a local run's. `Build`, `Unit Tests` and
  `Linting (clippy --all-targets -- -D warnings)` are green on ubuntu, macOS and Windows for `eaee524`.
  `Format Check` is red and has been red for many cycles: `cargo fmt --all` has never been run here.
- No dependency lockfile and no lint config exist in the tree, and neither does an audit config.
  `generate-lockfile` and `cargo deny` need network to a registry this sandbox cannot reach, so they
  are yours, not unfinished work I skipped on purpose.
- Model weights cannot be fetched here, which is why nothing has ever tried to run a real graph in
  `crates/`. Locally you can. That is the single biggest difference between your environment and the
  one that produced this branch.
- `docs/architecture/INVARIANTS.md` describes two enforcement scripts as absent because they are
  absent, and describes the CI dependency check as manifest-only because that is what it is. If you
  add either script, update that wording in the same commit: a documented check pointing at a path
  that does not exist is the exact failure this repo had, and `scripts/verify-doc-claims.py` exists to
  catch it.

Fetch the state. This work lives on a session branch that is merged into `main` when the owner accepts
PR #1, so try the default branch first and fall back to the branch name:

```bash
git pull origin main                      # if PR #1 has landed, this is the whole tree
# otherwise:
git fetch origin arena/01a06392-chiti-voice
git switch -c chiti-local --track origin/arena/01a06392-chiti-voice
git rev-parse HEAD                        # expect 5cd1e80 or a descendant
```

If neither is convenient, the offline transfer (a self-contained `git bundle`, a patch series, and a
sha256 to check them with) is regenerated on request -- it was last built at `5cd1e80` and lived at
`target/agent-handoff/` in the sandbox, which is gitignored, so it is not in the clone you are reading.

## 1. Make the tree honest about formatting

```bash
cargo fmt --all
cargo test --workspace            # expect: sample-reader's 5 integration tests pass
cargo clippy --all-targets -- -D warnings
python3 scripts/verify-doc-claims.py
python3 scripts/verify-doc-claims.py --self-test
```

`--self-test` matters more than it looks: the gate plants a false claim in a temp tree and demands it
be caught, because the first version of that checker silently passed a tree it never walked. If
`verify-doc-claims` reports a missing path you *know* is tracked, suspect the gate before you touch
the doc — it once reported `voice-packs/dist/*.cvpack` as missing because its skip set contained
`dist`, and the right fix was in the gate.

## 2. Run the thing that proves the public API is enough

```bash
cargo run -q -p sample-reader -- --lines apps/sample-reader/fixtures/lines.txt --out /tmp/chiti-sample.wav
```

Expected: one `line N chunks=… units=… framed=… row_matches_units=true framed_ok=true` per fixture
line, a `render voice=tara-mock bytes=… silent=true` line, a `note: vocal_core::REAL_SYNTHESIS_AVAILABLE=false`
line, exit status 0. The `.wav` is digital silence — that is the honest state of `crates/`, not a bug
in the sample. `apps/sample-reader` exists as a *separate crate* on purpose: an in-crate test compiles
whether an item is `pub` or `pub(crate)`, so it cannot show the public surface is usable. When you
change a signature that an integrator would touch, run this command, not only the unit tests.

Two of its report fields exist because an integrator gets them wrong:

- `framed` = `units + 2` per utterance. `phoneme_tokens::encode` returns `PAD`, the content ids, `PAD`,
  so `input_ids` must be sized from the encoded length, never from the character count. Pinned by
  `crates/vocal-core/tests/phoneme_framing.rs`; if you ever "fix" the counts by filtering to the
  vocabulary, read `strip_to_vocab`'s doc comment first — sequence length *is* the style row, so a
  filter changes prosody along with the sound.
- The chunking policy is taken from `Persona::chunking_policy()`, i.e. the pack's `persona.chunking`
  resolved against the model's real window (`509 / 8` for all three tracked packs). Do not hard-code
  509 in a new test; assert the relation, not the constant. The sample's tests deliberately do not.

## 3. The open decision that blocks real audio

`docs/architecture/ADR-001-initial-tts-backend.md` is still PROPOSED. Nothing in `crates/` can run the
ONNX graph, so the engine is `MockEngine` and `REAL_SYNTHESIS_AVAILABLE` is `false`. The two live
options are the `ort` crate versus a sidecar process running Python + onnxruntime. The graded fixtures
for whichever you pick are in `crates/vocal-core/tests/fixtures/kokoro/`, and Step 1's contract is
already pinned as code:

- `input_ids` int64 `[1, seq]` where `seq = chars + 2` and is clamped to 512; `style` f32 `[1, 256]`
  read as `voice_bin[n_tokens * 256 .. (n_tokens + 1) * 256]`; `speed` f32 `[1]`; output clamped as
  `floor(x * 32767)` at 24 kHz mono. A voice `.bin` is 522240 bytes = 510 × 256 f32. Ids are 0..=177.
- Graph inputs are only `input_ids`, `speed`, `style`. Pitch is a *casting* choice, not a runtime
  parameter: a non-zero `default_pitch` without `pitch_baked_into_style`, a `default_pitch` of exactly
  `1.0`, and a non-zero `IntentProfile.pitch` are all load errors on purpose. Energy maps to
  `target_dbfs + (energy - 0.5) * 12.0`; `pause_factor` is 0.5–3.0; expressiveness is F0-range
  **selection**, never blending.
- `crates/vocal-core/src/audio_levels.rs` implements the declared `target_dbfs` / `peak_ceiling` /
  `max_gain_db` stage and is graded against real graph output in `tests/dsp_parity.rs`, but only the
  mock path runs it. Wiring it into the real render path is the first job after the engine decision,
  and the reason it is mandatory is measured: peak across 54 English voices ranged 0.50–0.99 on one
  sentence.

Nothing here has measured either option, so treat the paragraph above as the shape of the question and
the numbers below as somebody else's. `ort` links the full onnxruntime, which is tens of megabytes of
added binary before a model loads, plus a transitive dependency graph that `cargo deny` has never looked
at. And the constraint that actually binds on a Pi, a robot and a toy is **peak RSS, not model size** —
for context, PicoVoice's published benchmarks put Piper at a 61 MB model against 2.6 GB of peak RAM.
That figure is from their blog and not from anything measured in this repo; `docs/research/NANO_ENGINE.md`
is what the repo itself says about size. Re-measure on the smallest target rather than arguing from either.

## 4. Constraints that do not move

- **Offline is non-negotiable.** No cloud call and no LLM in the synthesis path, ever. `scripts/install-ci.sh`
  wires a job that greps for the dependency shapes that would violate this; keep it green by not adding
  them rather than by editing it.
- **Generated voice, not borrowed.** "Use an existing open model" and "commission a voice actor" were
  both rejected. Synthesis must come from a generated persona voice.
- **Licensing blocks shipping, not developing.** `docs/LICENSES_THIRD_PARTY.md`: the Kokoro carrier's
  MIT licence covers its code, not its model data, so derived persona vectors are not shippable under
  `VOICE_INV_008`. Don't commit new `.bin` style vectors as deliverables; the spike's fixtures are test
  data and labelled as such. Also note the engine may be MIT while espeak-style phonemiser paths are
  GPL-3.0-or-later — that is why phonemes are an input to this crate and not a bundled converter.
- **`.cvpack` is a hostile-input boundary.** `crates/voice-pack/src/security.rs` enforces `PackLimits`
  (per-file and total bytes, compression ratio, entry count). Its executable rejection is
  extension-based only, which `docs/architecture/INVARIANTS.md` states as a gap: an extensionless ELF
  loads today. If you tighten that, tighten the sentence too.
- **Schema is exact-match, not semver.** `SUPPORTED_SCHEMA_VERSION` is `"1.0.0"` in four places and
  `manifest.rs` compares strings. There is no `semver` dependency and that is deliberate, not an
  oversight.
- **Determinism is a repository-owned claim only.** `crates/vocal-core/tests/determinism_test.rs`
  proves the Rust side is pure (no clock, no RNG — it scans its own source) and stable across a second
  engine instance; it does **not** prove ONNX determinism, and its header says so. Do not strengthen
  that claim without measuring an actual graph twice with `--nocapture` numbers in the commit message.

## 5. Two repo-local traps

- The live `.github/workflows/ci-phase1.yml` is stale; the corrected definition is `ops/ci/ci-phase1.yml`
  and `scripts/install-ci.sh` copies it into place. That exists because pushing workflow files needs the
  `workflows` permission. Until you run it, the docs-truth job (`name: Docs must not overclaim capability`)
  does not actually run on your pushes, even though `python3 scripts/verify-doc-claims.py` passes locally.
- If CI's output is unreadable to you and you need diagnostics, the two temporary hooks used to build this
  branch are recoverable and were deliberately deleted:

  ```bash
  git show 2831009:scripts/ci-test-capture.sh > scripts/ci-test-capture.sh   # 2373 bytes, test-runner annotations
  git show 7c66494:scripts/ci-rustc-wrapper.sh > scripts/ci-rustc-wrapper.sh # 3725 bytes, rustc annotations
  ```

  They are wired through a `.cargo/config.toml` that must also be deleted again, and the Windows job cannot
  exec them, so Windows goes red while they are in. An empty file wired as a test runner makes every test
  pass by swallowing exit statuses — check the restored files are non-empty before committing them, and
  remove them in the next commit rather than letting them age.

## 6. Suggested order

1. `cargo fmt --all`, then §1 and §2 green locally.
2. `cargo generate-lockfile` and `cargo deny check` (write the config; audit the licences of every crate
   in the graph, which has never been done — `docs/LICENSES_THIRD_PARTY.md` covers assets, not deps).
3. `python3 scripts/install-ci.sh`, so the corrected gate runs on pushes.
4. The `ort`-vs-sidecar ADR, decided with a measurement on the smallest target rather than a preference.
5. Wire `audio_levels` into the real render path, then replace the mock in `sample-reader`'s report with a
   real render — at which point `REAL_SYNTHESIS_AVAILABLE` flips to `true` and `README.md`'s headline claim
   must flip with it, because the doc gate compares the two and fails if they disagree.
