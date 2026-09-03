# Chiti Vocal Runtime — Product Requirements Document

**Version:** 0.1.0 — Draft  
**Date:** September 2026  
**Owner:** Chiti Technologies  
**Status:** Phase 1 (Heartbeat) Complete ✅  
**Classification:** Internal — Confidential

---

## Implementation Status

| Phase | Name | Status | Released |
|-------|------|--------|----------|
| 0 | Documentation & Architecture | ✅ Complete | 2026-09-01 |
| 1 | Heartbeat (MockEngine, Voice Packs, CLI) | ✅ Complete | 2026-09-02 |
| 2 | Local Service (HTTP daemon, Piper, Web SDK) | 🔄 In Planning | Q4 2026 |
| 3 | Streaming & Text Normalization | 📅 Planned | Q1 2027 |

**Phase 1 Deliverables:** Cargo workspace with VoiceEngine trait, MockEngine implementation, `.cvpack` voice pack format with security validation, CLI tool (speak, list, install, status), three reference personas (TARA/KASHI/BOBO), 20+ unit tests, CI/CD pipeline, offline synthesis validation.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Problem Statement](#2-problem-statement)
3. [Vision](#3-vision)
4. [Target Users](#4-target-users)
5. [Six Core Products](#5-six-core-products)
6. [Three Personas — Requirements](#6-three-personas--requirements)
7. [Functional Requirements](#7-functional-requirements)
8. [Non-Functional Requirements](#8-non-functional-requirements)
9. [Voice Pack (.cvpack) Specification Summary](#9-voice-pack-cvpack-specification-summary)
10. [System Invariants Summary](#10-system-invariants-summary)
11. [Architecture Overview](#11-architecture-overview)
12. [Development Phases](#12-development-phases)
13. [Performance Targets](#13-performance-targets)
14. [Quality Gates for v0.1](#14-quality-gates-for-v01)
15. [Error Model](#15-error-model)
16. [Observability Requirements](#16-observability-requirements)
17. [Security Requirements](#17-security-requirements)
18. [Long-Term Products (Future Scope)](#18-long-term-products-future-scope)
19. [Research Track](#19-research-track)
20. [Open Questions / Decisions Pending](#20-open-questions--decisions-pending)

---

## 1. Executive Summary

Chiti Vocal Runtime is an **offline-first, LLM-independent voice infrastructure platform**. It gives software a persistent vocal identity in the same way that typography gives software a visual identity.

Unlike TTS wrappers — which are thin clients over cloud speech APIs — Chiti Vocal Runtime is a **platform**. Runtime, packaging, persona, prosody, streaming, and developer experience are all first-class concerns, not afterthoughts. The platform is designed to be:

- **Offline-first by architecture**, not by configuration. Synthesis paths requiring any outbound network call are rejected at the design level.
- **LLM-independent by invariant.** The runtime receives text and renders it as audio. It does not decide what to say.
- **Provider-agnostic.** Acoustic backends (Kokoro, Piper, ONNX, future native) are swappable implementations behind a stable `VoiceEngine` interface.
- **Persona-aware.** Voice identity is encoded in a persona configuration layer that is structurally separated from the acoustic model.
- **Portable.** A `.cvpack` voice file contains everything needed to load and speak a voice on any supported platform.

**Primary goal:** A developer installs a `.cvpack` voice file and calls `voice.speak(text)` from any application type — website, desktop app, robot, mobile, AI agent — and hears the same voice, offline, without modifying the application for each platform.

---

## 2. Problem Statement

### 2.1 Current State of Voice in Software

| Problem | Impact |
|---------|--------|
| Cloud API dependency | Voice breaks when the network is unavailable; latency is uncontrollable; costs scale with usage |
| No offline capability | Applications cannot function in airplane mode, rural connectivity, or on-device hardware |
| No persona consistency | The same application sounds different across platforms, sessions, or API versions |
| LLM coupling | Voice is bundled with language generation; teams cannot swap voice without swapping the LLM |
| No provenance standard | There is no way to know who trained a voice, on what data, under what consent model |
| No portable identity | Every TTS integration is a bespoke wrapper; there is no shared runtime, no portable voice file |
| No packaging standard | There is no `.ttf` equivalent for synthetic voice — no standard format, no versioning, no licensing |
| No separation of concerns | Text generation and speech rendering are conflated, making each independently improvable only with full system replacement |

### 2.2 Developer Pain Points

- Integrating TTS into a website requires a cloud account, SDK, and API key — all of which create cost, latency, and failure surfaces.
- Switching TTS providers means rewriting every callsite.
- Evaluating voice quality requires spinning up a live service.
- There is no standard for expressing "use this voice, at this speed, with this persona" across providers.
- Testing offline voice behavior is not possible without specialized infrastructure.

### 2.3 Market Gap

No current product provides: offline synthesis + portable voice packaging + persona runtime + developer-first SDK as an integrated, coherent platform. Existing solutions (Amazon Polly, Google TTS, ElevenLabs, Coqui, Piper) address at most one or two of these dimensions. Chiti Vocal Runtime addresses all of them as a unified system.

---

## 3. Vision

> **Make voice identity as installable, portable, and composable as a font.**

A voice pack is to speech what a `.ttf` file is to text rendering:

| Font Analogy | Voice Analogy |
|-------------|--------------|
| `.ttf` / `.otf` file | `.cvpack` file |
| Font family + weight + style | Voice identity + persona + prosody profile |
| Font renderer (FreeType, CoreText) | Chiti Vocal Core (synthesis engine) |
| Text layout engine | Chiti Persona Runtime (text → synthesis parameters) |
| Web font (`@font-face`) | Voice Web SDK (`@chiti/voice-web`) |
| Font foundry / type designer | Chiti Voice Foundry (future) |

A developer should be able to install a voice, call `speak()`, and have it work — in the same way that installing a font makes it available to every application on the system. The font never asks "what text should I render?" — it only knows how to render what it is given. Chiti Vocal Runtime never asks "what should I say?" — it only knows how to say what it receives.

---

## 4. Target Users

### 4.1 Primary Users

| User Segment | Description | Key Need |
|-------------|-------------|---------|
| **Application Developers** | Engineers integrating voice into consumer or enterprise products | Simple API, consistent voice across platforms, no cloud dependency |
| **AI Agent Developers** | Engineers building LLM-powered agents that need a voice output layer | LLM-independent voice rendering, streaming, barge-in |
| **Hardware / Robot Engineers** | Embedded systems, robotics, IoT devices requiring on-device TTS | Minimal memory footprint, offline synthesis, Rust/C FFI |
| **Enterprise Privacy Teams** | Organizations requiring no audio or text to leave the device | Guaranteed loopback-only operation, no telemetry, auditable code |
| **Voice Designers / Researchers** | Designers and ML engineers building and evaluating custom vocal personas | Voice Lab GUI, pronunciation editor, benchmark tooling |

### 4.2 Secondary Users

| User Segment | Description |
|-------------|-------------|
| **Accessibility Engineers** | Teams building screen readers or assistive tools needing offline reliable TTS |
| **Education Platform Builders** | Products requiring child-safe, expressive, cost-controlled voice |
| **Game Developers** | Real-time NPC voice synthesis without cloud round-trips |

---

## 5. Six Core Products

### 5.1 Chiti Vocal Core

| Field | Detail |
|-------|--------|
| **Name** | Chiti Vocal Core |
| **Description** | The runtime engine abstraction and synthesis pipeline. Provider-agnostic, backend-swappable, and the only authorized path to audio output in the system. |
| **Primary Interface** | Rust library (`vocal-core`); exposed as FFI and via the local daemon HTTP API |
| **Key Capabilities** | VoiceEngine trait abstraction; backend registration and routing; text normalization pipeline; G2P (grapheme-to-phoneme) layer; prosody application; PCM output; hardware profiling; pack loading and validation |
| **What It Is NOT** | Not a text generator. Not an LLM. Not an audio player. Not a cloud proxy. |

### 5.2 Chiti Voice Pack (`.cvpack`)

| Field | Detail |
|-------|--------|
| **Name** | Chiti Voice Pack |
| **Description** | A portable, versioned, provenance-aware voice package. Contains the acoustic model, phoneme tables, metadata, persona defaults, and cryptographic manifest for one voice identity. |
| **Primary Interface** | File format (`.cvpack`); loaded by Vocal Core via the Pack Loader component |
| **Key Capabilities** | ZIP-based container; JSON manifest with schema version; engine family declaration; provenance fields (training data statement, consent attestation); checksum validation; path traversal protection |
| **What It Is NOT** | Not an executable. Not a plugin. Not a streaming audio file. Not a voice model in a proprietary binary format with no metadata. |

### 5.3 Chiti Persona Runtime

| Field | Detail |
|-------|--------|
| **Name** | Chiti Persona Runtime |
| **Description** | The layer that maps text, context, and intent signals into synthesis parameters. It knows that TARA is warm-professional and KASHI is calm-measured, and it applies prosody, rate, pitch range, and pause patterns accordingly. |
| **Primary Interface** | Internal Rust module within `vocal-core`; exposed via persona config in `.cvpack` and via API `intent` field |
| **Key Capabilities** | Intent-to-prosody mapping; emotion/style parameter application; persona-specific text preprocessing; multi-locale persona routing; persona isolation (no cross-persona bleed) |
| **What It Is NOT** | Not a sentiment analyzer. Not an LLM. Not a generative text modifier. Intent signals are always provided externally; the Persona Runtime applies them — it does not infer them. |

### 5.4 Chiti Vocal Local Service

| Field | Detail |
|-------|--------|
| **Name** | Chiti Vocal Local Service |
| **Description** | A loopback HTTP/WebSocket daemon that any local application — regardless of language, framework, or runtime — can call to synthesize speech. It is the universal local bridge from any application to Vocal Core. |
| **Primary Interface** | HTTP REST (`http://127.0.0.1:7731/v1/`) and WebSocket for streaming |
| **Key Capabilities** | Voice management endpoints; speak and stream endpoints; PCM/WAV/OGG format support; loopback-only binding; origin allowlist; health and status endpoints; graceful shutdown |
| **What It Is NOT** | Not a public internet service. Not a multi-tenant server. Not bound to any interface other than loopback. No inbound connections from remote hosts are accepted. |

### 5.5 Chiti Voice Web SDK

| Field | Detail |
|-------|--------|
| **Name** | Chiti Voice Web SDK |
| **Description** | The official TypeScript SDK (`@chiti/voice-web`) for browser and Node.js applications. It connects to the Vocal Local Service and exposes a clean, typed API for voice operations. |
| **Primary Interface** | npm package `@chiti/voice-web`; ESM and CJS builds; TypeScript declarations |
| **Key Capabilities** | `ChitiVoice.load()`, `voice.speak()`, `voice.stream()`, `voice.stop()`; event system for audio lifecycle; persona parameter passthrough; queue management; browser AudioContext integration |
| **What It Is NOT** | Not a standalone TTS engine in the browser (Phase 1–11). Browser-native synthesis (WASM/WebGPU) is a Phase 12 capability. Until Phase 12, the SDK requires the local daemon to be running. |

### 5.6 Chiti Voice Lab

| Field | Detail |
|-------|--------|
| **Name** | Chiti Voice Lab |
| **Description** | A developer-facing desktop GUI (Tauri + React + Tailwind) for loading voices, running evaluation sentences, comparing voice packs, listening to synthesis output, viewing waveforms, and benchmarking latency and quality. |
| **Primary Interface** | Desktop application (Windows, macOS, Linux via Tauri) |
| **Key Capabilities** | Voice pack browser and loader; SSML and plain text input; real-time waveform and spectrogram display; A/B comparison between two voice packs; pronunciation dictionary editor; benchmark runner (RTF, TTFA, memory); persona parameter playground |
| **What It Is NOT** | Not a production speech interface. Not a consumer product. Not a voice recorder. It is a developer tool only. |

---

## 6. Three Personas — Requirements

### 6.1 TARA

| Field | Requirement |
|-------|-------------|
| **Purpose** | Primary business, hospitality, and customer-facing voice persona |
| **Identity** | Warm, professional, Indian English female-presenting |
| **Primary Language** | Indian English (en-IN) |
| **Additional Languages** | Hindi (hi-IN) secondary; additional Indian languages as roadmap items |
| **Critical Evaluation Sentences** | "Your appointment is confirmed for Thursday at three PM." / "We'll be with you shortly — thank you for your patience." / "The total amount due is twelve thousand five hundred rupees." / "Your order has been dispatched and will arrive within two to three business days." |
| **Differentiation Requirements** | Must render ₹ currency correctly by verbal expansion; must not sound robotic on long sentences; warmth must be perceptible without being exaggerated; rate must be natural for Indian English (not UK or US-tuned baseline) |
| **Prohibited Characteristics** | No flattened American accent. No excessive formality that sounds cold. No child-like characteristics. |

### 6.2 KASHI

| Field | Requirement |
|-------|-------------|
| **Purpose** | Guidance, knowledge delivery, navigation, and information-dense speech |
| **Identity** | Calm, measured, Hindi/Sanskrit-aware male-presenting |
| **Primary Language** | Hindi (hi-IN) |
| **Additional Languages** | Sanskrit vocabulary support (common Devanagari terms in Hindi context); Indian English secondary |
| **Critical Evaluation Sentences** | "आगे बाईं ओर मुड़ें, और पाँच सौ मीटर चलें।" / "आपका प्रश्न महत्वपूर्ण है।" / "शांति और धैर्य ही सच्ची शक्ति है।" / "The Vedic concept of dharma encompasses righteous duty, cosmic order, and moral law." |
| **Differentiation Requirements** | Must render Sanskrit loanwords with appropriate phoneme fidelity; pacing must support information-dense content without rushing; measured cadence must be perceptible but not monotone |
| **Prohibited Characteristics** | No rushed pacing. No American-accented Hindi. No childlike or upbeat affect. |

### 6.3 BOBO

| Field | Requirement |
|-------|-------------|
| **Purpose** | Children's products, educational toys, companion robots, and high-expressiveness interfaces |
| **Identity** | Playful, expressive, fictional non-child character — not an impersonation of a child |
| **Primary Language** | Indian English (en-IN), simplified vocabulary register |
| **Additional Languages** | Hindi (hi-IN) secondary, simplified register |
| **Critical Evaluation Sentences** | "Woah! You got it right — amazing job!" / "Let's count together: one, two, three, four, five!" / "Uh oh! Let's try that again — you can do it!" / "I love learning new things with you every day!" |
| **Differentiation Requirements** | Must be high-expressiveness — energy variance between excited and calm states must be measurable and perceptible; must not sound like a real child (CIPA compliance); enthusiasm must be genuine-sounding, not mechanical |
| **Prohibited Characteristics** | No child voice impersonation. No flat robotic delivery on exclamation sentences. No adult corporate affect (no TARA-like professionalism). |

---

## 7. Functional Requirements

### 7.1 Offline Synthesis

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-001 | The system MUST synthesize speech with no outbound network connection. | Automated test: network interface blocked at OS level; synthesis completes successfully and produces valid audio. | **P0** |
| FR-002 | No synthesis code path MAY make any HTTP, DNS, or socket call to a non-loopback address. | Static analysis + runtime network call audit must find zero external calls in synthesis path. | **P0** |
| FR-003 | All model files required for synthesis MUST be embedded in or co-located with the `.cvpack` file. | A voice pack installed on a machine that has never had internet access must speak without any download prompts. | **P0** |

### 7.2 LLM Independence

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-004 | The Vocal Core MUST accept plain text input and render it without any LLM processing step. | Integration test: input text → audio output with zero LLM API calls or LLM library imports in the critical path. | **P0** |
| FR-005 | LLM adapters, if provided, MUST be optional external packages that are explicitly added by the consuming application. | `vocal-core` and `voice-web` package manifests must have zero LLM-related production dependencies. | **P0** |
| FR-006 | The system MUST NOT fail, degrade, or emit warnings when no LLM adapter is present. | Standalone synthesis test with no LLM package installed passes with exit code 0 and no warnings. | **P0** |

### 7.3 Provider Abstraction

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-007 | Vocal Core MUST expose a `VoiceEngine` interface/trait that all TTS backends implement. | Two different backends (e.g., Kokoro, Piper) can be swapped by changing one configuration line with no application code changes. | **P0** |
| FR-008 | No application code MAY directly instantiate a backend implementation. All synthesis MUST go through the `VoiceEngine` interface. | Code review gate: grep for direct backend instantiation in `apps/` returns zero results (the same grep over `packages/` applies once the SDK exists; that directory does not exist yet). | **P0** |
| FR-009 | The active backend MUST be declared in the voice pack manifest or in a runtime configuration file, not hardcoded. | Backend can be changed via manifest edit alone; no recompile required. | **P0** |

### 7.4 Voice Pack Installation and Validation

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-010 | The CLI MUST provide an `install` command that accepts a `.cvpack` file path or URL (local path only in offline mode). | `chiti-voice install <path>` succeeds for a valid pack; appropriate error emitted for invalid pack. | **P0** |
| FR-011 | The pack loader MUST validate the manifest schema version before loading any model files. | A pack with an unsupported schema version is rejected with error `PACK_SCHEMA_MISMATCH` before any model file is read. | **P0** |
| FR-012 | The pack loader MUST verify the SHA-256 checksum of all model files declared in the manifest. | A pack with a tampered model file is rejected with error `PACK_CHECKSUM_FAILED`. | **P0** |
| FR-013 | The pack loader MUST reject any pack containing file paths with path traversal sequences (`../`, absolute paths, symlinks). | A synthetically crafted malicious pack with `../../etc/passwd`-style paths is rejected with `PACK_PATH_TRAVERSAL`. | **P0** |
| FR-014 | The pack loader MUST reject packs with uncompressed content exceeding a configurable size limit (default: 2 GB). | A zip-bomb test pack is rejected with `PACK_SIZE_EXCEEDED` before full decompression. | **P0** |

### 7.5 Persona Intent Mapping

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-015 | The Persona Runtime MUST map a named intent (e.g., `"warm"`, `"alert"`, `"calm"`) to prosody parameters (rate, pitch range, pause duration). | Each persona ships with a default intent map; applying `"warm"` to TARA produces measurably different prosody than default. | **P0** |
| FR-016 | Persona configuration MUST be stored in the `.cvpack` manifest and MUST be structurally separate from the acoustic model files. | Modifying persona prosody defaults requires only manifest edit; acoustic model files are unchanged. | **P0** |
| FR-017 | The system MUST prevent cross-persona bleed — loading TARA must not affect BOBO's prosody parameters. | After loading TARA, BOBO's intent map is retrieved; all values match BOBO's defaults with zero TARA influence. | **P0** |

### 7.6 Text Normalization — Indian English

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-018 | The text normalizer MUST expand ₹ currency amounts to verbal form using Indian number system conventions. | Input: `"₹1,25,000"` → spoken: `"one lakh twenty-five thousand rupees"`. Test suite: 20 currency cases pass. | **P0** |
| FR-019 | The text normalizer MUST expand Indian-format dates (DD/MM/YYYY, DD Month YYYY). | Input: `"14/08/1947"` → spoken: `"fourteenth August nineteen forty-seven"`. Test suite: 15 date format cases pass. | **P0** |
| FR-020 | The text normalizer MUST expand 10-digit Indian mobile numbers to verbal form. | Input: `"98765-43210"` → spoken as grouped digits per locale convention. Test suite: 10 phone number cases pass. | **P0** |
| FR-021 | The text normalizer MUST handle common Indian English abbreviations (e.g., `"PM"`, `"CM"`, `"IAS"`, `"ISRO"`). | Abbreviation is either expanded or spelled as per a configurable lookup table. | **P0** |

### 7.7 Local HTTP API

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-022 | The local daemon MUST expose `POST /v1/speak` accepting JSON with `voice`, `text`, and `format` fields. | Request with valid fields returns HTTP 200 with audio body in specified format within 500 ms (excluding model inference time). | **P0** |
| FR-023 | The local daemon MUST bind exclusively to `127.0.0.1`. | `netstat` inspection of running daemon shows no listener on `0.0.0.0` or any non-loopback interface. | **P0** |
| FR-024 | The local daemon MUST expose `GET /v1/health` returning service status and loaded voice list. | Response is `{ "status": "ok", "voices": [...] }` when daemon is running normally. | **P0** |
| FR-025 | The local daemon MUST expose `POST /v1/stop` to cancel any in-progress synthesis. | Calling `/v1/stop` while synthesis is in progress terminates audio output within 100 ms and returns HTTP 200. | **P0** |

### 7.8 Voice Lab

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-026 | Voice Lab MUST display real-time waveform and spectrogram for synthesized audio. | Waveform renders within 200 ms of synthesis completion for inputs up to 30 seconds. | **P1** |
| FR-027 | Voice Lab MUST support A/B comparison between two loaded voice packs on the same input text. | Both voices synthesize the same input and their audio outputs can be played back alternately from the UI. | **P1** |
| FR-028 | Voice Lab MUST include a benchmark runner reporting TTFA, RTF, and peak memory for any loaded voice. | Benchmark completes and displays results for a standard 10-sentence test suite within 60 seconds. | **P1** |

### 7.9 Web SDK

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-029 | `@chiti/voice-web` MUST expose `ChitiVoice.load(voiceId)` returning a typed voice handle. | TypeScript type check passes. Load call with a valid voice ID succeeds and returns a handle with `speak`, `stream`, and `stop` methods. | **P1** |
| FR-030 | `voice.speak(text)` MUST return a Promise that resolves when audio playback completes. | Awaiting `speak()` in a browser context produces audible output and the Promise resolves after the audio ends. | **P1** |
| FR-031 | `voice.stop()` MUST interrupt in-progress synthesis and playback within 100 ms. | Calling `stop()` during playback silences audio and resolves any pending `speak()` Promise with a `StoppedError`. | **P1** |

### 7.10 Streaming Synthesis

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-032 | The system MUST support streaming PCM output — first audio chunk MUST be available before full text processing is complete. | TTFA for a 200-word input is less than or equal to TTFA for a 20-word input (within 20% margin). | **P1** |
| FR-033 | The WebSocket endpoint MUST emit audio chunks as they are synthesized, not after full synthesis completion. | WebSocket trace shows first audio frame arriving before synthesis of the full input is complete, verified with a 100-word test input. | **P1** |

### 7.11 Barge-In / Cancellation

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-034 | The system MUST support barge-in: cancelling synthesis in response to a user interrupt signal. | Sending a stop signal during active streaming synthesis halts audio output within 100 ms; no further audio chunks are emitted. | **P1** |
| FR-035 | Cancellation MUST release audio resources and leave the engine in a ready state for the next synthesis request. | After cancellation, a new `speak()` call with a fresh input succeeds without requiring a restart. | **P1** |

### 7.12 Pronunciation Dictionary

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-036 | The system MUST support a per-voice custom pronunciation dictionary mapping grapheme sequences to phoneme sequences. | Dictionary entry for `"Chiti"` overrides default G2P output and the correct pronunciation is verified by IPA comparison. | **P1** |
| FR-037 | Voice Lab MUST provide a UI for adding, editing, and testing pronunciation dictionary entries. | A new entry added via the UI takes effect on the next synthesis without restarting the daemon. | **P1** |

### 7.13 Hardware Profiling

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-038 | Vocal Core MUST perform hardware profiling at startup and expose device capability information (CPU cores, RAM, accelerator presence). | `GET /v1/status` response includes `hardware_profile` object with `cpu_cores`, `available_memory_mb`, and `accelerator` fields. | **P1** |
| FR-039 | Voice pack loading MAY automatically select quantization level based on detected hardware profile. | On a device with <2 GB available RAM, a voice pack with multiple quantization levels loads the lowest-quantization model automatically. | **P1** |

### 7.14 Browser-Native WASM / WebGPU Mode

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-040 | The system MUST provide a browser-native synthesis path using WASM (and optionally WebGPU) that requires no local daemon. | `@chiti/voice-web` with WASM bundle loads and speaks in a browser with local daemon disabled. | **P2** |
| FR-041 | WASM build MUST pass the offline synthesis test (FR-001) within the browser sandbox. | Service Worker caches all model assets; synthesis completes with browser DevTools network throttle set to "Offline". | **P2** |

### 7.15 Pack Signing and Provenance

| ID | Description | Acceptance Criteria | Priority |
|----|-------------|---------------------|----------|
| FR-042 | The `.cvpack` format MUST include a `provenance` manifest section with training data statement, consent attestation, and publisher identity fields. | All three persona packs ship with populated provenance fields; missing provenance fields cause a `PACK_PROVENANCE_INCOMPLETE` warning (not error, until signing is enforced). | **P2** |
| FR-043 | The system MUST support cryptographic signing of voice packs (Ed25519 signature over manifest hash). | A pack with a valid signature is marked `SIGNED`; an unsigned pack is marked `UNSIGNED` and a warning is emitted; a pack with an invalid signature is rejected. | **P2** |

---

## 8. Non-Functional Requirements

### 8.1 Latency

| Requirement | Threshold | Measurement Method |
|-------------|----------|-------------------|
| Warm command overhead (excluding model inference) | < 100 ms | Benchmark with model already loaded; measure time from API call receipt to first model invocation |
| Time to First Audio (TTFA) — streaming | < 500 ms on modern CPU for ≤ 50 words | Benchmark runner in Voice Lab; automated regression test |
| End-to-end speak() for short utterance (≤ 10 words) | < 1500 ms on reference hardware | Integration test on reference hardware spec |

### 8.2 Throughput

> **RTF (Real-Time Factor) MUST be less than 1.0 on modern CPU hardware.**
>
> RTF < 1.0 means synthesis is faster than playback — the voice engine is rendering audio faster than it would take to speak it. RTF > 1.0 is unacceptable for any production voice pack.

| Requirement | Threshold |
|-------------|----------|
| RTF on modern CPU (≥ 4 cores, ≥ 8 GB RAM) | < 0.5 (target), < 1.0 (minimum gate) |
| RTF on constrained hardware (≥ 2 cores, ≥ 2 GB RAM) | < 1.0 |

### 8.3 Memory

| Requirement | Threshold |
|-------------|----------|
| Memory limit | Configurable per deployment; default limit must be documented and enforced |
| Memory growth during sustained synthesis (60 minutes) | No unbounded growth; steady-state memory must stabilize |
| Voice pack hot-swap memory impact | Unloading a voice must release its memory within 500 ms |

### 8.4 Privacy

| Requirement | Specification |
|-------------|--------------|
| No telemetry | Zero outbound telemetry calls in any build configuration |
| No text logging | Input text is NOT logged at any level by default; requires explicit `TRACE` flag opt-in |
| No audio upload | Synthesized audio is NEVER sent off-device in local mode |
| No user identity | The runtime collects no device ID, user ID, or fingerprint |

### 8.5 Security

| Requirement | Specification |
|-------------|--------------|
| Loopback binding | Local daemon binds exclusively to `127.0.0.1`; this is not configurable to `0.0.0.0` |
| No CORS wildcard | The daemon does not emit `Access-Control-Allow-Origin: *`; origin is checked against allowlist |
| Pack security | See Section 9 and FR-012 through FR-014 |

### 8.6 Offline

| Requirement | Specification |
|-------------|--------------|
| Network independence | Synthesis works with network interface physically disabled |
| Automated offline test | CI pipeline includes a network-blocking integration test (using OS firewall rules or network namespace) that must pass before any release |
| Zero external model fetches | No voice pack or runtime component fetches model weights from any URL at synthesis time |

---

## 9. Voice Pack (`.cvpack`) Specification Summary

### 9.1 Container Format

| Property | Value |
|----------|-------|
| **File extension** | `.cvpack` |
| **Internal format** | ZIP archive (PKWARE ZIP, Deflate compression) |
| **Extraction policy** | Never extracted to temp directories in production; read in-memory via streaming ZIP reader |

### 9.2 Required Files

| File | Description |
|------|-------------|
| `manifest.json` | Pack manifest — required, must be the first entry in the ZIP |
| `model/` | Directory containing all model weight files declared in manifest |
| `phonemes/` | G2P tables, phoneme dictionaries, and IPA mappings |
| `persona.json` | Persona configuration: prosody defaults, intent map, supported locales |

### 9.3 Manifest Fields

```jsonc
{
  "schema": "cvpack/1.0",              // Schema version — validated before any other field
  "id": "com.chiti.voice.tara",        // Reverse-domain unique voice ID
  "version": "1.0.0",                  // Semantic version of this voice pack
  "display_name": "Tara",              // Human-readable name
  "publisher": "Chiti Technologies",   // Publisher name
  "languages": ["en-IN", "hi-IN"],     // BCP-47 language tags
  "engine_family": "kokoro",           // Backend engine family required
  "engine_version_min": "0.9.0",       // Minimum engine version for compatibility
  "models": [
    {
      "path": "model/tara_base.onnx",
      "role": "acoustic",
      "sha256": "<hex-digest>",
      "size_bytes": 87654321
    }
  ],
  "provenance": {
    "training_data_statement": "...",  // Plain-language description of training data
    "consent_attestation": "...",      // Consent model statement
    "created_date": "2026-09-01",
    "publisher_contact": "voice@chiti.in"
  },
  "signature": {                       // Present only when signed (Phase 11+)
    "algorithm": "Ed25519",
    "value": "<base64-signature>",
    "signed_by": "keys.chiti.in/voice-signing-2026"
  }
}
```

### 9.4 Security Rules

| Rule | Description |
|------|-------------|
| **Path traversal** | All paths in manifest must be relative, within the pack root, and contain no `..` sequences |
| **Absolute paths** | Absolute paths in manifest are rejected |
| **Symlinks** | Symbolic links inside the ZIP are rejected |
| **Executables** | No executable content (`.exe`, `.sh`, `.py`, `.so`, `.dll`) may appear in a voice pack |
| **Zip bomb** | Uncompressed size limit enforced before decompression begins |
| **Manifest position** | `manifest.json` must be the first file in the ZIP; packs where it appears elsewhere are rejected |

### 9.5 Schema Versioning

- Schema version is `cvpack/<major>.<minor>`.
- The runtime MUST reject any pack with an unsupported schema major version.
- Schema minor version increments are backward-compatible.
- Breaking manifest changes require a major version bump.

---

## 10. System Invariants Summary

These invariants are non-negotiable architectural rules. Violation of any invariant is a critical defect regardless of whether tests pass.

> **Invariant IDs follow `docs/architecture/INVARIANTS.md`, which is the source of truth.** Code
> comments, `SECURITY.md` and `STATE_MACHINE.md` all cite those IDs, and `scripts/verify-doc-claims.py`
> fails a document that pairs an ID with a different name. This table previously re-used IDs
> `VOICE_INV_003`-`012` for seven *different* requirements, so "implement VOICE_INV_008" meant
> loopback binding here and voice provenance everywhere else. Rows marked **PRD-only** state real
> requirements that no canonical invariant covers yet; they carry no ID rather than a conflicting one.



| ID | Name | Statement | Testable |
|----|------|-----------|----------|
| VOICE_INV_001 | Offline Independence | Synthesis MUST complete with no network connectivity. | Yes |
| VOICE_INV_002 | LLM Independence | The synthesis pipeline MUST NOT require any LLM component. | Yes |
| -- | Language-Voice Separation (PRD-only) | The voice layer accepts text; it does not generate it. No canonical invariant defines this yet. | Yes (dependency audit) |
| -- | Engine Interface (PRD-only) | All TTS backends MUST be accessed exclusively through the `VoiceEngine` interface. No canonical invariant defines this yet. | Yes (static analysis) |
| -- | No Direct Backend Instantiation (PRD-only) | No application or library outside of `vocal-core` MAY directly instantiate a backend. | Yes (code review gate) |
| VOICE_INV_004 | Persona Independence | Loading or using one persona MUST NOT affect another persona's parameters. | Yes |
| -- | Persona Config Separation (PRD-only) | Persona configuration is structurally separate from acoustic model files in every voice pack. | Yes (schema validation) |
| -- | Loopback Only (PRD-only, a clause of VOICE_INV_007) | The local daemon MUST bind only to `127.0.0.1`. | Yes (network inspection) |
| -- | No Telemetry (PRD-only, a clause of VOICE_INV_007) | The runtime MUST emit zero outbound telemetry in any build configuration. | Yes (network audit) |
| -- | Pack Integrity (PRD-only, enforced by VOICE_INV_008) | A voice pack with a failed checksum MUST be rejected before any model files are loaded. | Yes |
| -- | No Executable Content (PRD-only) | Voice packs MUST NOT contain any executable files; any such pack is rejected. | Yes |
| -- | RTF Bound (PRD-only) | No production voice pack MAY ship with an RTF >= 1.0 on reference hardware. | Yes (benchmark gate) |

---

## 11. Architecture Overview

### 11.1 Synthesis Pipeline

```
Application
    │
    ▼
Vocal Client API  (voice.speak / HTTP POST /v1/speak / WebSocket)
    │
    ▼
Input Normalizer  (currency, dates, phone numbers, abbreviations)
    │
    ▼
Language Router   (locale detection, script detection, language segmentation)
    │
    ▼
G2P Layer         (grapheme-to-phoneme; custom dictionary lookup → fallback model)
    │
    ▼
Voice Intent      (intent signal injection: "warm", "calm", "alert", default)
    │
    ▼
Persona Runtime   (intent → prosody parameters; rate, pitch, pause, energy)
    │
    ▼
Prosody Planner   (sentence-level timing, emphasis, boundary detection)
    │
    ▼
Voice Engine API  (VoiceEngine trait dispatch)
    │
    ├──▶ Backend: Kokoro (Phase 0–current)
    ├──▶ Backend: Piper  (Phase 0–current, ADR-001 pending)
    └──▶ Backend: Future (Native Acoustic Engine, research)
    │
    ▼
PCM Output        (raw float32 PCM frames)
    │
    ▼
Audio Pipeline    (resampling, format conversion, streaming chunking)
    │
    ▼
Output Target
    ├──▶ System speaker (local audio device)
    ├──▶ WAV file
    └──▶ HTTP/WebSocket stream (chunked PCM / OGG)
```

### 11.2 Key Architectural Rules

> **`VoiceEngine` is an interface, not a class.** No code outside of `vocal-core` engine registry may reference a concrete backend type.

> **No application may directly instantiate a backend.** All synthesis requests flow through the `VoiceEngine` trait and the Vocal Core routing layer.

> **Persona config is separate from acoustic model.** The persona manifest section and `persona.json` control prosody; the acoustic model is a passive model artifact. Changing one does not require rebuilding the other.

> **LLM adapters are optional and always external.** They sit upstream of the Vocal Client API. The synthesis pipeline receives text; what generated that text is irrelevant and invisible to the runtime.

> **The synthesis pipeline is synchronous in model of operation.** Streaming is achieved by chunking the output of a pipeline that processes text in segments — not by making the pipeline itself asynchronous in its internal logic.

---

## 12. Development Phases

| Phase | Name | Key Deliverables | Exit Condition |
|-------|------|-----------------|----------------|
| **Phase 0** | Foundation | Monorepo scaffold; `VoiceEngine` trait defined; all 12 invariants codified in `INVARIANTS.md`; ADR-001 backend decision record written (even if decision is pending) | Repository compiles; invariant document merged |
| **Phase 1** | Heartbeat | One voice (TARA) loads from a `.cvpack` file and speaks a sentence via CLI; offline synthesis test passes | `chiti-voice speak --voice tara "Hello"` produces audio with no internet; offline test: PASS |
| **Phase 2** | Local Service | HTTP daemon on `127.0.0.1:7731`; `POST /v1/speak` returns PCM; `GET /v1/health` works; loopback-only binding verified | Daemon starts, serves a speak request, passes loopback binding test |
| **Phase 3** | Web SDK | `@chiti/voice-web` npm package connects to local daemon; `ChitiVoice.load()` and `voice.speak()` work in browser and Node.js | Browser demo page speaks using SDK with daemon running |
| **Phase 4** | Three Voices | TARA, KASHI, and BOBO all load, speak, and pass their critical evaluation sentences | All three voices produce audible output; evaluation sentences reviewed and approved |
| **Phase 5** | Persona Runtime | Intent-to-prosody mapping wired end-to-end; measurable prosody difference between `"warm"` and `"calm"` intents on TARA | Benchmark shows statistically significant rate/pitch difference between intents |
| **Phase 6** | Text Normalization | Indian English: ₹ currency, dates, phone numbers, abbreviations all normalized correctly | 45-case normalization test suite passes (FR-018 through FR-021) |
| **Phase 7** | Voice Lab v0 | Tauri desktop app ships; load voice, type text, hear output, view waveform; A/B comparison; benchmark runner | Voice Lab builds and ships; TARA A/B against KASHI works |
| **Phase 8** | Streaming | WebSocket streaming endpoint; TTFA regression test added; barge-in and cancellation implemented | TTFA benchmark: first chunk arrives before full text synthesis completes |
| **Phase 9** | Pronunciation | Per-voice custom dictionary; G2P override; Voice Lab pronunciation editor | "Chiti" pronounces correctly from dictionary override; editor UI functional |
| **Phase 10** | Pack Security | Path traversal test; zip bomb test; executable rejection test; all pass in CI | Malicious pack tests: all three attack vectors rejected with correct error codes |
| **Phase 11** | Signing & Provenance | Ed25519 pack signing; SIGNED/UNSIGNED status; provenance fields in all three packs | Signed pack: SIGNED status. Tampered pack: rejected. Unsigned pack: UNSIGNED warning. |
| **Phase 12** | Browser Native | WASM synthesis in browser without daemon; Service Worker offline model caching | Browser with daemon killed speaks using WASM; DevTools Offline mode test passes |

---

## 13. Performance Targets

| Metric | Phase 1 Target | Research Target | North-Star |
|--------|---------------|----------------|------------|
| **TTFA (Time to First Audio)** | < 1500 ms | < 500 ms | < 200 ms (streaming) |
| **RTF (Real-Time Factor)** | < 1.0 (gate) | < 0.5 | < 0.2 (native engine) |
| **Peak Memory (single voice)** | < 500 MB | < 256 MB | < 128 MB (quantized) |
| **Voice Pack Size** | < 300 MB | < 100 MB | 1–10 MB (adapter model) |
| **Vocal Core Binary Size** | < 50 MB | < 30 MB | 20–40 MB (native engine) |
| **Warm Command Overhead** | < 200 ms | < 100 ms | < 50 ms |

> **North-star hypothesis:** A shared small speech foundation model of 20–40 MB plus a per-voice adapter model of 1–10 MB — analogous to how a font engine is shared across fonts. This is a research goal, not a committed shipping target.

---

## 14. Quality Gates for v0.1

The following checklist must be fully satisfied before v0.1 is tagged for release. No item may be waived.

- [ ] **Build passes** — monorepo builds cleanly on CI with no errors and no suppressed warnings
- [ ] **Unit tests pass** — all unit tests pass with zero failures; coverage ≥ 70% for `vocal-core`
- [ ] **Offline synthesis test passes** — automated test with OS-level network blocking confirms synthesis completes
- [ ] **Three voices load and speak** — TARA, KASHI, and BOBO each load and produce audible, intelligible output
- [ ] **Stop/cancellation works** — `/v1/stop` and `voice.stop()` halt synthesis within 100 ms
- [ ] **Corrupt pack rejected** — a pack with a tampered model file checksum is rejected with `PACK_CHECKSUM_FAILED`
- [ ] **Path traversal pack rejected** — a malicious pack with `../` paths is rejected with `PACK_PATH_TRAVERSAL`
- [ ] **No LLM dependency** — dependency audit finds zero LLM-related packages in `vocal-core` and `voice-web`
- [ ] **No cloud dependency** — dependency audit finds zero outbound HTTP clients in synthesis path
- [ ] **Licenses documented** — the catalogue exists (`docs/LICENSES_THIRD_PARTY.md`, written 2026-09-03) and records what is verified; the box stays unchecked because two entries there are still open: Kokoro's weight licence and the crate-level audit
- [ ] **RTF ≤ 1.0** — benchmark on reference hardware shows RTF < 1.0 for all three voices
- [ ] **Loopback binding verified** — `netstat` inspection confirms daemon is not bound to `0.0.0.0`
- [ ] **Error codes tested** — all typed errors in Section 15 have at least one test that triggers and verifies the error

---

## 15. Error Model

All errors are structured typed values with a stable machine-readable code. User-facing messages are advisory — applications should localize them; they MUST NOT log the input text in error messages by default.

| Error Code | Description | User-Facing Message Policy |
|------------|-------------|---------------------------|
| `VOICE_NOT_FOUND` | Requested voice ID is not installed or not loaded | "Voice '[id]' is not available. Install the voice pack and try again." |
| `PACK_NOT_FOUND` | `.cvpack` file not found at specified path | "Voice pack file not found." |
| `PACK_INVALID_FORMAT` | File is not a valid ZIP or is structurally malformed | "Voice pack format is invalid." |
| `PACK_SCHEMA_MISMATCH` | Manifest schema version is not supported by this runtime | "Voice pack schema version is not supported. Update Chiti Vocal Runtime." |
| `PACK_CHECKSUM_FAILED` | One or more model files failed SHA-256 verification | "Voice pack integrity check failed. The file may be corrupted." |
| `PACK_PATH_TRAVERSAL` | Pack contains path traversal sequences | "Voice pack was rejected for security reasons." |
| `PACK_SIZE_EXCEEDED` | Uncompressed pack content exceeds size limit | "Voice pack exceeds maximum allowed size." |
| `PACK_EXECUTABLE_CONTENT` | Pack contains executable file types | "Voice pack was rejected for security reasons." |
| `PACK_PROVENANCE_INCOMPLETE` | Required provenance fields are missing (warning, not error until Phase 11) | "Voice pack is missing provenance information." |
| `ENGINE_NOT_AVAILABLE` | Required backend engine is not installed or incompatible | "The voice engine required by this voice pack is not available." |
| `ENGINE_VERSION_MISMATCH` | Backend engine version is below `engine_version_min` | "Voice engine version is too old. Update the engine." |
| `SYNTHESIS_FAILED` | Backend returned an error during synthesis | "Voice synthesis failed. Check logs for details." |
| `SYNTHESIS_CANCELLED` | Synthesis was stopped by a barge-in or stop signal | Internal only — not shown to end users |
| `NORMALIZATION_FAILED` | Text normalization pipeline encountered an unrecoverable error | "Could not process the input text." |
| `LOCALE_NOT_SUPPORTED` | Requested locale is not supported by the loaded voice | "This voice does not support the requested language." |
| `DAEMON_NOT_RUNNING` | Web SDK cannot connect to local daemon | "Chiti Vocal Runtime is not running. Start the Vocal Local Service." |
| `DAEMON_AUTH_FAILED` | Origin not on allowlist (future) | "Request origin is not permitted." |
| `AUDIO_DEVICE_ERROR` | System audio device is unavailable or failed | "Could not access audio output device." |

---

## 16. Observability Requirements

### 16.1 Logging

| Requirement | Specification |
|-------------|--------------|
| **Format** | Structured JSON logs on all outputs; human-readable format available via `--log-format=text` flag |
| **Default level** | `INFO` |
| **Levels supported** | `ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE` |
| **User text logging** | Input text to synthesis is NEVER logged at `ERROR`, `WARN`, `INFO`, or `DEBUG` levels. It MAY appear at `TRACE` level only, with explicit opt-in. |
| **Audio logging** | Synthesized audio is never written to log output |

### 16.2 Log Fields

Every structured log entry MUST include:

```json
{
  "timestamp": "2026-09-01T02:39:43.000Z",
  "level": "INFO",
  "component": "vocal-core::pack_loader",
  "message": "Voice pack loaded successfully",
  "voice_id": "com.chiti.voice.tara",
  "duration_ms": 142
}
```

Fields containing user content (text, audio paths) are excluded from all levels except `TRACE`.

### 16.3 Network Isolation Test

A dedicated CI test verifies that no log entry triggers an outbound network connection. This test runs using a network namespace (Linux) or Windows Filtering Platform rules, with all non-loopback traffic blocked, and verifies that the full synthesis pipeline completes without any network-related log entry or error.

---

## 17. Security Requirements

### 17.1 Network Security

| Requirement | Specification |
|-------------|--------------|
| **Loopback binding** | Local daemon binds to `127.0.0.1` only. This is hardcoded, not configurable. |
| **Origin allowlist** | HTTP and WebSocket requests to the daemon are validated against a configurable origin allowlist. Default allowlist: `["http://localhost", "http://127.0.0.1"]` |
| **No CORS wildcard** | The daemon MUST NOT emit `Access-Control-Allow-Origin: *`. Any origin not on the allowlist receives HTTP 403. |
| **No remote administration** | No administrative endpoint is accessible from any non-loopback address. |

### 17.2 Pack Security

| Requirement | Specification |
|-------------|--------------|
| **Path traversal** | All paths in pack manifest are validated before any file extraction or reading |
| **Zip bomb** | Uncompressed size limit enforced by reading ZIP central directory before decompression |
| **Executable content** | File extension allowlist enforced; any file with executable extension causes `PACK_EXECUTABLE_CONTENT` rejection |
| **Symlinks** | ZIP entries that are symlinks are rejected |
| **Checksum** | SHA-256 of each model file verified against manifest before loading into memory |

### 17.3 Signing (Phase 11+)

| Status | Behaviour |
|--------|-----------|
| `SIGNED` | Pack has a valid Ed25519 signature verifiable against the publisher's public key |
| `UNSIGNED` | Pack lacks a signature block; a `WARN`-level log is emitted; pack is still loaded (until signing enforcement is activated) |
| `SIGNATURE_INVALID` | Pack has a signature block that fails verification; pack is rejected regardless of signing enforcement status |

> **Cryptographic signing infrastructure (key management, Certificate Transparency equivalent, revocation) is deferred to Phase 11 and requires a dedicated security design document (SEC-001) before implementation.**

---

## 18. Long-Term Products (Future Scope)

These products are not in scope for v0.1 through v0.5 but inform architectural decisions made today.

| Product | Description |
|---------|-------------|
| **Chiti Voice Foundry** | A platform for voice designers to record, train, and publish custom voice packs with full consent and licensing tooling |
| **Chiti Voice Compiler** | A toolchain that takes a voice recording dataset and a persona spec and produces a `.cvpack` — the compiler from voice data to voice pack |
| **Chiti Voice Registry** | A signed registry of published voice packs, with provenance verification, version resolution, and install-by-ID support (`chiti-voice install com.chiti.voice.tara`) |
| **Native Acoustic Engine** | Chiti-developed inference engine replacing ONNX Runtime dependency for maximum performance and minimum binary size on target hardware |
| **Chiti Vocal Micro** | Stripped-down Vocal Core for deeply embedded and microcontroller targets (RTOS, bare metal); sub-10 MB footprint goal |

---

## 19. Research Track

### 19.1 North-Star Architecture Hypothesis

> **A shared small speech foundation model (20–40 MB) plus a per-voice adapter model (1–10 MB) is sufficient to reproduce a recognizable, high-quality vocal persona. This is the font analogy applied to model architecture.**

This hypothesis is not a committed engineering target. It is a research direction that, if proven, would make Chiti Vocal Runtime competitive on embedded hardware and eliminate the current dependency on 100–300 MB single-voice models.

### 19.2 Research Questions

| Question | Exploration Approach |
|----------|---------------------|
| What is the minimum model size at which voice persona is recognizably distinct and consistent? | Train adapter models at decreasing sizes; evaluate on blind persona recognition test |
| What phoneme representation produces best quality at lowest model size? | Compare ARPABET, IPA, and learned phoneme embeddings across model sizes |
| What is the minimum vocoder size that produces intelligible, natural audio? | Evaluate HiFi-GAN, iSTFT-Net, and research vocoders at aggressive quantization levels |
| What quantization level preserves naturalness while minimizing memory footprint? | INT8, INT4, mixed-precision — evaluate via MOS (Mean Opinion Score) proxy tests |
| Can duration, pitch, and energy prediction be separated from the acoustic model without quality loss? | Ablation study: shared vs. per-voice duration/pitch/energy predictors |
| What is the minimum RTF achievable on a Raspberry Pi 5 class device? | Benchmark all candidates on reference embedded hardware |

### 19.3 Architecture Candidates Under Research

- **Shared foundation + adapter:** shared phoneme encoder and vocoder; per-voice pitch/energy/duration adapter (< 10 MB)
- **Full per-voice model:** current approach (Kokoro/Piper); good quality, larger footprint
- **Diffusion-based vocoder:** higher quality ceiling; higher compute cost — research only for now
- **Neural codec language model (VALL-E style):** requires LLM-class inference; explicitly excluded from production path (violates VOICE_INV_002)

---

## 20. Open Questions / Decisions Pending

| # | Question | Owner | Status | Due |
|---|----------|-------|--------|-----|
| OQ-001 | **Primary offline TTS backend choice: Kokoro vs Piper?** Kokoro offers better voice quality and Python ecosystem; Piper offers C++ portability and embedded readiness. ADR-001 must be written and decided before Phase 1 exit. | Engineering Lead | ⏳ ADR-001 pending | Phase 0 exit |
| OQ-002 | **Browser-native synthesis timeline.** Phase 12 is the target, but WASM model size and WebGPU browser support maturity need assessment before committing to a date. | Platform Lead | ⏳ Assessment pending | Phase 8 |
| OQ-003 | **Voice Foundry consent and licensing framework.** When the Foundry is built, what legal and ethical framework governs consent from voice actors? What rights does the publisher retain? This requires legal counsel input before any voice recording program begins. | Legal / Product | ⏳ Not started | Pre-Foundry |
| OQ-004 | **Cryptographic signing infrastructure.** Ed25519 key generation, distribution, storage, and revocation need a full security design document (SEC-001) before Phase 11 implementation begins. | Security Lead | ⏳ Not started | Phase 10 exit |
| OQ-005 | **Default port conflict policy.** Port 7731 is the proposed default daemon port. Conflict detection and fallback behavior (port scanning, config override, user notification) need a decision. | Engineering | ⏳ Open | Phase 2 |
| OQ-006 | **Daemon auto-start strategy.** Should the Vocal Local Service auto-start on login (launchd / systemd / Windows Service), or require explicit user start? OS permissions, user trust, and battery impact need assessment. | Platform | ⏳ Open | Phase 3 |
| OQ-007 | **KASHI Sanskrit phoneme coverage.** The depth of Sanskrit vocabulary support in KASHI's G2P model is undefined. A word list and coverage target must be agreed before Phase 4. | Linguistics / Voice | ⏳ Open | Phase 3 |

---

*Document owned by Chiti Technologies. All rights reserved. Internal use only.*

*For questions, corrections, or status updates, contact the Chiti Vocal Runtime product team.*

---

<div align="center">

**Chiti Technologies** · Chiti Vocal Runtime PRD v0.1.0-draft · September 2026

</div>
