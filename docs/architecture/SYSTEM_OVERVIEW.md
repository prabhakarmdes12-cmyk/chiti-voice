# Chiti Vocal Runtime — System Overview

> **Version:** 0.1.0-alpha  
> **Status:** Architecture Definition  
> **Last Updated:** September 2026  
> **Owner:** Chiti Platform Team

---

## Mission

Build an **offline-first voice platform** that gives applications a reliable, private, high-quality voice without depending on any external service.

The key architectural invariant, stated plainly:

```
LANGUAGE GENERATION  !=  VOICE GENERATION
```

The text that gets spoken may come from a rules engine, a local LLM, a cloud LLM, a static script, or a database query. **The Chiti Vocal Runtime does not care.** Its sole job is to convert text into high-quality, persona-consistent audio — deterministically, privately, and fast.

The runtime must work with:

- ❌ Internet: **OFF**
- ❌ LLM: **OFF**
- ❌ Cloud APIs: **OFF**

Any architecture that violates this is out of scope for the core runtime.

---

## Six Logical Products

The Chiti Vocal Runtime is not a single library. It is a family of six products that together form the complete platform.

| # | Product | Description | Primary Technology | Interface Type |
|---|---------|-------------|-------------------|----------------|
| 1 | **Chiti Vocal Core** | The offline TTS synthesis engine. Converts text to audio via persona-aware synthesis pipeline. Runs on-device. | Rust | Native library (C FFI / WASM) |
| 2 | **Chiti Voice Pack (`.cvpack`)** | Portable, signed, self-contained voice model bundle. Includes model weights, phoneme tables, persona config, and manifest. | ONNX + ZIP archive | File format / Package |
| 3 | **Chiti Persona Runtime** | Maps application intent and persona configuration to engine synthesis parameters. Decouples persona definition from engine internals. | Rust (embedded in Core) | Internal API |
| 4 | **Chiti Vocal Local Service** | A localhost HTTP/WebSocket daemon that exposes the Vocal Core to web browsers and other local processes. Handles security, queueing, and origin validation. | Rust (Axum) | HTTP + WebSocket |
| 5 | **Chiti Voice Web SDK** | TypeScript SDK for web applications. Connects to the Local Service or drives WASM-compiled Core directly in-browser. Abstracts execution mode from the application. | TypeScript | npm package |
| 6 | **Chiti Voice Lab** | Developer workbench UI for testing voices, tuning personas, benchmarking models, and authoring voice packs. | TypeScript (React) + Web SDK | Electron / Web App |

---

## Synthesis Pipeline

The full path from application text to speaker audio:

```
APPLICATION
    │
    │  text + SynthesisRequest
    ▼
┌─────────────────────┐
│   Vocal Client API  │  ← TypeScript SDK or Native API
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│  Input Normalizer   │  ← number expansion, abbreviation resolution,
└─────────────────────┘    unicode normalization, sentence segmentation
    │
    ▼
┌─────────────────────┐
│  Language Router    │  ← detects script / language tag,
└─────────────────────┘    routes to correct G2P and voice model
    │
    ▼
┌─────────────────────┐
│     G2P Layer       │  ← Grapheme-to-Phoneme conversion
└─────────────────────┘    (language-specific phoneme tables)
    │
    ▼
┌─────────────────────┐
│   Voice Intent      │  ← application-supplied intent label
└─────────────────────┘    (greeting, alert, narration, etc.)
    │
    ▼
┌─────────────────────┐
│  Persona Runtime    │  ← maps persona + intent → synthesis params
└─────────────────────┘    (speed, pitch, energy, style vector)
    │
    ▼
┌─────────────────────┐
│  Prosody Planner    │  ← pause insertion, emphasis, boundary marking
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│  Voice Engine API   │  ← abstract interface (swappable backend)
└─────────────────────┘
    │
    ├──────────┬──────────────────────┐
    ▼          ▼                      ▼
┌────────┐ ┌────────┐         ┌─────────────┐
│Engine A│ │Engine B│   ...   │ Engine N    │
│(Piper) │ │(Kokoro)│         │ (Future)    │
└────────┘ └────────┘         └─────────────┘
    │          │
    └────┬─────┘
         ▼
      PCM Audio Frames
         │
         ▼
┌─────────────────────┐
│   Audio Pipeline    │  ← resampling, normalization, silence trimming
└─────────────────────┘
    │
    ├──────────────────────┐
    ▼                      ▼
┌─────────┐         ┌──────────────┐
│ Speaker │         │  WAV / File  │
│ Playback│         │  Stream Out  │
└─────────┘         └──────────────┘
```

