# Chiti Vocal Runtime — Development Guide

## What this tree actually is

**Read this before anything else in this file.** The sections below it describe an *intended*
architecture, and several components named in them have never existed in this repository — a previous
version of this page listed Next.js, Tailwind, a TypeScript SDK, an HTTP daemon and an `ort` dependency as
part of the stack, and an agent that trusted it would have written code against nothing. An agent that
runs this file's commands cannot be misled by a sentence, so: what exists, what does not, and the one
line that decides whether anything can speak.

**Built, and green in CI** (as of `eaee524`: Build on ubuntu/macos/windows, Unit Tests, clippy
`-D warnings`, docs-truth, dependency audit, invariants; only `Format Check` is red, pending
`cargo fmt --all`):

- `crates/vocal-core` — the `VoiceEngine` trait and registry, `MockEngine`, the typed error model, the
  phoneme token table, the chunking planner (`utterance_plan.rs`), `persona.rs`, `wav.rs`,
  `audio_levels.rs`, and the ONNX-shaped pieces under `src/engine/`.
- `crates/voice-pack` — the `.cvpack` container, manifest validation, and the security limits.
- `apps/chiti-voice-cli` — speak / list / status / install / version.
- `apps/sample-reader` — a **separate consumer crate** whose tests run the built binary as a subprocess,
  so the public API has to be sufficient on its own; in-crate tests cannot show that.

**The capability line, from source rather than prose:** `REAL_SYNTHESIS_AVAILABLE` is `false`. Nothing in
`crates/` can speak yet — `MockEngine` emits digital silence and every shipped `.cvpack` carries a
placeholder model. Treat this sentence as the ground truth for "is there a voice", and note that
`scripts/verify-doc-claims.py` fails when this file or `README.md` disagrees with the constant, so it
cannot quietly drift.

**Specified, not present — do not write code against these until they exist:** no HTTP daemon (there is
no `axum` dependency and no service crate; `docs/api/HTTP_API.md` is a specification), no TypeScript
anywhere in the tree (no `package.json` at all, therefore no Voice Lab UI, no Web SDK, no Tailwind, no
Next.js), no ONNX runner (`ort` is not a dependency; `docs/architecture/ADR-001-initial-tts-backend.md`
is still PROPOSED and that decision is open and yours to make), no `Cargo.lock` and no `deny.toml` (both
need network to a registry, so they are a local step, not abandoned work), and no `apps/voice-lab/`. The
design system below is a specification for a UI that does not exist, not a description of code.


## Design System
Chiti Technologies Unified Design System v3:
- Voice Lab uses: Outfit (display), Inter (body), JetBrains Mono (code/diagnostics)
- Dark mode default
- 8pt grid
- Glassmorphism for Voice Lab panels
- Lucide React icons

## Key Invariants
Refer to `docs/architecture/INVARIANTS.md` for full details:
- `VOICE_INV_001` — Offline Independence
- `VOICE_INV_002` — LLM Independence
- `VOICE_INV_003` — Provider Independence
- `VOICE_INV_004` — Persona Independence
- `VOICE_INV_005` — Deterministic Core
- `VOICE_INV_006` — Graceful Degradation
- `VOICE_INV_007` — Local Privacy
- `VOICE_INV_008` — Voice Provenance
- `VOICE_INV_009` — Interruptibility
- `VOICE_INV_010` — Streaming Safety
- `VOICE_INV_011` — Resource Limits
- `VOICE_INV_012` — Version Compatibility

## Three Personas
- **Tara:** Warm professional, Indian English primary, business/hospitality.
- **Kashi:** Calm Hindi-first guide, educational/cultural apps.
- **Bobo:** Expressive fictional companion, children/robots.

## Error Codes
| Code | Category | HTTP Status |
|---|---|---|
| `VOICE_NOT_FOUND` | Client | 404 |
| `INVALID_TEXT` | Client | 400 |
| `ENGINE_UNAVAILABLE` | Server | 503 |
| `SYNTHESIS_FAILED` | Internal | 500 |
| `RESOURCE_LIMIT_EXCEEDED` | Client | 413 |

