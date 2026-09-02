# Chiti Vocal Runtime — Development Guide

## Design System
Chiti Technologies Unified Design System v3:
- Voice Lab uses: Outfit (display), Inter (body), JetBrains Mono (code/diagnostics)
- Dark mode default
- 8pt grid
- Glassmorphism for Voice Lab panels
- Lucide React icons

## Stack
- **Rust:** Native runtime, audio pipeline, local service, engine adapters
- **TypeScript:** Voice Lab UI (Next.js 16), Web SDK, CLI tooling
- **Python:** Research, model evaluation, dataset preparation, benchmark scripts
- **ONNX Runtime:** Model inference (desktop: ort crate, browser: onnxruntime-web)
- **Tailwind CSS v4:** Voice Lab UI
- **Tokio / Axum:** Async runtime & HTTP daemon

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

## Sprint Log

### 2026-09-01 – Project Foundation
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