---

## VoiceEngine Interface

All TTS backends must implement this TypeScript interface (mirrored in Rust via trait). The application layer never depends on a concrete engine.

```typescript
/**
 * Abstract contract for all TTS backend engines.
 * Applications and the Persona Runtime interact exclusively through this interface.
 * Concrete implementations: PiperEngine, KokoroEngine, MockEngine.
 */
export interface VoiceEngine {
  /**
   * Initialize the engine with the given voice model bundle path.
   * Must complete before any synthesis call.
   * Idempotent: calling again with same voice is a no-op.
   */
  initialize(voicePath: string, options?: EngineInitOptions): Promise<void>;

  /**
   * Load a specific voice from an already-initialized engine.
   * Allows hot-swapping voices without re-initializing the engine.
   */
  loadVoice(voiceId: string): Promise<void>;

  /**
   * Synthesize text fully to a PCM buffer.
   * Returns only when synthesis is complete.
   * Use stream() for large texts or real-time applications.
   */
  synthesize(request: SynthesisRequest): Promise<SynthesisResult>;

  /**
   * Synthesize text as a stream of PCM chunks.
   * Yields chunks as they are produced by the engine.
   * First chunk must arrive within firstChunkLatencyMs (engine capability).
   */
  stream(request: SynthesisRequest): AsyncIterable<PcmChunk>;

  /**
   * Cancel an in-progress synthesis by requestId.
   * Must be safe to call even if synthesis has already completed.
   * Must resolve within 100 ms.
   */
  cancel(requestId: string): Promise<void>;

  /**
   * Returns current engine health status and diagnostic info.
   * Used by the Local Service health endpoint and Voice Lab UI.
   */
  health(): Promise<EngineHealth>;

  /**
   * Returns the static capabilities of this engine.
   * Capabilities do not change after initialization.
   */
  capabilities(): EngineCapabilities;

  /**
   * Tear down all resources held by this engine.
   * After dispose(), initialize() must be called again before use.
   */
  dispose(): Promise<void>;
}

export interface EngineCapabilities {
  /** Engine identifier, e.g. "piper-v1", "kokoro-v0.19", "mock" */
  engineId: string;

  /** Semantic version of the engine adapter */
  adapterVersion: string;

  /** Supported language codes, e.g. ["en-IN", "hi-IN"] */
  supportedLanguages: string[];

  /** Whether streaming synthesis is supported */
  supportsStreaming: boolean;

  /** Whether style/emotion vectors are supported */
  supportsStyleControl: boolean;

  /** Whether speaker embeddings can be swapped at runtime */
  supportsSpeakerEmbeddings: boolean;

  /** Maximum text length (characters) per synthesis request */
  maxTextLength: number;

  /** Expected latency to first audio chunk, milliseconds */
  firstChunkLatencyMs: number;

  /** Expected real-time factor on reference hardware (< 1.0 = faster than real-time) */
  realtimeFactor: number;

  /** Supported sample rates in Hz */
  supportedSampleRates: number[];

  /** Native output sample rate */
  nativeSampleRate: number;
}

export interface EngineInitOptions {
  /** Optional: override model cache directory */
  modelCacheDir?: string;

  /** Number of threads to use for inference (default: auto) */
  numThreads?: number;

  /** Enable verbose engine logging */
  verbose?: boolean;
}

export interface EngineHealth {
  status: 'healthy' | 'degraded' | 'unavailable';
  engineId: string;
  voiceLoaded: boolean;
  activeRequests: number;
  lastSynthesisMs?: number;
  diagnostics?: Record<string, unknown>;
}
```

---

## SynthesisRequest Contract