## Run these first

```bash
cargo fmt --all                                            # the one red check on this branch
cargo test --workspace
cargo clippy --all-targets -- -D warnings
python3 scripts/verify-doc-claims.py
python3 scripts/verify-doc-claims.py --self-test           # proves the doc gate can still fail
cargo run -q -p sample-reader -- --lines apps/sample-reader/fixtures/lines.txt --out /tmp/chiti.wav
```

That last one is the integration check that matters: it loads a real `.cvpack`, applies the pack's
resolved chunking policy, plans, renders, writes a WAV, and prints one report line per input. Expect
`silent=true` and a `note:` line naming the capability flag, and exit status 0. When you change a
signature an integrator would touch, run it — the unit tests compile inside their own crate and cannot see
a visibility or composition problem.

## Contracts that bite

These are already pinned by tests; the sentences exist so you do not rediscover them as a bug report.

1. **`phoneme_tokens::encode` returns `chars + 2` ids**, framed `PAD … PAD`, truncated at `MAX_TOKENS`
   (512). Size `input_ids` from the encoded length, never from the character count. Pinned by
   `crates/vocal-core/tests/phoneme_framing.rs`.
2. **Never filter input to the vocabulary in the synthesis path.** `strip_to_vocab` exists and its doc
   comment says why: sequence length *is* the style row, so a filter changes prosody along with dropping a
   sound. Unmapped symbols become counted `PAD` slots instead, which is what upstream does.
3. **Take chunking from `Persona::chunking_policy()`**, not from a constant. The three tracked packs all
   declare `509 / 8`; a new test that hard-codes 509 pins a manifest value instead of the relation, and
   `DEFAULT_MAX_UNITS` already exists as the single source.
4. **Input is phonemes, not orthography.** This build has no grapheme-to-phoneme converter, by design: the
   phonemiser paths upstream are GPL-3.0-or-later while the engine surface is MIT, and the graded Python
   path lives in `docs/research/KOKORO_OFFLINE_SPIKE.md`. Accepting text and passing it through would
   produce an API that looks right and pronounces garbage.
5. **Schema compatibility is exact string equality** on `SUPPORTED_SCHEMA_VERSION` (`"1.0.0"`), in four
   places on purpose. There is no `semver` dependency, and adding one to "improve" this changes
   `VOICE_INV_012` behaviour.
6. **`.cvpack` is a hostile-input boundary.** `crates/voice-pack/src/security.rs` enforces `PackLimits`;
   its executable rejection is extension-based only, and `docs/architecture/INVARIANTS.md` records that as
   a gap rather than a strength. An extensionless ELF loads today.
7. **`audio_levels` is implemented and measured, but only the mock path runs it.** `target_dbfs` /
   `peak_ceiling` / `max_gain_db` are not decoration: peak across the surveyed English voices ran
   0.50–0.99, and 0.99 is one hot voice from clipping. Wiring that stage into a real render is Step 2.
8. **`crates/vocal-core/tests/determinism_test.rs` proves repository-owned purity only** (no clock, no
   RNG, stable across two engine instances). It does not prove ONNX determinism and its header says so.

## Sprint Log

### 2026-09-03 – Docs corrected against the tree; `AGENTS.md` was the last one holding a fiction

Done in this slice, each item bound to a check rather than to a sentence:

- `docs/architecture/INVARIANTS.md` is now the **registry** for invariant IDs (`### VOICE_INV_0NN — Name`
  plus the `| **ID** |` / `| **Name** |` rows), because `PRD.md` had reused 003–012 for nine different
  requirements and two documents could cite the same ID meaning different things. The gate enforces
  ID↔name in prose *and* in source comments. `RTF ≥ 1.0` is recorded there as the only exit criterion with
  neither implementation nor measurement.
- Enforcement claims were separated into what runs and what does not: `VOICE_INV_011`'s `PackLimits`
  budget is enforced on the archive side and not on the request side; `VOICE_INV_012` is exact string
  equality with no `semver` dependency; `VOICE_INV_005`'s test proves repository-owned purity, not ONNX
  determinism; "no executable content in a pack" is enforced **by extension only**.
