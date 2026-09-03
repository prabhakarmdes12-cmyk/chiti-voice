# Chiti Vocal Runtime

> **The voice infrastructure layer for software, agents, robots, and the web.**

[![Rust](https://img.shields.io/badge/Rust-1.88+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![ONNX Runtime](https://img.shields.io/badge/ONNX_Runtime-1.18+-005CED?style=flat-square&logo=onnx&logoColor=white)](https://onnxruntime.ai/)
[![License](https://img.shields.io/badge/License-Proprietary-red?style=flat-square)](./LICENSE)

---

## ⚠️ Read this before you plan anything around this repo

**Current state: architecture and packaging are implemented. Speech synthesis is not.**

There is **No audible voice** in this repository. Concretely:

| Question | Answer today |
|---|---|
| Can it speak? | **No.** `MockEngine` emits digital silence; `PiperEngine` returns `ENGINE_NOT_AVAILABLE`. `vocal_core::REAL_SYNTHESIS_AVAILABLE == false`. |
| Is there anything to listen to? | Two kinds, and only one of them is product-direction audio. `assets/offline-spike/` holds nine clips synthesised **for real, offline**: four stock-voice utterances from `scripts/spike-kokoro-offline.py` against a genuine int8 Kokoro graph (measured in [`docs/research/KOKORO_OFFLINE_SPIKE.md`](./docs/research/KOKORO_OFFLINE_SPIKE.md)), plus five persona casts from `scripts/derive-persona-style.py` (measured in [`docs/research/PERSONA_STYLE_VECTORS.md`](./docs/research/PERSONA_STYLE_VECTORS.md), recipes in `docs/research/persona-recipes/`). This is the first audio this repo has ever produced that a person can judge. `assets/persona-auditions/` keeps the ~23 s per-persona reference clips for ROADMAP path B/B′/C (§5.1). **Nothing in `crates/` can speak yet**: `REAL_SYNTHESIS_AVAILABLE` is `false`, and the spike's contract is now pinned in `crates/vocal-core/tests/fixtures/kokoro/` so the Rust engine has something to be graded against. |
| Is there a voice model? | **No.** `voice-packs/*/model.onnx` never existed; `dist/*.cvpack` contained a 36-byte placeholder and is labelled `status: "placeholder"`. |
| Is there a CLI that works? | Partially: `list`, `status`, `verify`, `install` really work (they load and validate packs). `speak` runs the whole pipeline and writes a **silent** WAV. |
| Is there an HTTP daemon or TypeScript SDK? | **No.** Both are specs in `docs/api/`, nothing more. |
| Do the CI gates mean anything? | The ones that decide whether this code is usable, yes: `Unit Tests`, `Linting (clippy)`, `Dependency Audit`, `System Invariants` and `Offline Synthesis Test (Quality Gate)` all pass and fail on their merits. Two do not: the live job's `VOICE_INV_001 validated` line is an `echo` (the real `unshare -rn` isolation is staged in `ops/ci/` and still needs `scripts/install-ci.sh`), and `Format Check` is red because `cargo fmt --all` has never been run on this tree. |
| Does it compile? | **Verified, yes** — `cargo build --workspace` is green on ubuntu, macOS and Windows, on both stable and nightly (run `33692329068`, 11 of 12 jobs). It previously did not: `Cargo.toml` declared an `examples/simple_speak.rs` that did not exist, which failed manifest parsing for the *whole workspace*, and CI had been red since the workflow landed. |

Earlier revisions of this README, `PHASE1_COMPLETE.md` and `AGENTS.md` described Phase 1
as complete, all exit criteria passed, three voices loading, and a TypeScript SDK
established. **Those claims were false.** CI on `main` was failing, all three shipped
`.cvpack` files failed their own checksums, and the offline test asserted nothing. The
history is preserved in `PHASE1_COMPLETE.md` with a correction note; the rules that keep
it from happening again are in the `docs-truth` CI job.

---

## What this project is

An **offline-first, LLM-independent voice infrastructure layer**. It gives an
application, agent, robot or website a persistent, portable, licensable vocal identity —
without a cloud API and without an LLM in the synthesis path.

A `.cvpack` voice file is to speech what a `.ttf` font file is to text: installable,
portable, composable, licensable.

### Architectural invariants

> **`LANGUAGE GENERATION != VOICE GENERATION`** — this is a voice *renderer*. It decides
> how to say things, never what to say. LLM integration is optional, external, upstream.

> **Offline-first** — synthesis must work with the network cable unplugged.

Two mechanisms keep these honest — one live, one pending adoption:

- **Live now:** `vocal_core::REAL_SYNTHESIS_AVAILABLE == false` plus tests that assert the
  real backend refuses to synthesize, so "it speaks" cannot be quietly asserted.
- **After you run `scripts/install-ci.sh`:** CI executes the suite inside a network-less
  namespace (`sudo unshare -rn`) and fails if isolation did not apply, verifies every
  shipped `.cvpack` against its own manifest, audits the resolved dependency graph for
  network/LLM clients, checks that provenance fields are not fabricated, and greps the
  docs for capability claims the code cannot support.

> **Why `install-ci.sh`:** the corrected workflow is staged in [`ops/ci/ci-phase1.yml`](./ops/ci/ci-phase1.yml)
> rather than installed, because pushing to `.github/workflows/` needs the `workflows`
> permission. **Until you run it, the live workflow's "quality gates" are the old ones that
> cannot fail** — see [`ops/ci/README.md`](./ops/ci/README.md). Do not treat its green/red
> output as a capability claim in either direction.

Full list: [`docs/architecture/INVARIANTS.md`](./docs/architecture/INVARIANTS.md).

---

## Build and run

```bash
cargo build --workspace --all-targets        # default features: no ONNX, no network at build time
cargo test --workspace
cargo run -p vocal-core --example simple_speak -- "Hello" /tmp/out.wav

# CLI
cargo run -p chiti-voice-cli --bin chiti-voice -- status
cargo run -p chiti-voice-cli --bin chiti-voice -- list
cargo run -p chiti-voice-cli --bin chiti-voice -- verify voice-packs/dist/tara.cvpack
cargo run -p chiti-voice-cli --bin chiti-voice -- install voice-packs/dist/tara.cvpack --allow-placeholder
cargo run -p chiti-voice-cli --bin chiti-voice -- speak --voice tara "Hello, world!" --allow-silence
```

Voice packs are built and verified with the pack tooling (checksums must always be
computed from the bytes actually going into the archive):

```bash
python3 scripts/build-voice-packs.py build
python3 scripts/build-voice-packs.py build --require-real-models   # release: refuses placeholders
python3 scripts/build-voice-packs.py verify
```

`--limits embedded` / `--limits tiny` enforce the resource budgets a Raspberry Pi or toy
target needs (VOICE_INV_011) while loading packs.

### Enabling real speech

Nothing here produces audio until all three exist:

1. a real ONNX model at `voice-packs/<id>/model.onnx` (with its `MODEL_CARD` license read
   and copied into `provenance`),
2. ONNX inference implemented in `crates/vocal-core/src/engine/piper.rs` behind
   `--features piper` (the `ort` and `ndarray` dependencies are declared and waiting, used
   by no code),
3. `REAL_SYNTHESIS_AVAILABLE` flipped to `true` in the same PR, with a test that decodes a
   real pack and asserts non-zero PCM.

`docs/ROADMAP_EMBEDDED.md` is the current plan for getting there.

---

## Repository layout (what actually exists)

```
chiti-voice/
├── apps/               # Runnable targets
├── crates/             # Rust libraries
├── voice-packs/        # Pack sources (manifest.json) + built dist/*.cvpack
├── ops/                # Staged CI definitions awaiting permission to install
├── scripts/            # Pack builder/verify, test-fixture generator, install-ci.sh
└── docs/               # Architecture, API specs, personas, research
```

| Path | Contents |
|------|----------|
| `crates/vocal-core` | `VoiceEngine` trait, engine registry, `MockEngine` (silence), `PiperEngine` (unimplemented), personas, state machine, error codes, WAV encoding |
| `crates/voice-pack` | `.cvpack` container: manifest schema, size/rate-limited loader, security validator |
| `apps/chiti-voice-cli` | `speak`, `list`, `verify`, `status`, `install`, `version` |
| `voice-packs/{tara,kashi,bobo}` | Persona manifests (no models) |
| `docs/architecture` | System overview, invariants, state machine, security, privacy, ADR-001 |
| `docs/api` | HTTP + TypeScript SDK **specifications** (not implemented) |
| `docs/research` | Model-size and quality research tracks |

> Planned directories (`packages/voice-web`, `engines/`, `tools/`, `apps/voice-lab`,
> `apps/vocal-local`, `research/`, `tests/`) **do not exist yet** and are deliberately not
> listed above. The previous README documented them as if they did.

---

## Personas

Three persona *specifications* exist. Three voices do not — persona ≠ model.

| Persona | Identity | Primary use |
|---------|----------|-------------|
| **TARA** | Warm, professional, Indian English, female-presenting | Business, hospitality, customer interfaces |
| **KASHI** | Calm, measured, Hindi/Sanskrit-aware, male-presenting | Guidance, knowledge, navigation |
| **BOBO** | High-expressiveness, playful, fictional character | Children's products, toys, robots |

---

## Six logical products (vision vs. status)

| Product | Status |
|---------|--------|
| Chiti Vocal Core | Interface + mock backend implemented; **no real backend** |
| Chiti Voice Pack (`.cvpack`) | Implemented: format, manifest, limits, security validation |
| Chiti Persona Runtime | Data model implemented; **not wired to any engine** |
| Chiti Vocal Local Service | Spec only |
| Chiti Voice Web SDK | Spec only |
| Chiti Voice Lab | Not started |

---

## Documentation

| Document | |
|----------|--|
| [`docs/architecture/SYSTEM_OVERVIEW.md`](./docs/architecture/SYSTEM_OVERVIEW.md) | Architecture walkthrough |
| [`docs/architecture/INVARIANTS.md`](./docs/architecture/INVARIANTS.md) | The 12 system invariants |
| [`docs/architecture/STATE_MACHINE.md`](./docs/architecture/STATE_MACHINE.md) | Engine lifecycle |
| [`docs/architecture/SECURITY.md`](./docs/architecture/SECURITY.md) | Threat model |
| [`docs/architecture/PRIVACY.md`](./docs/architecture/PRIVACY.md) | Privacy and data handling |
| [`docs/architecture/ADR-001-initial-tts-backend.md`](./docs/architecture/ADR-001-initial-tts-backend.md) | Backend selection (**status: PROPOSED — decision not made**) |
| [`docs/voice-pack/SPECIFICATION.md`](./docs/voice-pack/SPECIFICATION.md) | `.cvpack` format |
| [`docs/api/HTTP_API.md`](./docs/api/HTTP_API.md) | Local daemon API (**not implemented**) |
| [`docs/api/TYPESCRIPT_API.md`](./docs/api/TYPESCRIPT_API.md) | SDK API (**not implemented**) |
| [`docs/ROADMAP_EMBEDDED.md`](./docs/ROADMAP_EMBEDDED.md) | **Start here**: plan for real, offline, device-sized voice |
| [`PRD.md`](./PRD.md) | Product requirements |
| [`LICENSE`](./LICENSE) | Proprietary notice + third-party obligations (draft — needs review) |

---

## Roadmap

Phase numbering in `PRD.md` assumed backend-then-service-then-SDK. For an
**embedded, offline-only** goal that ordering buries the risky part, so
[`docs/ROADMAP_EMBEDDED.md`](./docs/ROADMAP_EMBEDDED.md) re-sequences it: one real voice
first, then size/latency on the actual device, then the service/SDK surface.

| Phase | Name | Status |
|-------|------|--------|
| 0 | Foundation, invariants, docs | ✅ done |
| 1 | Interfaces, `.cvpack` format, CLI skeleton, CI | ⚠️ code done except real audio; **gates were false, now repaired** |
| 2 | Real synthesis: ONNX backend, one voice, WAV out | ⛔ not started ← **the actual next step** |
| 3 | Size/latency budget on target hardware | ⛔ not started |
| 4 | Local HTTP/WS daemon (127.0.0.1) | ⛔ not started |
| 5 | `@chiti/voice-web` SDK | ⛔ not started |
| 6+ | Text normalization, streaming, personas wiring, Voice Lab, signing, browser-native | ⛔ not started |

---

## Tech stack

| Layer | Technology | Status |
|-------|-----------|--------|
| Core runtime | Rust (tokio, async-trait) | ✅ |
| Inference | ONNX Runtime via `ort` | declared, optional feature, **unused** |
| Pack format | ZIP + JSON manifest + blobs, SHA-256, size/ratio limits | ✅ |
| CLI | `clap` 4 | ✅ (WAV file output; no device playback) |
| Local daemon | Rust + Axum | spec only |
| SDK | TypeScript | spec only |
| Voice Lab | Tauri/Next.js + React + Tailwind | not started |
| Research | Python (pack tooling) | ✅ partial |

Adding a dependency here has a bar: no network client in the synthesis path, and it must
cross-compile onto the target device. That is why `ort` is optional, why the `dirs` crate
isn't used (the CLI reads `HOME`/`USERPROFILE`), and why `mockito` was removed.

---

## License

Copyright © 2026 Chiti Technologies. All rights reserved. See [`LICENSE`](./LICENSE).

The `LICENSE` file is a **draft awaiting legal review**, and it documents the
third-party obligations that a proprietary notice does not override — notably that
espeak-ng (used by Piper for phonemization) is GPL-3.0 and that Piper's voice models are
licensed per voice, not MIT by association. Resolving those is a prerequisite for shipping,
not a formality.

---

<div align="center">

**Chiti Technologies** · *Chiti Vocal Runtime is a Chiti Technologies product.*

</div>
