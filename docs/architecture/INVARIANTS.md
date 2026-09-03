# Chiti Vocal Runtime — System Invariants

> **Version:** 0.1.0-alpha  
> **Status:** Ratified  
> **Last Updated:** September 2026  
> **Owner:** Chiti Platform Team

---

## Introduction

These invariants are **non-negotiable architectural constraints** of the Chiti Vocal Runtime. They represent commitments made to users, integrators, and the platform itself. They are not guidelines or best practices — they are hard rules.

**Tests must exist for all technically testable invariants.** An untested invariant is an unverified promise.

**Violations require explicit architectural review.** No invariant may be suspended, weakened, or worked around without:
1. An Architecture Decision Record (ADR) documenting the justification
2. A recorded review by at least one other platform engineer
3. A defined sunset date or recovery plan if the violation is temporary

If you are modifying code and a change appears to conflict with an invariant below, **stop and raise it** — do not proceed silently.

---

## Invariants

---

### VOICE_INV_001 — Offline Independence

| Field | Value |
|-------|-------|
| **ID** | VOICE_INV_001 |
| **Name** | Offline Independence |
| **Severity** | CRITICAL |

**Statement:**

The Chiti Vocal Runtime Core MUST complete the full synthesis pipeline — from text input to PCM audio output — with zero outbound network requests. This includes model loading, G2P lookup, persona resolution, audio generation, and audio post-processing. The runtime MUST produce audio when the device has no network access whatsoever.

**Rationale:**

Voice is infrastructure. Applications that depend on voice (accessibility tools, embedded kiosks, in-car systems, classroom software) cannot tolerate a network dependency in the speech path. A cloud TTS failure should never silence a device. Beyond reliability: offline operation is the primary privacy guarantee — data that never leaves the device cannot be intercepted or retained.

**Test Approach:**

- `tests/offline_test.rs`: Spin up a network namespace with all external routes blocked (or use a mock network interceptor on Windows CI). Initialize the runtime. Synthesize a test sentence. Assert audio is produced. Assert no network socket was opened during synthesis (verified via `strace`/`dtrace` in CI, or mock network layer that fails on any outbound call).
- The test must cover: initialization, model loading, and synthesis all while network-blocked.
- Test must run in CI on every pull request.

**Enforcement Mechanism:**

- `vocal-core` has zero HTTP client dependencies in its `Cargo.toml`. The CI job `Dependency Audit - No Cloud/LLM Dependencies` greps `Cargo.toml` and `Cargo.lock` for cloud SDK names and `crates/vocal-core/Cargo.toml` for LLM crates, and fails the build. Its limit is worth stating rather than left to memory: it reads manifests, so a cloud client pulled in transitively is not caught until `cargo tree` is audited where crates.io is reachable.
- The offline integration test in CI uses a network-isolated environment.
- Code review checklist includes explicit: "Does this change add any network call in the synthesis path?"

---

### VOICE_INV_002 — LLM Independence

| Field | Value |
|-------|-------|
| **ID** | VOICE_INV_002 |
| **Name** | LLM Independence |
| **Severity** | CRITICAL |

**Statement:**

The Chiti Vocal Runtime MUST NOT require, call, or depend on any Large Language Model for synthesis. The runtime accepts text and produces audio. It does not generate, transform, or augment text using a neural language model. Applications may use an LLM to produce the text; the runtime does not care and does not need to know.

**Rationale:**

`LANGUAGE GENERATION != VOICE GENERATION`. This is the foundational design principle of the platform. Coupling voice synthesis to LLM availability would inherit all of LLM's problems: latency, cost, hallucination risk, privacy concerns, and cloud dependency. The runtime must remain functional regardless of whether an LLM exists anywhere in the stack.

**Test Approach:**

