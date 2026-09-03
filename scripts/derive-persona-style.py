#!/usr/bin/env python3
"""Derive a persona as a weighted blend of stock Kokoro style vectors, then measure what it did.

The premise, from `docs/research/KOKORO_OFFLINE_SPIKE.md` finding 1: in this engine family a
*voice* is a 510 x 256 float32 matrix, not a model. So a persona can be expressed as a recipe over
existing vectors and rendered against the shared 88 MB graph — which makes it the only version of
"generate me a voice" that is reproducible, auditable, and 522 KB per speaker.

Two things this deliberately does not pretend:

* **A blend is not a speaker.** It is an interpolation between real people's recordings and it
  inherits their licence terms as a derivative work, so it is for *direction* (does this register
  and this pace read as KASHI?) and for placeholder builds — never for a release. `VOICE_INV_008`
  refuses a `real` pack without provenance, and a blend has none. The shippable asset still needs
  path C: a commissioned speaker, a consent contract, and training that yields a vector we own.
* **The blend is not a perceptual average.** Every row is mixed, not just the one row a sentence
  happens to use, because a voice whose character depends on how the caller chunked the text would
  be an unreproducible bug. Measuring the result against its sources is how we learn whether the
  blend landed where the weights imply — and the printed deviation says plainly when it does not,
  because a neural model is not linear in its style input.

    /tmp/pv/bin/python scripts/derive-persona-style.py --model-dir models \
        --persona kashi --sources hm_omega:0.6,hf_alpha:0.4 --speed 0.92 --lang hi \
        --text "आपका ड्राइवर शुक्रवार को आ रहा है।" --report --wav-out /tmp/kashi.wav

Use `--recipe-out docs/research/persona-recipes/<persona>.json` to keep the *recipe* in git while
the derived vector stays out: the file that reproduces a voice is worth more than the bytes.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import wave
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import voice_metrics as vm  # noqa: E402


def parse_sources(spec: str) -> dict[str, float]:
    """`a:0.6,b:0.4` -> normalised weights. Bare names are equal-weighted."""
    out: dict[str, float] = {}
    for part in spec.split(","):
        part = part.strip()
        if not part:
            continue
        name, sep, w = part.partition(":")
        out[name.strip()] = float(w) if sep and w else 1.0
    total = sum(out.values())
    if total <= 0:
        raise SystemExit("weights must sum to a positive number")
    return {k: v / total for k, v in out.items()}


def main() -> int:
    ap = argparse.ArgumentParser(description="Blend Kokoro style vectors into a persona and measure it")
    ap.add_argument("--model-dir", default="models")
    ap.add_argument("--persona", required=True)
    ap.add_argument("--sources", required=True, help="voice[:weight],voice[:weight]")
    ap.add_argument("--text", default="Your total is 240 rupees, and the driver will arrive on Friday.")
    ap.add_argument("--lang", default="en-us", help="espeak-ng voice used for phonemisation")
    ap.add_argument("--speed", type=float, default=1.0)
    ap.add_argument("--threads", type=int, default=1)
    ap.add_argument("--out-bin", default=None, help="write the blended vector (models/ is gitignored)")
    ap.add_argument("--wav-out", default=None)
    ap.add_argument("--recipe-out", default=None)
    ap.add_argument("--target-dbfs", type=float, default=None,
                    help="post-stage loudness target; the spec's `Energy` has no model input, so gain is the "
                         "honest approximation and it also prevents the clipping this survey found in 8 voices")
    ap.add_argument("--peak-ceiling", type=float, default=0.98)
    ap.add_argument("--max-gain-db", type=float, default=12.0,
                    help="refuse to amplify more than this: normalising a near-silent clip to a "
                         "loud target otherwise raises the noise floor by hundreds of dB "
                         "(vocal-core's LoudnessSpec::DEFAULT_MAX_GAIN_DB is the same number)")
    ap.add_argument("--report", action="store_true", help="also print the interpolation verdict lines")
    args = ap.parse_args()

    try:
        import numpy as np
        import onnxruntime as ort
        from piper.phonemize_espeak import EspeakPhonemizer
    except ImportError as e:  # noqa: BLE001
        sys.exit(f"missing dependency ({e}): pip install onnxruntime numpy piper-tts")

    model_dir = Path(args.model_dir)
    weights = parse_sources(args.sources)
    unknown = [n for n in weights if not (model_dir / "voices" / f"{n}.bin").exists()]
    if unknown:
        sys.exit(f"voices not in {model_dir}/voices: {', '.join(unknown)} — re-run scripts/fetch-offline-model.py --all-voices")

    vocab, maxlen, pattern = vm.load_tokenizer(model_dir / "tokenizer.json")
    so = ort.SessionOptions()
    so.intra_op_num_threads = args.threads
    so.inter_op_num_threads = args.threads
    sess = ort.InferenceSession(str(model_dir / "kokoro-quantized.onnx"), so, providers=["CPUExecutionProvider"])
    ph = EspeakPhonemizer()

    phonemes = " ".join("".join(s) for s in ph.phonemize(args.lang, args.text))
    ids = vm.encode_ids(phonemes, vocab, maxlen, pattern)
    n_tokens = max(len(ids) - 2, 0)
    print(f"persona {args.persona}: {len(weights)} source vector(s), {n_tokens} phoneme tokens, speed {args.speed}")

    blend = np.zeros(510 * 256, dtype=np.float64)
    per_source: list[dict] = []
    for name, w in weights.items():
        m = vm.load_style(model_dir / "voices" / f"{name}.bin")
        blend += m.astype(np.float64) * w
        t0 = time.perf_counter()
        src = vm.measure(vm.synth(sess, phonemes, m, ids, args.speed), n_tokens)
        src.update({"voice": name, "weight": round(w, 4), "infer_s": round(time.perf_counter() - t0, 2)})
        per_source.append(src)

    blend32 = blend.astype(np.float32)
    t0 = time.perf_counter()
    pcm = vm.synth(sess, phonemes, blend32, ids, args.speed)
    infer = time.perf_counter() - t0
    got = vm.measure(pcm, n_tokens)
    got.update({"voice": f"blend:{args.persona}", "weight": 1.0, "infer_s": round(infer, 2)})

    cols = ["voice", "weight", "f0 Hz", "range Hz", "phon/s", "dBFS", "peak", "dur s"]
    print(vm.fmt_row(cols))
    print(vm.fmt_row(["---"] * len(cols)))
    for r in per_source + [got]:
        print(vm.fmt_row([r["voice"], f"{r['weight']:.2f}", vm.fmt(r["f0_median_hz"]), vm.fmt(r["f0_range_hz"]),
                          vm.fmt(r["phonemes_per_s"], ".2f"), vm.fmt(r["level_dbfs"]), vm.fmt(r["peak"], ".3f"),
                          vm.fmt(r["duration_s"], ".2f")]))

    f0s = [r["f0_median_hz"] for r in per_source if r["f0_median_hz"]]
    if args.target_dbfs is not None:
        before = dict(got)
        f = pcm.astype("float64") / 32768.0
        rms_now = float(np.sqrt(np.mean(f * f))) or 1e-9
        peak_now = float(np.max(np.abs(f))) or 1e-9
        # Mirrors `crates/vocal-core/src/audio_levels.rs::plan` step for step: silence is left
        # alone instead of dividing, then the ceiling and the gain cap each get to veto the target.
        if rms_now == 0.0 or peak_now == 0.0:
            gain, ceiling_limited, gain_limited = 1.0, False, False
        else:
            want = 10.0 ** (args.target_dbfs / 20.0)
            by_target, by_ceiling = want / rms_now, args.peak_ceiling / peak_now
            gain = min(by_target, by_ceiling)
            ceiling_limited = by_ceiling < by_target - 1e-12
            cap = 10.0 ** (args.max_gain_db / 20.0)
            gain_limited = gain > cap
            if gain_limited:
                gain = cap
        pcm = np.clip(np.floor(f * gain * 32767.0), -32768, 32767).astype("<i2")
        got = vm.measure(pcm, n_tokens)
        got.update({"voice": f"blend:{args.persona}", "weight": 1.0, "infer_s": round(infer, 2)})
        notes = []
        if ceiling_limited:
            notes.append("peak ceiling held it back; the target was unreachable without clipping")
        if gain_limited:
            notes.append(f"amplification capped at {args.max_gain_db:+.1f} dB; this clip is too quiet to normalise honestly")
        print(f"  loudness: {before['level_dbfs']} -> {got['level_dbfs']} dBFS (gain {20*float(np.log10(max(gain, 1e-12))):+.1f} dB, "
              f"peak {got['peak']:.3f}{'; ' + ', '.join(notes) if notes else ''})")
        # The table above intentionally shows the *pre*-normalisation numbers for the sources, so
        # the gain applied here stays attributable; only the blend row changes after it.

    if args.report and got["f0_median_hz"] and len(f0s) > 1:
        lo, hi = min(f0s), max(f0s)
        inside = lo * 0.97 <= got["f0_median_hz"] <= hi * 1.03
        wsum = sum(r["weight"] for r in per_source if r["f0_median_hz"])
        target = sum(r["f0_median_hz"] * r["weight"] for r in per_source if r["f0_median_hz"]) / max(wsum, 1e-9)
        dev = 100 * abs(got["f0_median_hz"] - target) / target
        print(f"\n  register: sources span {lo}-{hi} Hz; blend measures {got['f0_median_hz']} Hz "
              f"({'inside the range, as a blend should be' if inside else 'OUTSIDE the source range — this mix does not interpolate'})")
        print(f"  weighted-mean prediction {target:.1f} Hz, deviation {dev:.1f}%: the graph is not linear in style, "
              f"so a recipe predicts direction, never a value — measure the result before believing the weights")

    if args.wav_out:
        out = Path(args.wav_out)
        out.parent.mkdir(parents=True, exist_ok=True)
        with wave.open(str(out), "wb") as w:
            w.setnchannels(1); w.setsampwidth(2); w.setframerate(vm.SAMPLE_RATE)
            w.writeframes(pcm.tobytes())
        print(f"  audio  -> {out}")
    if args.out_bin:
        Path(args.out_bin).write_bytes(blend32.tobytes())
        print(f"  vector -> {args.out_bin} (models/ is gitignored; this is a derivative, not a licensable asset)")
    if args.recipe_out:
        rec = Path(args.recipe_out)
        rec.parent.mkdir(parents=True, exist_ok=True)
        rec.write_text(json.dumps({
            "persona": args.persona,
            "kind": "derived-blend — not a speaker; a derivative work of the source voices",
            "sources": [{"voice": k, "weight": round(v, 4)} for k, v in weights.items()],
            "blend_rule": "elementwise weighted sum over the full 510x256 f32 matrix (every row)",
            "speed": args.speed, "espeak_voice": args.lang, "text": args.text,
            "model": "kokoro-quantized.onnx (pin its sha256 and each source vector's before reproducing)",
            "provenance_status": "incomplete by design — VOICE_INV_008 must keep refusing this as a 'real' pack",
            "measured": {k: got.get(k) for k in ("f0_median_hz", "f0_range_hz", "phonemes_per_s", "level_dbfs", "peak", "duration_s")},
            "sources_measured": [{k: r.get(k) for k in ("voice", "weight", "f0_median_hz", "f0_range_hz", "phonemes_per_s", "level_dbfs")} for r in per_source],
        }, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"  recipe -> {rec}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
