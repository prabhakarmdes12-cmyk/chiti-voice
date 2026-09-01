# Chiti Vocal Runtime

> **The voice infrastructure layer for software, agents, robots, and the web.**

[![Rust](https://img.shields.io/badge/Rust-1.78+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.5+-3178C6?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Python](https://img.shields.io/badge/Python-3.11+-3776AB?style=flat-square&logo=python&logoColor=white)](https://www.python.org/)
[![ONNX Runtime](https://img.shields.io/badge/ONNX_Runtime-1.18+-005CED?style=flat-square&logo=onnx&logoColor=white)](https://onnxruntime.ai/)
[![Tailwind CSS](https://img.shields.io/badge/Tailwind_CSS-3.4+-06B6D4?style=flat-square&logo=tailwindcss&logoColor=white)](https://tailwindcss.com/)
[![License](https://img.shields.io/badge/License-Proprietary-red?style=flat-square)](./LICENSE)

---

Chiti Vocal Runtime is an **offline-first, LLM-independent voice infrastructure platform** for software. It gives every application, agent, robot, and website a persistent, portable, and provenance-aware vocal identity — without a cloud API, without an LLM, without a bespoke wrapper.

A `.cvpack` voice file is to speech what a `.ttf` font file is to text rendering: installable, portable, composable, and licensable.

---

## Architectural Invariant

> **`LANGUAGE GENERATION != VOICE GENERATION`**
>
> Chiti Vocal Runtime is **not** a text generator. It is a voice renderer. It accepts text and renders it as audio. It does not decide *what* to say — only *how* to say it. LLM integration is optional, external, and always upstream.

---

## Offline-First Principle

Synthesis **must** work with the network cable physically unplugged. The runtime holds zero tolerance for synthesis paths that require any outbound network call. An automated network-blocking test is a required quality gate before any release.

---

## Quick Start

### CLI

```bash
# Install a voice pack
chiti-voice install tara.cvpack

# Speak using the installed voice
chiti-voice speak --voice tara "Welcome to Chiti Vocal Runtime."
```

### TypeScript (Web / Node.js)

```ts
import { ChitiVoice } from "@chiti/voice-web";

const voice = await ChitiVoice.load("tara");
await voice.speak("Welcome. How may I help you?");
```

### HTTP API (Local Daemon)

```http
POST http://127.0.0.1:7731/v1/speak
Content-Type: application/json

{
  "voice": "tara",
  "text": "Your order has been confirmed.",
  "format": "pcm_f32",
  "stream": true
}
```

---

## Personas

Chiti Vocal Runtime ships with three reference personas:

| Persona | Identity | Primary Use |
|---------|----------|-------------|
| **TARA** | Warm, professional, Indian English female-presenting | Business, hospitality, customer interfaces |
| **KASHI** | Calm, measured, Hindi/Sanskrit-aware male-presenting | Guidance, knowledge delivery, navigation |
| **BOBO** | High-expressiveness, playful, fictional non-child character | Children's products, toys, robots |

---

## Six Logical Products

| Product | Description |
|---------|-------------|
| **Chiti Vocal Core** | The core runtime engine abstraction and synthesis pipeline — provider-agnostic, backend-swappable |
| **Chiti Voice Pack (`.cvpack`)** | Portable, versioned, signed, provenance-aware voice package format |
| **Chiti Persona Runtime** | Maps text + intent + persona metadata → synthesis parameters and prosody |
| **Chiti Vocal Local Service** | Loopback HTTP/WebSocket daemon enabling any local application to call voice synthesis |
| **Chiti Voice Web SDK** | `@chiti/voice-web` TypeScript SDK for browser and Node.js applications |
| **Chiti Voice Lab** | Developer GUI for testing, comparing, benchmarking, and tuning voices |

---

## Monorepo Structure

```
chiti-voice/
├── apps/               # Runnable applications
│   ├── vocal-local/    # Local HTTP/WS daemon
│   └── voice-lab/      # Developer GUI (Tauri + React)
├── packages/           # Shared libraries and SDKs
│   ├── vocal-core/     # Core runtime (Rust)
│   ├── voice-web/      # @chiti/voice-web TypeScript SDK
│   └── vocal-types/    # Shared type definitions
├── engines/            # Voice engine backend adapters
│   ├── engine-kokoro/  # Kokoro TTS adapter
│   ├── engine-piper/   # Piper TTS adapter
│   └── engine-onnx/    # Generic ONNX backend
├── voices/             # Voice pack sources and build tooling
│   ├── tara/
│   ├── kashi/
│   └── bobo/
├── tools/              # Build tools, CLI, pack builder
│   ├── chiti-voice-cli/
│   └── cvpack-builder/
├── research/           # Model experiments, benchmarks, architecture explorations
├── tests/              # Integration, offline, and E2E tests
└── docs/               # Specification documents
```

| Directory | Contents |
|-----------|----------|
| `apps/` | Runnable application targets |
| `packages/` | Shared runtime libraries and SDKs |
| `engines/` | Pluggable TTS backend adapters |
| `voices/` | Voice pack source data and build configs |
| `tools/` | CLI, pack builder, dev utilities |
| `research/` | Model experiments and benchmarking |
| `tests/` | Integration, offline blocking, and E2E test suites |
| `docs/` | All specification and architecture documents |

---

## Development Phases

| Phase | Name | Key Deliverable |
|-------|------|----------------|
| **Phase 0** | Foundation | Monorepo scaffold, invariants codified, ADR-001 backend decision |
| **Phase 1** | Heartbeat | One voice speaks offline from CLI — TARA says "Hello" from `.cvpack` |
| **Phase 2** | Local Service | HTTP daemon running on loopback, `/v1/speak` returns PCM |
| **Phase 3** | Web SDK | `@chiti/voice-web` connects to local daemon, works in browser |
| **Phase 4** | Three Voices | TARA, KASHI, BOBO all load, speak, and pass evaluation sentences |
| **Phase 5** | Persona Runtime | Intent-to-prosody mapping, emotion/style parameters wired end-to-end |
| **Phase 6** | Text Normalization | Indian English: ₹ currency, dates, phone numbers, abbreviations |
| **Phase 7** | Voice Lab v0 | Developer GUI ships: load, speak, compare, waveform view |
| **Phase 8** | Streaming | Real-time PCM/WAV streaming, barge-in, cancellation |
| **Phase 9** | Pronunciation | Custom dictionary, word-level G2P override, IPA editor in Voice Lab |
| **Phase 10** | Pack Security | Checksum validation, path traversal guard, zip bomb protection |
| **Phase 11** | Signing & Provenance | Cryptographic pack signing, publisher verification, UNSIGNED status |
| **Phase 12** | Browser Native | WASM/WebGPU in-browser synthesis — no daemon required |

---

## Documentation

| Document | Description |
|----------|-------------|
| [`docs/SYSTEM_OVERVIEW.md`](./docs/SYSTEM_OVERVIEW.md) | End-to-end architecture walkthrough |
| [`docs/INVARIANTS.md`](./docs/INVARIANTS.md) | All 12 system invariants (VOICE_INV_001–012) |
| [`docs/HTTP_API.md`](./docs/HTTP_API.md) | Local daemon HTTP and WebSocket API reference |
| [`docs/CVPACK_SPECIFICATION.md`](./docs/CVPACK_SPECIFICATION.md) | `.cvpack` format, manifest schema, security rules |
| [`PRD.md`](./PRD.md) | Full Product Requirements Document |

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Core Runtime | Rust (vocal-core, engine adapters, local daemon) |
| Voice Lab UI | Tauri + React + Tailwind CSS |
| SDK | TypeScript (`@chiti/voice-web`) |
| Inference | ONNX Runtime (CPU, with GPU roadmap) |
| Research | Python + PyTorch |
| Pack Format | ZIP + JSON manifest + model blobs |
| CLI | Rust + Clap |

---

## Contributing

Chiti Vocal Runtime is proprietary software owned by Chiti Technologies. Contribution guidelines, code of conduct, and CLA details will be published prior to any external contributor program opening.

For internal contributors, see `docs/CONTRIBUTING_INTERNAL.md`.

---

## License

Copyright © 2026 Chiti Technologies. All rights reserved.

This software and all associated assets are proprietary to Chiti Technologies. Redistribution, modification, or use outside of a valid Chiti Technologies license agreement is strictly prohibited. See [LICENSE](./LICENSE) for full terms.

---

<div align="center">

**Chiti Technologies** · Building infrastructure for intelligent software.

*Chiti Vocal Runtime is a Chiti Technologies product.*

</div>
