# ADR-001: Initial TTS Backend Selection

| Field | Value |
|-------|-------|
| **Status** | PROPOSED — accepted in part for the T0 tier; see `docs/ROADMAP_EMBEDDED.md` §2 Step 1 and ADR-002 requirement |
| **Date** | September 2026 |
| **Deciders** | Chiti Platform Team |
| **Supersedes** | — |
| **Superseded by** | — |
| **Related ADRs** | ADR-002 (planned): Hindi voice model selection · ADR-003 (planned): Browser-native engine selection |

---

## Context

Chiti Vocal Runtime Phase 1 was built with a `MockEngine` — a silence-emitting stub that validates the full pipeline without producing real speech. Phase 1 established:

- The `VoiceEngine` interface (abstract, swappable)
- The Persona Runtime (persona config → synthesis params)
- The State Machine (UNINITIALIZED → READY → SYNTHESIZING → PLAYING → READY)
- The `.cvpack` format and manifest verification (**verification was added 2026-09-03; the
  three shipped packs previously failed their own checksums**)
- Error codes in `vocal-core` (**pack-security codes were unreachable until the
  `From<LoadError> for VoiceError` mapping**)
- A CI skeleton (**with gates that could not fail; now replaced**)
- **Not** a TypeScript SDK, **not** a local daemon, **not** any audible output

**Phase 2 requires real speech.** The team must select a concrete TTS backend to integrate behind the `VoiceEngine` interface and produce an offline demo with:
- A working Tara voice (Indian English)
- A working Kashi voice (Hindi)
- Sub-second first-chunk latency on a mid-range laptop

**The choice of backend must not couple applications to it.** The `VoiceEngine` abstraction means this is a runtime implementation decision, not an API decision. Applications will never see the engine name.

**The decision is about which backend to integrate first**, not which backend to use forever. The architecture explicitly supports multiple registered engines.

---

## Requirements for the Backend

The following requirements were used to evaluate candidates. Weight reflects importance to Phase 2 success.

| Requirement | Weight | Why |
|-------------|--------|-----|
| Indian English quality | HIGH | Tara is the primary demo voice; quality must be convincing |
| Hindi support | HIGH | Kashi is the second core persona; Hindi must work offline |
| Model size < 150 MB | MEDIUM | Must fit comfortably on developer laptops and demo devices; NANO tier target is < 30 MB |
| CPU real-time factor < 1.0 | HIGH | Audio must be produced faster than it plays; RTF > 1.0 means choppy streaming |
| ONNX format available | HIGH | ONNX Runtime is the chosen inference backend; non-ONNX models require a separate runtime dependency |
| Browser WASM compatibility | MEDIUM | Phase 11 deliverable; not required for Phase 2 but must not be architecturally blocked |
| Streaming support | HIGH | First-chunk latency requirement; full synthesis before playback is not acceptable for long texts |
| License permissiveness | HIGH | Must allow commercial use and redistribution in `.cvpack` bundles. **Evaluate the whole path, not the engine repo's license:** Piper's engine is MIT but its voices are licensed per model card, and Piper's phonemization backend espeak-ng is GPL-3.0, which is a distribution-obligation question for a proprietary binary. See `docs/ROADMAP_EMBEDDED.md` §3 and `LICENSE`. **This requirement was scored in the earlier draft without checking the G2P dependency, and that gap is why ADR-002 must record an explicit decision.** |
| Actively maintained | MEDIUM | Security updates and bug fixes; unmaintained projects are a long-term risk |
| Quantization support | MEDIUM | Required for VOCAL NANO tier (< 30 MB target); INT8 quantization must be possible |

---

## Candidates Evaluated

### Candidate A: Kokoro / Kokoro-82M

**Architecture:** Flow-matching TTS, StyleTTS2-inspired. Produces high-quality, expressive speech with style vector control.

| Property | Value |
|----------|-------|
| Parameters | ~82 million |
| Model size (FP32) | ~330 MB |
| Model size (INT8 quantized) | ~82 MB |
| License | Apache 2.0 |
| ONNX availability | Community-converted; not official |
| Maintenance status | Active (as of September 2026) |

**Indian English:** Good. Trained on diverse English corpora. Accent is configurable via speaker embeddings. Indian English speaker embeddings are available in the community.

**Hindi:** Limited native support. The base model does not include a Hindi voice. Hindi would require training or fine-tuning a separate Hindi speaker embedding, which is a significant research effort not suitable for Phase 2.

**CPU Real-Time Factor:** ~0.5–0.8 on a modern laptop (Intel Core i7, no GPU). Faster than real-time on the reference machine, but sensitive to thread count and memory bandwidth.

**Streaming:** Chunk-based streaming is architecturally possible (the model can generate phoneme-aligned segments) but is not a built-in feature of the current community releases. Would require custom implementation.