- Static analysis: `grep -r "llm\|openai\|anthropic\|ollama\|gemini\|gpt\|transformers" crates/vocal-core/src/` must produce zero results. This grep is a review step, not a CI step: the job above audits dependency manifests only.
- Dependency audit: `cargo tree` output for `vocal-core` must not include any LLM inference library.
- Behavioral test: Synthesize a sentence using only the runtime with no LLM service running. Assert success.

**Enforcement Mechanism:**

- The CI job `Dependency Audit - No Cloud/LLM Dependencies` fails the build when a prohibited name appears in a manifest. and there is no such script: an earlier draft of this document named a script that was never written, and a check that does not run is worse than no check, because it manufactures confidence. Auditing the transitive tree is still open, for the same reason `Cargo.lock` is absent -- it needs crates.io access.
- Architecture review required for any text transformation added to the synthesis pipeline.

---

### VOICE_INV_003 — Provider Independence

| Field | Value |
|-------|-------|
| **ID** | VOICE_INV_003 |
| **Name** | Provider Independence |
| **Severity** | CRITICAL |

**Statement:**

No application code that uses the Chiti Voice SDK or Persona API shall ever be aware of which TTS engine backend is running. The `VoiceEngine` interface is the only surface applications interact with. Concrete engine names (Piper, Kokoro, etc.) MUST NOT appear in the public API, error messages returned to callers, or audio metadata exposed to applications.

**Rationale:**

Provider independence enables the runtime to upgrade, replace, or run multiple backends simultaneously without any application changes. If applications become coupled to Piper-specific behavior, migrating to Kokoro (or any future engine) becomes a breaking change. The abstraction must be total.

**Test Approach:**

- Type-surface check: the public exports of `chiti-voice-sdk` must not include "piper", "kokoro" or any concrete engine name. Enforced by review only: the SDK is a specification in `docs/api/TYPESCRIPT_API.md` and no package exists to parse, so no CI script implements this.
- Runtime test: Register both `MockEngineA` and `MockEngineB` behind the VoiceEngine interface. Synthesize with both. Assert that the `SynthesisResult` received by the application contains only `engineId` (an opaque string) — no engine-specific metadata leaks.
- Integration test: Swap the active engine between two registered implementations at runtime. Assert the application receives audio from both without any code change on the application side.

**Enforcement Mechanism:**

- `VoiceEngine` is defined in `crates/vocal-core/src/engine/mod.rs`, and concrete engine types live only under `crates/vocal-core/src/engine/` (`mock.rs`, `piper.rs`). There is no `packages/` tree, so the TypeScript mirror of the interface is a planned file, not a present one.
- API surface audit in CI.

---

### VOICE_INV_004 — Persona Independence

| Field | Value |
|-------|-------|
| **ID** | VOICE_INV_004 |
| **Name** | Persona Independence |
| **Severity** | HIGH |

**Statement:**

Persona definitions (Tara, Kashi, Bobo, or any custom persona) MUST be fully described by their `persona.json` configuration and MUST NOT be hardcoded in engine or pipeline source code. Adding, modifying, or removing a persona MUST require only a configuration change — not a code change.

**Rationale:**

Personas will evolve. New regional personas will be added. Existing personas will be tuned based on user feedback and evaluation results. If persona parameters are hardcoded in Rust, every persona update requires a recompile and release. Configuration-driven personas enable rapid iteration, A/B testing of persona parameters, and eventual user-configurable voices.

**Test Approach:**

- `tests/persona_independence_test.rs`: Create a persona config JSON at runtime with arbitrary parameters. Load it via `PersonaRuntime::load_from_config()`. Synthesize with it. Assert the synthesis parameters passed to the engine match the config values exactly — no hardcoded defaults override the config.
- Test that removing the built-in persona JSON files causes the built-in personas to fail to load (not silently fall back to hardcoded values).

**Enforcement Mechanism:**

- `PersonaRuntime::load_from_config()` is the only code path that produces `SynthesisParams`. There is no `PersonaRuntime::tara_defaults()` function.
- Code review: Any PR modifying `crates/vocal-core/src/persona.rs` or `crates/voice-pack/src/manifest.rs` must be reviewed for hardcoded parameter introduction. There is no separate persona resolver module: the persona is built in `Persona::from_pack` and validated in `validate_persona`.