```typescript
/**
 * Full specification for a single synthesis request.
 * Passed through the entire pipeline from Vocal Client API to Voice Engine.
 */
export interface SynthesisRequest {
  /** Unique identifier for this request. Generated locally, never from user identity. */
  requestId: string;

  /** The text to synthesize. Must not be empty. Max length enforced by engine capabilities. */
  text: string;

  /**
   * Language and locale for synthesis.
   * BCP-47 format. Examples: "en-IN", "hi-IN", "en-US".
   * If omitted, Persona default language is used.
   */
  language?: string;

  /**
   * Persona to use for this synthesis.
   * Must be a loaded persona ID: "tara" | "kashi" | "bobo" | custom.
   * If omitted, the runtime default persona is used.
   */
  personaId?: string;

  /**
   * Application-level intent label.
   * Used by Persona Runtime to select the appropriate style profile.
   * Examples: "greeting", "alert", "narration", "error", "confirmation", "question"
   */
  intent?: string;

  /**
   * Override speaking rate. 1.0 = natural speed.
   * Range: 0.5 - 2.0. Persona defaults apply if omitted.
   */
  speakingRate?: number;

  /**
   * Override pitch. 1.0 = natural pitch.
   * Range: 0.5 - 2.0. Persona defaults apply if omitted.
   */
  pitch?: number;

  /**
   * Override output volume (energy scale). 1.0 = natural.
   * Range: 0.1 - 2.0.
   */
  volume?: number;

  /**
   * Desired output sample rate in Hz.
   * Engine will resample if necessary. Default: 22050.
   */
  sampleRate?: number;

  /**
   * Output audio encoding format.
   * Default: "pcm-f32" (raw float32 PCM, interleaved).
   */
  outputFormat?: 'pcm-f32' | 'pcm-s16' | 'wav' | 'ogg';

  /**
   * Enable SSML processing for this request.
   * If true, `text` field is parsed as SSML.
   * Default: false.
   */
  ssml?: boolean;

  /**
   * Arbitrary metadata for debugging / tracing.
   * Never logged in production. Never transmitted.
   */
  metadata?: Record<string, string>;
}

export interface SynthesisResult {
  requestId: string;
  /** PCM audio data as Float32Array */
  audio: Float32Array;
  sampleRate: number;
  durationMs: number;
  /** Number of characters synthesized */
  characterCount: number;
  /** Wall-clock time for synthesis, milliseconds */
  synthesisTimeMs: number;
  /** Engine that produced this result */
  engineId: string;
}

export interface PcmChunk {
  requestId: string;
  /** Sequential chunk index starting at 0 */
  chunkIndex: number;
  audio: Float32Array;
  sampleRate: number;
  /** True if this is the final chunk */
  isFinal: boolean;
}
```

---

## Persona Runtime Flow

The Persona Runtime translates **who is speaking** and **why they are speaking** into concrete synthesis parameters. It is the only component that knows about persona identity.

```
         ┌──────────────┐     ┌──────────────┐
         │ Persona      │     │ Voice Intent │
         │ Config JSON  │     │ (from app)   │
         └──────┬───────┘     └──────┬───────┘
                │                    │
                └─────────┬──────────┘
                          ▼
               ┌──────────────────────┐
               │   Persona Runtime    │
               │                      │
               │  1. Load persona cfg │
               │  2. Lookup intent    │
               │     profile          │
               │  3. Apply overrides  │
               │  4. Return params    │
               └──────────┬───────────┘
                          │
                          ▼
               ┌──────────────────────┐
               │  SynthesisParams     │
               │  {speakingRate,      │
               │   pitch, energy,     │
               │   styleVector,       │
               │   speakerId}         │
               └──────────────────────┘
```

**Example Persona Config:**

```json
{
  "personaId": "tara",
  "version": "1.0.0",
  "displayName": "Tara",
  "language": "en-IN",
  "voiceId": "tara-en-in-v1",
  "character": "Warm, clear, professional Indian English speaker",

  "defaults": {
    "speakingRate": 1.0,
    "pitch": 1.05,
    "energy": 1.0,
    "styleVector": [0.6, 0.3, 0.1]
  },

  "intentProfiles": {
    "greeting": {
      "speakingRate": 0.95,
      "pitch": 1.10,
      "energy": 1.15,
      "styleVector": [0.8, 0.1, 0.1],
      "description": "Warm, slightly elevated energy for openings"
    },
    "alert": {
      "speakingRate": 1.10,
      "pitch": 0.95,
      "energy": 1.30,
      "styleVector": [0.2, 0.7, 0.1],
      "description": "Clear, firm delivery for urgent information"
    },
    "narration": {
      "speakingRate": 0.90,
      "pitch": 1.00,
      "energy": 0.90,
      "styleVector": [0.5, 0.2, 0.3],
      "description": "Relaxed, even pacing for long-form content"
    },
    "error": {
      "speakingRate": 0.92,
      "pitch": 0.93,
      "energy": 0.95,
      "styleVector": [0.3, 0.5, 0.2],
      "description": "Measured, non-alarming error communication"
    },
    "confirmation": {
      "speakingRate": 1.02,
      "pitch": 1.08,
      "energy": 1.05,
      "styleVector": [0.7, 0.2, 0.1],
      "description": "Bright, affirmative tone"
    },
    "question": {
      "speakingRate": 0.98,
      "pitch": 1.12,
      "energy": 1.00,
      "styleVector": [0.6, 0.1, 0.3],
      "description": "Rising intonation, curious"
    }
  }
}
```

