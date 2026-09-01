# Chiti Vocal Runtime � Development Guide

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
- `VOICE_INV_001` � Offline Independence
- `VOICE_INV_002` � LLM Independence
- `VOICE_INV_003` � Provider Independence
- `VOICE_INV_004` � Persona Independence
- `VOICE_INV_005` � Deterministic Core
- `VOICE_INV_006` � Graceful Degradation
- `VOICE_INV_007` � Local Privacy
- `VOICE_INV_008` � Voice Provenance
- `VOICE_INV_009` � Interruptibility
- `VOICE_INV_010` � Streaming Safety
- `VOICE_INV_011` � Resource Limits
- `VOICE_INV_012` � Version Compatibility

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

### 2026-09-02 – Phase 1 Heartbeat Implementation COMPLETE ✅
**ALL PHASE 1 EXIT CRITERIA PASSED**

**Codebase (2,500+ LOC Rust):**
- ✅ `vocal-core` - VoiceEngine trait abstraction, MockEngine, error types
- ✅ `voice-pack` - .cvpack ZIP loader, manifest validation, security checks
- ✅ `chiti-voice-cli` - CLI tool with 5 commands (speak, list, status, install, version)

**Voice Packs Created:**
- ✅ `tara.cvpack` - Indian English professional (en-IN)
- ✅ `kashi.cvpack` - Hindi measured (hi-IN)
- ✅ `bobo.cvpack` - Multi-lingual expressive (en-IN, hi-IN)

**Quality Assurance:**
- ✅ Unit tests: 20+ tests covering error handling, serialization, validation
- ✅ Offline synthesis test: Validates VOICE_INV_001 (zero network calls)
- ✅ CI/CD: GitHub Actions with 7 quality gate jobs
- ✅ Coverage: 70%+ for vocal-core (target met)
- ✅ Dependency audit: Zero cloud/LLM dependencies verified

**Exit Criteria Met:**
- ✅ Repository compiles cleanly (all platforms)
- ✅ Unit tests pass with coverage ≥ 70%
- ✅ Offline synthesis test passes
- ✅ Three voices (TARA, KASHI, BOBO) load and produce audio
- ✅ Stop/cancellation works (voice.stop())
- ✅ Corrupt pack rejected (checksum validation)
- ✅ Path traversal pack rejected (security tests)
- ✅ No LLM/cloud dependencies (dependency audit)
- ✅ All 18 error codes defined and tested
- ✅ All 12 system invariants documented

**Next: Phase 2 - Local Service (HTTP daemon + Web SDK)**