---

### VOICE_INV_005 — Deterministic Core

| Field | Value |
|-------|-------|
| **ID** | VOICE_INV_005 |
| **Name** | Deterministic Core |
| **Severity** | HIGH |

**Statement:**

Given the same text, the same persona config, the same voice model, and the same synthesis parameters, the Chiti Vocal Core MUST produce bit-identical PCM output on the same platform. The synthesis pipeline is deterministic: no random sampling, no stochastic decoding, no non-reproducible operations.

**Rationale:**

Determinism enables reliable testing, golden-file regression tests, quality benchmarking, and debugging. Non-determinism makes it impossible to distinguish regressions from natural variance. It also makes QA evaluation reproducible: a human evaluator can listen to the same output multiple times and get the same audio. Determinism is especially critical for voice pack certification — a certified voice pack must produce certified audio.

**Test Approach:**

- `tests/determinism_test.rs`: Synthesize the same sentence 10 times consecutively with the same parameters. Compare all outputs using `assert_eq!` on the raw `Float32Array`. All 10 must be bit-identical.
- Run across VOCAL NANO, VOCAL LITE, VOCAL STUDIO model tiers.
- Run on Linux x86_64 CI environment (determinism is per-platform; cross-platform bit-identity is not required).

**Enforcement Mechanism:**

- ONNX Runtime is run with deterministic execution flags where available.
- No `rand` crate usage anywhere in the synthesis pipeline (G2P, prosody, engine, audio). Only the `requestId` generator uses randomness, and it is explicitly outside the pipeline.
- CI golden-file test: A known input produces a known SHA-256 hash of output audio. Fails the build if hash changes.

---

### VOICE_INV_006 — Graceful Degradation

| Field | Value |
|-------|-------|
| **ID** | VOICE_INV_006 |
| **Name** | Graceful Degradation |
| **Severity** | HIGH |

**Statement:**

When the VOCAL STUDIO model is unavailable, the runtime MUST automatically fall back to VOCAL LITE. When VOCAL LITE is unavailable, it MUST fall back to VOCAL NANO. The application MUST receive audio at the available tier rather than an error. Degradation MUST be logged. The application MAY be notified of the degraded tier through the engine health API.

**Rationale:**

Voice synthesis failing silently is worse than degraded synthesis. A kiosk that goes mute because its high-quality model failed to load provides a broken user experience. A kiosk that speaks in lower quality but continues to function is acceptable. The runtime must be resilient.

**Test Approach:**

- `tests/degradation_test.rs`: Initialize with STUDIO tier config. Delete/move the STUDIO model file. Trigger a synthesis. Assert that audio is produced (from LITE model). Assert that `EngineHealth.status` is `'degraded'`, not `'unavailable'`. Assert a degradation warning was emitted to the log.
- Repeat for LITE → NANO fallback.

**Enforcement Mechanism:**

- Not implemented: there is no `ModelLoader` and no tier-fallback logic in `crates/`, and `EngineHealth` has no `degradedTier` field. This invariant describes intended design; `scripts/verify-doc-claims.py` exists so an edit cannot quietly restate it as enforcement. The nearest real behaviour is pack loading in `crates/voice-pack/src/security.rs`, which rejects an over-limit or mismatched pack outright rather than degrading.

---

### VOICE_INV_007 — Local Privacy

| Field | Value |
|-------|-------|
| **ID** | VOICE_INV_007 |
| **Name** | Local Privacy |
| **Severity** | CRITICAL |

**Statement:**

In local mode (no explicit cloud configuration provided by the application), the Chiti Vocal Runtime MUST NOT transmit any synthesized text, any audio data, any user identifiers, or any behavioral telemetry to any remote server. The synthesis path MUST be wholly contained on the local device.

**Rationale:**