---

## Repository Structure

```
chiti-vocal-runtime/
│
├── README.md                        # Project entry point and quickstart
├── ARCHITECTURE.md                  # Pointer to docs/architecture/
├── Cargo.toml                       # Rust workspace root
├── package.json                     # npm workspace root (web packages)
│
├── docs/
│   ├── architecture/                # THIS DIRECTORY
│   │   ├── SYSTEM_OVERVIEW.md       # This document
│   │   ├── INVARIANTS.md            # Non-negotiable constraints
│   │   ├── ADR-001-*.md             # Architecture Decision Records
│   │   ├── STATE_MACHINE.md         # Audio lifecycle state machine
│   │   ├── SECURITY.md              # Threat model and controls
│   │   └── PRIVACY.md               # Privacy architecture
│   ├── api/                         # API reference (generated + hand-written)
│   └── guides/                      # Developer how-to guides
│
├── crates/
│   ├── chiti-vocal-core/            # Core Rust library
│   │   ├── src/
│   │   │   ├── lib.rs               # Public API entry point
│   │   │   ├── pipeline/            # Synthesis pipeline stages
│   │   │   │   ├── normalizer.rs    # Input normalization
│   │   │   │   ├── router.rs        # Language routing
│   │   │   │   ├── g2p.rs           # Grapheme-to-phoneme
│   │   │   │   ├── prosody.rs       # Prosody planning
│   │   │   │   └── audio.rs         # Audio pipeline (resample, normalize)
│   │   │   ├── engine/              # VoiceEngine trait + adapters
│   │   │   │   ├── mod.rs           # VoiceEngine trait definition
│   │   │   │   ├── piper.rs         # Piper TTS adapter
│   │   │   │   ├── kokoro.rs        # Kokoro adapter (Phase 3)
│   │   │   │   └── mock.rs          # Mock engine for tests/CI
│   │   │   ├── persona/             # Persona Runtime
│   │   │   │   ├── mod.rs
│   │   │   │   ├── config.rs        # Persona config deserialization
│   │   │   │   └── resolver.rs      # Intent → params resolution
│   │   │   ├── pack/                # Voice Pack (.cvpack) handling
│   │   │   │   ├── manifest.rs      # Manifest schema + validation
│   │   │   │   ├── loader.rs        # Pack loading and extraction
│   │   │   │   └── verify.rs        # Checksum and security verification
│   │   │   └── state/               # State machine
│   │   │       └── machine.rs
│   │   └── tests/
│   │       ├── offline_test.rs      # VOICE_INV_001 enforcement
│   │       ├── determinism_test.rs  # VOICE_INV_005 enforcement
│   │       └── invariants_test.rs   # All invariant test suite
│   │
│   ├── chiti-vocal-service/         # Local HTTP/WS daemon (Axum)
│   │   └── src/
│   │       ├── main.rs
│   │       ├── server.rs            # Axum router setup
│   │       ├── handlers/            # REST + WS handlers
│   │       ├── security/            # Origin validation, rate limiting
│   │       └── queue.rs             # Request queue management
│   │
│   └── chiti-vocal-cli/             # Command-line tool
│       └── src/
│           └── main.rs
│
├── packages/
│   ├── chiti-voice-sdk/             # TypeScript Web SDK (npm)
│   │   ├── src/
│   │   │   ├── index.ts             # Public exports
│   │   │   ├── client.ts            # VocalClient class
│   │   │   ├── modes/
│   │   │   │   ├── local-service.ts # LocalServiceMode connector
│   │   │   │   └── browser-native.ts# WASM/ONNX browser mode
│   │   │   ├── types.ts             # Shared TypeScript types
│   │   │   └── worker/
│   │   │       └── synthesis.worker.ts # Web Worker for browser-native mode
│   │   └── package.json
│   │
│   └── chiti-voice-lab/             # Developer workbench (React)
│       ├── src/
│       │   ├── App.tsx
│       │   ├── pages/
│       │   │   ├── VoiceTester.tsx  # Synthesize and play text
│       │   │   ├── PersonaEditor.tsx # Edit persona config
│       │   │   ├── Benchmark.tsx    # Engine benchmarking
│       │   │   └── PackBuilder.tsx  # Voice Pack authoring
│       │   └── components/
│       └── package.json
│
├── models/                          # Voice model assets (gitignored except stubs)
│   ├── tara/
│   │   ├── tara-en-in-v1.onnx
│   │   └── persona.json
│   ├── kashi/
│   └── bobo/
│
├── packs/                           # Built .cvpack bundles (CI output)
│
├── scripts/                         # Dev + CI scripts
│   ├── build-pack.py                # Assemble .cvpack from model + persona
│   ├── verify-pack.sh               # Validate pack checksums
│   └── benchmark.py                 # Evaluation pipeline
│
└── .github/
    └── workflows/
        ├── ci.yml                   # Build, test, invariant checks
        └── pack-validation.yml      # Voice pack security scan
```

