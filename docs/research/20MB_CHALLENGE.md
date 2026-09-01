# The 20 MB Voice Challenge
Classification: RESEARCH

## Challenge Statement
Create a voice persona that:
- Fits in 20 MB total
- Runs offline on a Raspberry Pi 4 (2 GB RAM)
- Is recognizably and consistently Tara (or Kashi or Bobo)
- Produces intelligible English speech at > 4.0/5.0 MOS
- Produces real-time speech (RTF < 1.0) on target hardware

## Why 20 MB?
- Comparable to a high-quality font family.
- Fits in a typical web app service worker cache.
- Deployable on embedded hardware with 256 MB storage.
- Small enough to be treated as a "voice asset" rather than a heavy ML model.

## Approaches
1. **Aggressive Quantization:** INT8 / INT4 static quantization of ONNX graphs.
2. **Architecture Distillation:** Student-teacher model distillation.
3. **Shared Foundation + Adapter:** Common backbone + small speaker adapters.
4. **Hybrid Rule + Neural:** Deterministic G2P + minimal neural vocoder.

## Success Criteria
| Metric | Minimum | Target | North-Star |
|---|---|---|---|
| Total Size | < 50 MB | < 20 MB | < 10 MB |
| RTF (RPi4) | < 2.0 | < 1.0 | < 0.5 |
| MOS (English) | > 3.5 | > 4.0 | > 4.3 |
| Persona ID Rate | > 60% | > 75% | > 85% |