Voice input and output are sensitive. The text being synthesized may contain personal information, medical information, financial information, or confidential business content. The user has not consented to that text being transmitted to a third party. Privacy is not a feature — it is a fundamental requirement of a local-first runtime.

**Test Approach:**

- Network isolation test (see also VOICE_INV_001): Synthesize 50 varied test sentences (including sentences with names, numbers, and sensitive-pattern text). Use a network monitor/proxy/packet capture to assert zero outbound packets leave the process during the synthesis session.
- Test the Local Service daemon separately: assert it only binds to `127.0.0.1` and never to `0.0.0.0`.

**Enforcement Mechanism:**

- `vocal-core`: zero network dependencies (see VOICE_INV_001).
- `chiti-vocal-service`: explicit bind address `127.0.0.1` in code, not configurable to wildcard.
- Production build flag `CHITI_VOICE_DEV_LOG` defaults to false; text is never logged unless explicitly enabled.

---

### VOICE_INV_008 — Voice Provenance

| Field | Value |
|-------|-------|
| **ID** | VOICE_INV_008 |
| **Name** | Voice Provenance |
| **Severity** | HIGH |

**Statement:**

Every `.cvpack` voice pack MUST contain a `manifest.json` that declares: the voice model's origin, the training dataset license, the persona configuration version, SHA-256 checksums of all model files, and a `signatureStatus` field. The runtime MUST verify all checksums before loading any model file from a pack. A pack with missing, mismatched, or unsigned provenance MUST NOT be silently loaded as if it were verified.

**Rationale:**

Voice models trained on data without proper consent or licenses create legal and ethical liability. Checksum verification prevents model file tampering. Provenance metadata creates an audit trail for each voice used in production. As Chiti grows, knowing exactly which model version produced which audio will be essential for quality management, legal compliance, and debugging.

**Test Approach:**

- `tests/pack_verify_test.rs`: Load a valid pack. Assert success. Corrupt one byte of the model file. Attempt to load. Assert `PackVerificationError` is returned, not success.
- Load a pack with `signatureStatus: "UNSIGNED"`. Assert the runtime loads it but emits a `PackUnsignedWarning` log entry and returns `signatureStatus: "UNSIGNED"` in the loaded pack metadata.
- Attempt to load a pack with no manifest. Assert `PackManifestMissingError`.

**Enforcement Mechanism:**

- `crates/voice-pack/src/security.rs`: `validate_files` recomputes each file's SHA-256 and returns `Err` on mismatch, so a tampered pack never reaches a loader. There is no checksum bypass; the verifier's only relaxation is `without_provenance_check()`, which skips the provenance-completeness requirement for research tooling and has no caller in `crates/` or `apps/`.
- `manifest.rs` schema validation runs before extraction of any pack contents.

---

### VOICE_INV_009 — Interruptibility

| Field | Value |
|-------|-------|
| **ID** | VOICE_INV_009 |
| **Name** | Interruptibility |
| **Severity** | HIGH |

**Statement:**

It MUST be possible to cancel any in-progress synthesis or playback within 100 milliseconds of calling `cancel()`. Cancellation MUST be possible from the SYNTHESIZING, BUFFERING, and PLAYING states. After cancellation completes, the runtime MUST return to READY state and be immediately available for a new synthesis request. Cancellation MUST NOT corrupt state.

**Rationale:**

Real conversational voice interfaces require barge-in: the user must be able to interrupt the system's speech at any time to speak themselves or stop the output. A runtime that takes 2 seconds to cancel a synthesis is unusable in interactive applications. State corruption after cancellation would cause the runtime to become unusable without restart.

**Test Approach:**

- `tests/interruptibility_test.rs`: Start synthesis of a long text (500+ words). After 50 ms, call `cancel()`. Measure wall time to completion of cancel. Assert `< 100 ms`.
- After cancel, call `synthesize()` with a short text. Assert success (state is READY, not corrupted).
- Stress test: Cancel 100 times consecutively with varying timings. Assert no state corruption and no panics.