---

## Technology Stack

| Component | Language | Rationale |
|-----------|----------|-----------|
| **Vocal Core** (synthesis engine, pipeline, persona runtime, state machine) | **Rust** | Memory safety without GC, deterministic performance, C FFI for native integration, compiles to WASM for browser, suitable for embedded/IoT targets |
| **Local Service daemon** | **Rust** (Axum) | Same binary as Core, minimal footprint, async I/O, no runtime overhead |
| **Web SDK** | **TypeScript** | Ergonomic for web developers, strong typing, tree-shakeable, dual mode (local service + browser-native) |
| **Voice Lab UI** | **TypeScript** (React) | Rapid UI iteration, component reuse with Web SDK, Electron packaging for desktop |
| **Model evaluation, benchmark scripts, research** | **Python** | Scientific ecosystem (numpy, librosa, resemblyzer), fast prototyping, MOS evaluation |
| **Voice models (inference)** | **ONNX** | Backend-agnostic inference format, supported by ONNX Runtime (native + WASM), enables model swapping without code changes |
| **Voice Pack format** | **ZIP + JSON manifest** | Universal tooling, easy inspection, manifest-first validation before extraction |

---

## Runtime Tiers

The runtime supports three hardware tiers. The tier determines which model variant is loaded, not which VoiceEngine adapter is used.

| Tier | Goal | Hardware Target | Model Size | Quality | Use Case |
|------|------|----------------|------------|---------|----------|
| **VOCAL NANO** | Runs on anything | Raspberry Pi 4, budget Android, IoT edge | < 30 MB | Intelligible, natural | Embedded kiosks, robots, devices with no GPU |
| **VOCAL LITE** | Everyday laptop | Mid-range laptop (no GPU), modern Android/iOS | 30–100 MB | High quality, expressive | Desktop apps, mobile apps, developer machines |
| **VOCAL STUDIO** | Maximum quality | High-end laptop, workstation, GPU optional | 100–350 MB | Near-human quality, full style control | Voice Lab, professional content, production audio |

The tier is selected automatically via hardware profiling at startup, or can be overridden explicitly.

---

## Three Personas Summary

| Persona | Purpose | Language | Character | Key Differentiator |
|---------|---------|----------|-----------|-------------------|
| **TARA** | Primary assistant voice for productivity and information delivery | Indian English (`en-IN`) | Warm, clear, composed, professional. Confident without being cold. | Clarity-first Indian English prosody; optimized for longer narration and UI guidance |
| **KASHI** | Hindi-first voice for vernacular applications | Hindi (`hi-IN`) + code-switching | Rich, earthy, culturally grounded. Conversational and unhurried. | Natural Hindi prosody with graceful English word insertion; regional authenticity |
| **BOBO** | Child-facing, playful voice for education and entertainment | Indian English + Hindi | Bright, bubbly, enthusiastic. Age-appropriate energy. | Elevated pitch range, exaggerated prosody contours, child-safe character constraints |

---

## Audio Pipeline States

