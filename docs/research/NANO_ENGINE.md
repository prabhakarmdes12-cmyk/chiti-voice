# Chiti Vocal Runtime — Nano Engine Research Track
Classification: RESEARCH (not production commitments)
Date: September 2026

## Research Question
> How small can a pleasant, recognizable, expressive, multilingual machine voice become — while remaining offline-capable on low-end hardware?

## The Font Analogy
```
TYPOGRAPHY MODEL:
  Large shared renderer (FreeType, HarfBuzz) + small font files (.ttf, ~100 KB–5 MB)
  ? Many typefaces, one renderer

VOCAL RUNTIME HYPOTHESIS:
  Shared small acoustic foundation (~20–40 MB) + small voice adapters (~1–10 MB per voice)
  ? Many voices, one model
```

## Architecture Candidates to Investigate
1. **VITS / FastSpeech2 Distillation** — Distill a large model into a smaller backbone.
2. **StyleTTS2 Adapter Pattern** — Shared backbone + per-speaker style embeddings.
3. **Flow-Matching Nano** — Minimal flow-matching architecture from scratch.
4. **Kokoro-q8 INT8 Quantization** — Quantize existing Kokoro to INT8 via ONNX.
5. **Piper VITS (existing baseline)** — Baseline: already small, what is the quality floor?
6. **Low-Rank Adaptation (LoRA)** — Shared backbone + per-voice LoRA adapter weights.

## Baseline Targets
- **Phase 1 baseline:** < 150 MB total (model + voice assets)
- **Research target:** < 60 MB
- **North-star:** ~20 MB pleasant, recognizable persona

## Hardware Test Matrix
| Hardware | Target Tier | Goal |
|---|---|---|
| Laptop (i7 / M2) | STUDIO | Highest quality, fast RTF |
| Mid-range Laptop | LITE | Standard desktop experience |
| Raspberry Pi 4 / 5 | LITE / NANO | Embedded kiosk / robot runtime |
| Android Mid-Range | NANO | Mobile offline runtime |
| Browser (WASM) | WASM | Zero-install web voice |