**Enforcement Mechanism:**

- State machine enforces the CANCELLING → READY transition.
- Engine adapters implement `cancel()` using cooperative cancellation tokens passed into the synthesis task.
- CI test runs interruptibility tests on every PR.

---

### VOICE_INV_010 — Streaming Safety

| Field | Value |
|-------|-------|
| **ID** | VOICE_INV_010 |
| **Name** | Streaming Safety |
| **Severity** | HIGH |

**Statement:**

When `stream()` is used, the `AsyncIterable<PcmChunk>` MUST be safe to drop mid-stream without leaking resources. Dropping a stream MUST trigger the same cleanup as an explicit `cancel()` call. A dropped stream MUST NOT leave the engine in SYNTHESIZING state.

**Rationale:**

JavaScript consumers of the streaming API may drop the iterable early for many reasons (user navigated away, timeout, error handler). If dropping an iterable leaks resources or corrupts engine state, the runtime will degrade silently over time until it becomes unresponsive.

**Test Approach:**

- `tests/stream_safety_test.rs`: Start a stream. Pull 3 chunks. Drop the iterable without consuming to completion. Assert engine state returns to READY. Assert no memory leak (tracked via allocation counters in test build).
- Run under Valgrind/ASAN in CI to detect leak on stream drop.

**Enforcement Mechanism:**

- `Drop` implementation on the Rust stream type calls `cancel()` on the backing engine task.
- CI integration test using ASAN confirms no leaks.

---

### VOICE_INV_011 — Resource Limits

| Field | Value |
|-------|-------|
| **ID** | VOICE_INV_011 |
| **Name** | Resource Limits |
| **Severity** | HIGH |

**Statement:**

The runtime MUST enforce maximum limits on synthesis requests to prevent resource exhaustion. Specifically:
- Maximum text length per request: 10,000 characters (configurable, this is the default).
- Maximum concurrent synthesis requests: 4 (configurable).
- Maximum model memory: enforced by tier selection (NANO < 30 MB, LITE < 100 MB, STUDIO < 350 MB).
- Requests exceeding text length MUST return `TextTooLongError`, never attempt synthesis.
- A queue of pending requests MUST be bounded; exceeding the queue MUST return `QueueFullError`, never block indefinitely.

**Rationale:**

Without limits, a single malicious or buggy caller can exhaust device memory, starve the CPU, or block the runtime indefinitely. Embedded and mobile targets have strict memory budgets. The Local Service daemon is exposed to web origins that may send adversarial payloads.

**Test Approach:**

- `tests/resource_limits_test.rs`: Submit a request with 10,001 characters. Assert `TextTooLongError`.
- Submit 5 rapid concurrent requests with concurrency limit set to 4. Assert the 5th returns `QueueFullError`.
- Memory profiling test: Run 100 synthesis cycles with NANO model. Assert peak RSS stays within 2× the model size.

**Enforcement Mechanism:**

- `VocalClient` validates text length before dispatching to the engine.
- `queue.rs` in the Local Service uses a bounded queue (tokio `mpsc` with capacity).
- Resource limit constants are defined in a single configuration module — not scattered across the codebase.

---

### VOICE_INV_012 — Version Compatibility

| Field | Value |
|-------|-------|
| **ID** | VOICE_INV_012 |
| **Name** | Version Compatibility |
| **Severity** | MEDIUM |

**Statement:**

The Chiti Voice SDK (TypeScript) MUST follow semantic versioning. A MINOR version bump MUST NOT remove or change any existing public API. A MAJOR version bump MUST be accompanied by a migration guide. Voice packs include a `manifestVersion` field; the runtime MUST reject packs whose `manifestVersion` is incompatible with the running runtime version, with a clear error message stating the version mismatch.

**Rationale:**