```
                          ┌──────────────┐
                          │ UNINITIALIZED│
                          └──────┬───────┘
                                 │ initialize()
                                 ▼
                          ┌──────────────┐
                          │ INITIALIZING │◄── model loading in progress
                          └──────┬───────┘
                     success │        │ error
                             ▼        ▼
                          ┌──────┐  ┌───────┐
                          │READY │  │ ERROR │
                          └──┬───┘  └───┬───┘
          ┌──────────────────┘          │ recovery attempt
          │ speak()                     ▼
          ▼                        ┌──────────┐
    ┌─────────────┐                │RECOVERING│
    │ SYNTHESIZING│◄───────────────┴──────────┘
    └──────┬──────┘
           │ synthesis complete
           ▼
    ┌─────────────┐
    │  BUFFERING  │ ← audio chunks arriving, pre-buffer filling
    └──────┬──────┘
           │ buffer threshold met
           ▼
    ┌─────────────┐
    │   PLAYING   │◄──────────────────┐
    └──────┬──────┘                   │ resume()
           │            ┌─────────────┴──┐
           │            │     PAUSED     │
           │            └────────────────┘
           │ pause()           ▲
           └───────────────────┘
           │ playback complete
           ▼
        ┌──────┐
        │READY │ ← returns to ready for next speak()
        └──────┘

  -- Cancellation path (from SYNTHESIZING, BUFFERING, or PLAYING) --

  [SYNTHESIZING / BUFFERING / PLAYING]
           │ cancel() / barge-in
           ▼
    ┌─────────────┐
    │ CANCELLING  │ ← flushes queue, stops engine, clears buffers
    └──────┬──────┘
           │ confirmed
           ▼
        ┌──────┐
        │READY │
        └──────┘

  -- Error recovery path --

  [ANY STATE]
      │ unrecoverable error
      ▼
  ┌───────┐
  │ ERROR │
  └───┬───┘
      │ retryable? → RECOVERING → READY
      │ not retryable?
      ▼
  ┌─────────────┐
  │ UNAVAILABLE │ ← engine cannot be used; dispose() and reinitialize required
  └─────────────┘
```

---

## Web Execution Modes

### Mode 1: Local Service Mode

The application communicates with a locally-installed Chiti Vocal Service daemon over loopback HTTP/WebSocket.

```
┌──────────────────────────────────────────────────────────┐
│  BROWSER TAB                                             │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │  Application Code                                  │  │
│  │                                                    │  │
│  │   const vocal = new VocalClient({ mode: 'local'}) │  │
│  │   await vocal.speak("Hello")                      │  │
│  └───────────────────────┬────────────────────────────┘  │
│                          │ HTTP POST / WebSocket          │
└──────────────────────────┼───────────────────────────────┘
                           │ localhost:45231 (loopback only)
┌──────────────────────────┼───────────────────────────────┐
│  CHITI VOCAL LOCAL SERVICE (Native Daemon)               │
│                          │                               │
│  ┌───────────────────────▼──────────────────────────┐   │
│  │  Axum HTTP Server                                 │   │
│  │  * Origin validation                              │   │
│  │  * Rate limiting                                  │   │
│  │  * Request queue                                  │   │
│  └───────────────────────┬───────────────────────────┘   │
│                          │                               │
│  ┌───────────────────────▼───────────────────────────┐   │
│  │  Chiti Vocal Core (Rust native)                   │   │
│  │  * Full synthesis pipeline                        │   │
│  │  * Persona Runtime                                │   │
│  │  * Piper / Kokoro engine                          │   │
│  └───────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────┘
```

### Mode 2: Browser-Native Mode

The Vocal Core runs entirely inside the browser tab. No daemon required.

```
┌──────────────────────────────────────────────────────────────┐
│  BROWSER TAB                                                 │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Application Code                                    │   │
│  │   const vocal = new VocalClient({ mode: 'browser' }) │   │
│  └──────────────────────────┬─────────────────────────── ┘  │
│                             │ postMessage                    │
│  ┌──────────────────────────▼───────────────────────────┐   │
│  │  Web Worker (synthesis.worker.ts)                    │   │
│  │                                                      │   │
│  │  ┌──────────────────────────────────────────────┐   │   │
│  │  │  ONNX Runtime Web                            │   │   │
│  │  │  * WASM backend (CPU)                        │   │   │
│  │  │  * WebGPU backend (GPU, if available)        │   │   │
│  │  └──────────────────────────┬───────────────────┘   │   │
│  │                             │ PCM Float32Array       │   │
│  └──────────────────────────── ┼───────────────────────┘   │
│                                │ postMessage (chunks)        │
│  ┌─────────────────────────────▼───────────────────────┐    │
│  │  AudioWorklet (audio pipeline + playback)           │    │
│  │  * Real-time scheduling                             │    │
│  │  * No main-thread blocking                          │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                              │
│  [!] Model fetched from localhost / bundled asset (no cloud) │
└──────────────────────────────────────────────────────────────┘
```