**Browser WASM:** Experimental. ONNX Runtime Web can run the model in theory, but the ~82 MB INT8 model plus the WASM runtime is a large initial download. Performance on low-end devices is unverified.

**Pros:**
- Highest available quality among evaluated candidates
- Apache 2.0 license — commercially permissive
- Active community, good tooling
- Rich style control via style vectors — excellent for persona differentiation
- Good expressiveness, suitable for Tara's warm, professional character

**Cons:**
- Larger model than Piper; NANO tier would require aggressive quantization
- Hindi requires a separate, non-trivial research track
- ONNX conversion is community-maintained, not official
- WASM performance unverified on target devices
- Streaming requires custom implementation effort

---

### Candidate B: Piper TTS

**Architecture:** VITS-based neural TTS. Trained per-language and per-voice. Well-documented, widely deployed.

| Property | Value |
|----------|-------|
| Parameters | ~28–65 million (voice-dependent) |
| Model size | ~28 MB (low quality) to ~65 MB (high quality) |
| License | MIT |
| ONNX availability | Native ONNX format — this is the primary distribution format |
| Maintenance status | Active; Rhasspy project + community |

**Indian English:** Available voices for `en-IN`. Quality is good — intelligible and natural. Less expressive than Kokoro but suitable for professional assistant voice at LITE tier.

**Hindi:** Available voice for `hi-IN`. Proven offline Hindi synthesis out of the box. This directly satisfies the Kashi Phase 2 requirement.

**CPU Real-Time Factor:** ~0.05–0.3 on a modern laptop. Extremely fast — capable of real-time synthesis on a Raspberry Pi 4. Well within the RTF < 1.0 requirement even on constrained hardware.

**Streaming:** Sentence-level streaming is well-supported. The synthesizer processes one sentence at a time and yields audio; the first chunk arrives after the first sentence is synthesized (typically < 200 ms).

**Browser WASM:** Better prospect than Kokoro due to smaller model size. ONNX Runtime Web can run Piper models; community reports of working browser deployments exist.

**Pros:**
- Small, fast — best-in-class inference performance
- MIT license — maximally permissive
- Native ONNX — no conversion required
- Proven Hindi voice (`hi-IN`) — Kashi works out of the box
- Multiple quality tiers per voice — NANO, LITE, STUDIO tier mapping is natural
- Established deployment track record (used in Home Assistant, Rhasspy, etc.)
- Raspberry Pi capable — supports embedded/IoT targets

**Cons:**
- Lower expressiveness than Kokoro — less style control
- Fewer prosodic variation options — persona differentiation will be more limited
- Older VITS architecture — may be superseded by flow-matching models for quality
- Indian English voice is good but not exceptional; may not fully represent Tara's character at STUDIO tier

---

### Candidate C: Coqui TTS (archived)

**Status:** Project archived as of early 2024. Not recommended for new integration.

**Risk Assessment:** The core maintainers have stopped active development. Security vulnerabilities will not be patched. The community fork situation is fragmented. Integrating an archived project as a primary backend creates unacceptable long-term maintenance risk.

**Decision:** **Eliminated.** Not evaluated further.

---

### Candidate D: Edge TTS / Cloud TTS (eliminated)

**Reason for Elimination:** Any cloud TTS service — Microsoft Edge TTS, Google Cloud TTS, ElevenLabs, AWS Polly, or similar — **violates VOICE_INV_001 (Offline Independence)**. Cloud synthesis requires an internet connection. No further evaluation is needed; the invariant forecloses this option regardless of quality or cost.

**Decision:** **Eliminated.** Violates VOICE_INV_001. Cannot be reconsidered without an architectural review and ADR amending the invariant.

---

## Decision Matrix

Scores: 3 = Fully meets requirement · 2 = Partially meets · 1 = Does not meet

| Requirement | Weight | Kokoro Score | Kokoro Weighted | Piper Score | Piper Weighted |
|-------------|--------|-------------|-----------------|-------------|----------------|
| Indian English quality | HIGH (3) | 3 | 9 | 2 | 6 |
| Hindi support | HIGH (3) | 1 | 3 | 3 | 9 |
| Model size < 150 MB | MEDIUM (2) | 2 | 4 | 3 | 6 |
| CPU RTF < 1.0 | HIGH (3) | 2 | 6 | 3 | 9 |
| ONNX format | HIGH (3) | 2 | 6 | 3 | 9 |
| Browser WASM | MEDIUM (2) | 1 | 2 | 2 | 4 |
| Streaming support | HIGH (3) | 2 | 6 | 3 | 9 |
| License permissiveness | HIGH (3) | 3 | 9 | 3 | 9 |
| Actively maintained | MEDIUM (2) | 3 | 6 | 3 | 6 |
| Quantization support | MEDIUM (2) | 2 | 4 | 3 | 6 |
| **TOTAL** | | | **55** | | **73** |

---

## Decision

**RECOMMENDED:**