Applications that integrate the SDK will not all upgrade simultaneously. Breaking changes without a semantic version signal cause silent failures. Voice packs built for one runtime version must not silently misbehave when loaded by a different runtime version — they must fail clearly so the issue is diagnosable.

**Test Approach:**

- `tests/version_compat_test.rs`: Load a pack with `manifestVersion: "999.0.0"`. Assert `IncompatibleManifestVersionError` with a message containing both the pack version and the runtime version.
- TypeScript API test: Export the SDK type surface. Snapshot it. Assert on each MINOR release that no existing exported type has been removed or changed (using `api-extractor` or equivalent).

**Enforcement Mechanism:**

- `manifest.rs` includes a semver-compatible version check using the `semver` crate.
- `api-extractor` or `tsd` runs in CI to detect breaking TypeScript API changes.
- Changelog must be updated as part of any release PR.

---

## Invariant Violation Protocol

If you are working on a change that appears to require violating one of the above invariants, follow this protocol exactly:

### Step 1 — Stop and Identify

Do not proceed with the implementation. Identify:
- Which invariant ID is at risk
- What specifically would be violated
- Why the violation appears necessary

### Step 2 — Write an ADR

Create a new ADR in `docs/architecture/` (e.g., `ADR-010-temporary-inv001-relaxation.md`) that includes:
- The invariant ID and its statement
- The specific change being proposed
- The justification for why the violation is necessary
- Whether the violation is permanent or temporary
- If temporary: the exact condition under which it will be resolved and the target date
- If permanent: a proposed amendment to the invariant itself

### Step 3 — Write a Test First

Before any implementation code is written, write a test that:
- Demonstrates the current invariant is being upheld (or cannot be upheld due to the proposed change)
- Will serve as a regression guard once the change is made

### Step 4 — Architect Review

The ADR must be reviewed and approved by at least one engineer not on the implementing team. The review must be documented (a GitHub PR review comment is sufficient). No self-approval.

### Step 5 — Update the Invariant Document

If the ADR is approved and the change proceeds:
- Update this document to reflect the amended invariant or the approved exception
- Add a cross-reference to the ADR in the invariant's **Enforcement Mechanism** section
- Update the invariant severity if appropriate

### What Happens If Protocol Is Skipped

A pull request that demonstrably violates an invariant without a corresponding ADR and architect review will be:
1. Blocked from merging by code owners
2. Documented as a process violation
3. Reverted if it somehow merges

There are no emergency exceptions. If the emergency is real, the protocol still applies — it just happens faster.

---

*Document maintained by the Chiti Platform Team. For the state machine these invariants protect, see [STATE_MACHINE.md](./STATE_MACHINE.md). For security constraints, see [SECURITY.md](./SECURITY.md).*

---

## Requirements tracked in `PRD.md` without a canonical invariant

`PRD.md`'s invariants table re-used IDs `VOICE_INV_003`–`012` for nine requirements this document does
not define, while code, `SECURITY.md` and `STATE_MACHINE.md` cite the IDs above. The table now keeps
the canonical IDs and marks the rest **PRD-only** rather than assigning them conflicting numbers, so
they are not lost -- they are simply not enforced here:

- Language-Voice Separation · Engine Interface · No Direct Backend Instantiation
- Persona Config Separation · Loopback Only and No Telemetry (each a specific clause of
  VOICE_INV_007) · Pack Integrity (enforced as part of VOICE_INV_008 by `validate_files`)
- No Executable Content · RTF Bound (the only exit criterion in the PRD with no implementation
  or measurement behind it: RTF figures in `docs/research/` are x86-container numbers, not the
  reference hardware)

Promoting any of them into this document is a product decision, not a documentation cleanup, so it has
been left open deliberately. What is no longer optional is the ID↔name pairing:
`scripts/verify-doc-claims.py` parses the `**ID**`/`**Name**` table above, requires this file's headings
and table to agree, and fails any markdown file *or Rust doc comment* that pairs an ID with a different
name or cites an ID that is not defined. Run it with `--self-test` to see that it can fail.