- `apps/sample-reader` exists, and it immediately contradicted this file's own README: `encode` returns
  `chars + 2` ids, not `chars`. Now documented in `phoneme_tokens.rs`, pinned by
  `crates/vocal-core/tests/phoneme_framing.rs`, and asserted end-to-end by a consumer outside the crate.
- This file claimed Next.js, Tailwind, a TypeScript SDK, an HTTP daemon and `ort` as the stack. None of
  them have ever existed: there is no `package.json` in the tree, no `axum` dependency, no `ort`
  dependency. `.gitignore` likewise asserted `Cargo.lock` was committed on purpose; the file is absent.
  Both corrected, and the doc gate gained a rule so an omission of the capability line fails CI -- passing
  by silence was the actual defect, not the wrong number.
- Still open and unchanged: no real inference, no MOS, RTF/RSS numbers are x86 Python measurements, Hindi
  and persona-clip phonemes are unreviewed, and the Kokoro weight licence is unverified, which blocks
  shipping derived persona vectors under `VOICE_INV_008`.

Initial repository audit and complete Chiti-style documentation generation.
- Created `README.md`, `PRD.md`, `AGENTS.md`
- Created all architecture documentation in `docs/architecture/`
- Created `.cvpack` format specification in `docs/voice-pack/`
- Created persona specifications (Tara, Kashi, Bobo) in `docs/personas/`
- Created API references in `docs/api/`
- Created research track guides in `docs/research/`
- Status: Phase 0 documentation complete.

### 2026-09-03 – Audit + correction of the Phase 1 record
The entry below was inaccurate. Kept for history, corrected here:
- The workspace did **not** compile (missing `examples/simple_speak.rs`), so no test in it ran. CI on `main` is red.
- `voice-pack` never loaded a shipped pack successfully: all three `.cvpack` files failed their own checksum/size declarations.
- `vocal-core` coverage was never measured; no coverage tool is configured.
- "Stop/cancellation works (voice.stop())" — there is no `voice.stop()`; no TypeScript SDK exists in this tree.
- Provenance in the manifests asserted `consent_obtained: true` and `model_license: Apache 2.0` for a model that does not exist. Fabricated provenance is the most serious item here; it is now a CI-checked field.
Fixes: pack limits + allowlist + provenance enforcement in `voice-pack`; CLI wired to real loading/verification; fixtures + integration tests; real network isolation in CI; `REAL_SYNTHESIS_AVAILABLE` capability flag that docs must agree with; `LICENSE`/`.gitignore` added.

### 2026-09-02 – Phase 1 Heartbeat — architecture landed, **synthesis NOT implemented**

**Codebase (2,500+ LOC Rust):**
- ✅ `vocal-core` - VoiceEngine trait abstraction, MockEngine, error types
- ✅ `voice-pack` - .cvpack ZIP loader, manifest validation, security checks
- ✅ `chiti-voice-cli` - CLI tool with 5 commands (speak, list, status, install, version)

**Voice Packs Created:**
- ✅ `tara.cvpack` - Indian English professional (en-IN)
- ✅ `kashi.cvpack` - Hindi measured (hi-IN)
- ✅ `bobo.cvpack` - Multi-lingual expressive (en-IN, hi-IN)

**Quality Assurance (as originally claimed — see correction above):**
- ⚠️ "20+ tests": 35 test functions existed, many tautological; none could run because the build was broken
- ⚠️ "Offline synthesis test validates VOICE_INV_001": the test ran a network-free mock; CI had no isolation (added 2026-09-03)
- ⚠️ "7 quality gate jobs": several could not fail (`|| echo`, `test -f` on pack existence, `echo`-only verification)
- ⚠️ "Coverage 70%+": never measured
- ⚠️ "Dependency audit: zero cloud/LLM deps": grepped manifests only; `Cargo.lock` did not exist