> **Piper TTS as the Phase 2 primary backend.**  
> **Kokoro as the Phase 2B / Phase 3 quality upgrade target.**

### Rationale

**Why Piper first:**

1. **Hindi works today.** The `hi-IN` Piper voice is available, ONNX-native, and proven. Kashi can speak Hindi in Phase 2 without a research detour. Hindi is a non-negotiable requirement.

2. **Smallest to fastest path to a working demo.** Piper is the fastest available offline TTS. First-chunk latency will be excellent. The NANO tier target is achievable. A working demo on constrained hardware validates the full architecture.

3. **MIT license.** Maximally permissive. No legal review needed for redistribution in `.cvpack` bundles.

4. **Native ONNX.** Zero conversion work. Load the ONNX model file, wrap it in `PiperEngineAdapter`, ship.

5. **Proven at scale.** Piper is deployed in production in Home Assistant (millions of devices). Its behavior is well-understood. There are no integration surprises.

**Why Kokoro later (not never):**

1. **Tara quality ceiling.** Piper's Indian English is good but not exceptional. Tara's character requires warmth, expressiveness, and naturalness that Kokoro's style vectors can deliver better. For the STUDIO tier, Kokoro is the right target.

2. **Persona differentiation.** Kokoro's style control enables richer intent profile differentiation. A Piper engine will have limited latitude to vary delivery between `greeting` and `narration`; Kokoro can vary meaningfully.

3. **The interface absorbs the swap.** Because the `VoiceEngine` interface abstracts the backend completely, moving from Piper to Kokoro (or running both) is an implementation change, not an API change. Applications are unaffected.

**Coexistence plan:**

Both Piper and Kokoro can be registered as named engines in the engine registry:
- `"piper-lite"` → PiperEngineAdapter (Phase 2, Hindi + English)
- `"kokoro-studio"` → KokoroEngineAdapter (Phase 3, English, quality tier)

The Persona Runtime selects the engine based on the hardware tier and persona config. Applications never pick an engine by name.

---

## Consequences

### Immediate Actions (Phase 2)

1. **Implement `PiperEngineAdapter`** in `crates/vocal-core/src/engine/piper.rs`. This text predates the crate rename, and the file that exists today is a skeleton: `REAL_SYNTHESIS_AVAILABLE` is still `false`, so nothing in `crates/` produces audio from the graph yet.
   - Load Piper ONNX model via `ort` (ONNX Runtime Rust bindings).
   - Implement `initialize()`, `loadVoice()`, `synthesize()`, `stream()`, `cancel()`, `health()`, `capabilities()`, `dispose()`.
   - Stream by sentence boundary (Piper's natural unit).

2. **Build Tara en-IN voice pack** using Piper's `en_IN-...` voice.
   - Wrap in `.cvpack` format with `persona.json` for Tara.
   - Run full checksum verification and manifest validation.

3. **Build Kashi hi-IN voice pack** using Piper's `hi_IN-...` voice.
   - Wrap in `.cvpack` format with `persona.json` for Kashi.

4. **Remove MockEngine as the default** in integration and demo configurations. MockEngine remains mandatory for CI unit tests.

5. **Benchmark PiperEngineAdapter** against the evaluation sentence set:
   - RTF on reference laptop (Intel Core i7, 16 GB RAM, no GPU)
   - RTF on Raspberry Pi 4 (4 GB)
   - First-chunk latency (ms)
   - MOS score (automated UTMOS/MOSNet; human evaluation in Phase 4)

### Parallel Research Track (Phase 2B)

6. **Begin `KokoroEngineAdapter` implementation** in parallel — not blocking Phase 2.
   - Use community ONNX conversion of Kokoro-82M.
   - Target: English synthesis working with Tara persona at STUDIO tier.
   - Hindi: investigate community Hindi speaker embeddings; document findings in ADR-002.

### Ongoing

7. **MockEngine remains mandatory for all CI tests.** No test may depend on Piper or Kokoro being installed. Tests that require a real engine must be tagged `#[ignore]` in CI and run in a separate `integration` test suite.

8. **Benchmark both engines** against the same evaluation sentences with the same metrics when KokoroEngineAdapter is ready. Document results in a benchmark report. Use results to inform ADR-002 (Hindi model) and ADR-003 (browser-native engine).

9. **Revisit this ADR after Phase 4 human evaluation results.** If MOS scores indicate Piper quality is insufficient for Tara, escalate Kokoro integration timeline.

---

## Related Decisions

| ADR | Status | Topic |
|-----|--------|-------|
| ADR-002 | PLANNED | Hindi voice model selection (Kashi) — which specific hi-IN model, training data, and quantization strategy |
| ADR-003 | PLANNED | Browser-native engine selection — which model variant runs via ONNX Runtime Web + WASM for browser-native mode |

---

*Decision recorded by the Chiti Platform Team, September 2026. To amend or supersede this decision, create a new ADR and update the Superseded by field above.*