---

## Hardware Profiling

At startup the runtime profiles the current device and returns a `HardwareProfile` to guide model tier selection.

```typescript
export interface HardwareProfile {
  /**
   * Recommended runtime tier based on device capabilities.
   * Applications should use this unless they have specific requirements.
   */
  profile: 'nano' | 'lite' | 'studio';

  /**
   * Recommended engine adapter for this hardware.
   * e.g. "piper-nano", "piper-lite", "kokoro-studio"
   */
  recommendedEngine: string;

  /**
   * WebGPU availability (browser-native mode only).
   * undefined in native context.
   */
  webgpu?: {
    available: boolean;
    adapterName?: string;
    estimatedGflops?: number;
  };

  /**
   * WASM SIMD support (browser-native mode only).
   * Critical for inference performance.
   */
  simd?: boolean;

  /**
   * WASM multi-threading support.
   */
  wasmThreads?: boolean;

  /** Number of logical CPU cores */
  cpuCores: number;

  /**
   * Estimated available RAM in MB.
   * May be approximate on some platforms.
   */
  estimatedRamMb: number;

  /** Platform identifier */
  platform: 'linux' | 'windows' | 'macos' | 'android' | 'ios' | 'browser';

  /**
   * Architecture identifier.
   * Relevant for native binary selection.
   */
  arch: 'x86_64' | 'aarch64' | 'wasm32' | 'armv7';
}
```

**Profiling Logic (simplified):**

```
cpuCores <= 2 OR estimatedRamMb < 512   →  nano
cpuCores >= 4 AND estimatedRamMb >= 2048 →  studio
otherwise                                →  lite
```

---

## Long-term Architecture (North Star)

The eventual target is a unified Chiti Vocal Runtime Core serving all application surfaces through a clean Persona API, agnostic to both text source and delivery target.

```
  ┌─────────────────────────────────────────────────────────────────┐
  │                TEXT SOURCES (orthogonal to runtime)             │
  │                                                                 │
  │   ┌──────────────┐  ┌───────────────┐  ┌───────────────────┐  │
  │   │  Rules-based │  │  Local LLM    │  │   Cloud LLM       │  │
  │   │  templates   │  │  (Ollama etc.)│  │   (opt-in only)   │  │
  │   └──────┬───────┘  └───────┬───────┘  └────────┬──────────┘  │
  └──────────┼──────────────────┼───────────────────┼─────────────┘
             │                  │                   │
             └──────────────────┴───────────────────┘
                                │  text (any source)
                                ▼
  ┌─────────────────────────────────────────────────────────────────┐
  │               CHITI VOCAL RUNTIME CORE                         │
  │                                                                 │
  │     ┌─────────┐   ┌─────────┐   ┌─────────┐                   │
  │     │  TARA   │   │  KASHI  │   │  BOBO   │  (+ custom)       │
  │     │ en-IN   │   │  hi-IN  │   │  en-IN  │                   │
  │     └────┬────┘   └────┬────┘   └────┬────┘                   │
  │          └─────────────┴─────────────┘                         │
  │                        │                                        │
  │              ┌──────────▼───────────┐                          │
  │              │    Persona API       │                          │
  │              │  (stable, versioned) │                          │
  │              └──────────┬───────────┘                          │
  └─────────────────────────┼───────────────────────────────────────┘
                            │  audio stream
     ┌──────────────────────┼──────────────────────────────┐
     │                      │                              │
     ▼                      ▼                    ▼         ▼
┌─────────┐          ┌──────────┐         ┌──────────┐ ┌──────────┐
│  Web    │          │  Mobile  │         │ Desktop  │ │ IoT /    │
│ (SDK)   │          │(iOS/And.)│         │(Electron)│ │ Robot    │
└─────────┘          └──────────┘         └──────────┘ └──────────┘
```

The Persona API is the stable external surface. Everything below it (engine, model, hardware tier) is an implementation detail that can evolve without breaking applications.

---

*Document maintained by the Chiti Platform Team. For invariants and constraints, see [INVARIANTS.md](./INVARIANTS.md). For backend selection rationale, see [ADR-001-initial-tts-backend.md](./ADR-001-initial-tts-backend.md).*