**Exit Criteria as recorded (mostly not met — see the 2026-09-03 entry):**
- ✗ Repository did not compile
- ✗ Coverage unmeasured; tests could not run
- ✗ Test was vacuous (no network isolation)
- ✗ No model in any pack; MockEngine audio is silence
- ✗ Not implemented; no SDK exists
- ⚠️ Validator exists, but every shipped pack failed it
- ⚠️ Check existed; had no size/rate limits or entry allowlist
- ✅ No LLM/cloud dependencies (dependency audit)
- ⚠️ Defined; pack-security codes unreachable until the From<LoadError> mapping
- ✅ All 12 system invariants documented

**Next: Phase 2 - Local Service (HTTP daemon + Web SDK)**

> **Superseded (2026-09-03).** There is no Phase 2 until something can speak. The plan is
> `docs/ROADMAP_EMBEDDED.md` §2 (engine contract → real inference on a device budget), and the runbook for
> working on it is `docs/HANDOFF_LOCAL_AGENT.md`. A daemon and an SDK over a mock that returns silence is
> how this repository produced its last false "COMPLETE".

## CI status

The corrected quality gates are **staged, not installed**: `.github/workflows/` could not
be updated by the tooling that wrote them (needs the `workflows` permission). Run
`scripts/install-ci.sh`, commit, and push. Until then the live workflow's "PASSED" output
means nothing — it is mostly `echo` statements and one `|| echo` that cannot fail. See
`ops/ci/README.md`.

## Rules for future work (from the 2026-09-03 audit)

1. **Never write "COMPLETE"/"✅" for a capability without a machine-checkable assertion
   that fails when it is absent.** The `docs-truth` CI job exists for this. When you add a
   capability, extend that job's checks in the same PR.
2. **`REAL_SYNTHESIS_AVAILABLE` in `crates/vocal-core/src/lib.rs` is the single source of
   truth for "can it speak"; `ops/ci/ci-phase1.yml` is the source of truth for what CI
   enforces, and it is inert until installed.** Flip it only with a test that decodes a real `.cvpack`
   model and asserts non-zero PCM.
3. **Never fabricate or infer provenance.** `consent_obtained`, `model_license` and
   `dataset_attribution` must be copied from the actual model card/speaker contract of the
   file you are shipping. Placeholder packs set `status: "placeholder"` and leave those
   fields null — that is the honest state, enforced by `build-voice-packs.py
   --require-real-models` and the `supply-chain` job.
4. **Do not commit model weights to git.** Drop them in `voice-packs/<id>/model.onnx`
   (gitignored) and let the pack builder hash them.
5. **A test that cannot fail is worse than no test.** Prefer fixtures of hostile input
   (`scripts/make-test-fixtures.py`) over happy-path self-confirmation.
6. **Adding a dependency requires clearing two bars:** no network client in the synthesis
   path, and it must cross-compile to the target device (see `docs/ROADMAP_EMBEDDED.md`).

7. **Do not re-add the diagnostics harnesses and leave them in.** `scripts/ci-rustc-wrapper.sh` and
   `scripts/ci-test-capture.sh` (wired through a `.cargo/config.toml`) exist in history to turn rustc
   diagnostics and failing-test panics into check-run annotations when CI logs are unreadable. They are
   deleted on purpose once green: the Windows job cannot exec them, and an *empty* file wired as the test
   runner makes every test pass by swallowing exit statuses. That happened — a `git show <sha>:path > path`
   with a bad sha truncated the file and the commit went out anyway.
8. **A gate that invents findings is worse than no gate.** `scripts/verify-doc-claims.py` once reported the
   repository's *tracked* `voice-packs/dist/*.cvpack` files as missing because its skip set contained
   `dist`, and it blocked a correct commit; it now enumerates tracked files with `git ls-files`. If a doc
   check fires on something you can see with your own eyes, fix the checker in the same commit and say so
   in the message.
9. **Never write a number into documentation that nothing measured.** A sample README on this branch shipped
   an example run with invented values; it is now a field shape with `…` wherever a value was never
   produced, and the only literals are ones a reader can grep out of a manifest.
