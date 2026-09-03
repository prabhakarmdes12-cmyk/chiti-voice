#!/usr/bin/env python3
"""Generate the DSP parity fixtures the Rust engine is graded against.

    python3 scripts/make-dsp-parity-fixtures.py

Run this where the offline model exists (`scripts/fetch-offline-model.py --accept-licence`);
everything it writes into the repo is *data*, so CI needs neither model nor network to check the
Rust side.

Why these particular fixtures: `crates/vocal-core/src/audio_levels.rs` and `wav.rs` implement two
rules that decide whether the Rust engine can reproduce our reference audio at all —

  1. float -> int16 is `clamp(floor(x * 32767), -32768, 32767)` (the reference JS floors; Rust's
     `wav.rs` used to round, which is a permanent <= 1 LSB disagreement on every sample);
  2. loudness normalisation is `gain = min(target_linear / rms, ceiling / peak)`, accumulated in
     float64, with the peak ceiling allowed to win over the target.

So the strongest available test is not a synthetic vector: it is *the real float output of the
reference run*, converted, compared against the bytes already committed as evidence. If Rust
reproduces `assets/offline-spike/af_heart-en_us.wav` from the graph's own samples, the conversion
rule is right on real data, including its sign and near-zero behaviour.

Inputs are stored as float32 **bit patterns** (u32), not JSON numbers, so no decimal formatting
sits between what ONNX returned and what Rust decodes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

import voice_metrics as vm  # noqa: E402  the shared source of truth for these rules

OUT = ROOT / "crates" / "vocal-core" / "tests" / "fixtures" / "kokoro" / "dsp_parity.json"
REFERENCE_WAV = ROOT / "assets" / "offline-spike" / "af_heart-en_us.wav"
STYLE_DIM = 256  # one style row; voice_metrics.synth keeps the same default
TEXT = "This sentence was synthesised on a single board, with the network cable unplugged."
MAX_GAIN_DB = 12.0  # mirrored by vocal-core's `LoudnessSpec::MAX_GAIN_DB`
HEAD = 512  # samples per case: covers sign, scale, sub-LSB and boundary behaviour


def bits_of(wave: np.ndarray) -> list[int]:
    """float32 values as u32 bit patterns, so the fixture cannot lose precision in text."""
    return [int(b) for b in np.asarray(wave, dtype="<f4").reshape(-1).view(np.uint32)]


def to_pcm16(wave: np.ndarray) -> np.ndarray:
    """Rule 1, in the reference's own arithmetic: floor, then clamp."""
    scaled = np.floor(np.asarray(wave, dtype=np.float64).reshape(-1) * 32767.0)
    return np.clip(scaled, -32768.0, 32767.0).astype("<i2")


def loudness_gain(wave: np.ndarray, target_dbfs: float, ceiling: float) -> tuple[float, bool, float]:
    """Rule 2. Returns (gain, limited_by_ceiling, boundary_margin).

    `limited` is true when the peak ceiling rather than the loudness target set the gain — the case
    Bobo's cast hit for real, and the reason the ceiling exists. `boundary_margin` is how close the
    nearest scaled sample sits to an integer, i.e. how much float32-vs-float64 slack the fixture can
    absorb before `floor` could disagree; a small value means a brittle fixture worth flagging.
    """
    f = np.asarray(wave, dtype=np.float64).reshape(-1)
    if f.size == 0:
        return 1.0, False, 1.0
    rms = float(np.sqrt(np.mean(f * f)))
    peak = float(np.max(np.abs(f)))
    if rms == 0.0 or peak == 0.0:
        return 1.0, False, 1.0  # silence: nothing to scale, and no division by zero
    want = 10.0 ** (target_dbfs / 20.0)
    gain = min(want / rms, ceiling / peak)
    scaled = f * gain * 32767.0
    margin = float(np.min(np.abs(scaled - np.round(scaled))))
    return gain, gain < want / rms - 1e-9, margin


def pick_window(wave: np.ndarray, target: float, ceiling: float, size: int, min_margin: float):
    """Choose a sample window whose floor decisions cannot hinge on a last-ulp difference.

    numpy sums pairwise and Rust's loop sums in order, so a gain can differ by ~1 ulp; that only
    matters if a scaled sample sits close to an integer, where 1 ulp flips `floor`. Searching for a
    window with a comfortable margin keeps the fixture *exact* instead of teaching the Rust test to
    tolerate slop, which matters because this fixture's whole job is pinning a rounding rule.

    Two more filters, both learned the hard way. The first pass at this script happily accepted
    samples[2048:2560] and reported a gain of +147.94 dB, because that window is silence and
    "normalise silence to -21 dBFS" means "raise the noise floor by twelve orders of magnitude".
    So a window must carry real signal (`peak >= 0.05`) *and* the resulting gain must be one an
    engine would actually apply (no more than `MAX_GAIN_DB` of *amplification* — attenuation is
    always safe, which is also how `LoudnessSpec` is defined) — otherwise the fixture would pin a rule
    whose guard the runtime is supposed to refuse.
    """
    fallback = None
    for off in (0, 2048, 4096, 8192, 16384, 32768, 65536, 98304):
        seg = wave[off : off + size]
        if seg.size < size:
            continue
        gain, limited, margin = loudness_gain(seg, target, ceiling)
        if float(np.max(np.abs(seg))) < 0.05:
            continue  # silence: any gain here is amplifying a quantisation artefact
        if 20.0 * np.log10(max(gain, 1e-12)) > MAX_GAIN_DB:  # only amplification is capped
            continue  # outside what a sane engine may apply; see audio_levels.rs
        if margin >= min_margin:
            return off, seg, gain, limited, margin
        fallback = fallback or (off, seg, gain, limited, margin)
    return fallback


def normalise_to_pcm(wave: np.ndarray, target_dbfs: float, ceiling: float) -> np.ndarray:
    """The whole chain under test: gain, then the floor conversion."""
    gain, _, _ = loudness_gain(wave, target_dbfs, ceiling)
    return to_pcm16(np.asarray(wave, dtype=np.float64).reshape(-1) * gain)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--model-dir", default=str(ROOT / "models"))
    ap.add_argument("--out", default=str(OUT))
    ap.add_argument("--head", type=int, default=HEAD)
    ap.add_argument("--min-margin", type=float, default=1e-3,
                    help="fail if any case comes closer than this to a floor boundary")
    ap.add_argument("--dump-wave", default=None, help="also write the raw float32 output here (debug)")
    args = ap.parse_args()

    try:
        import onnxruntime as ort
        from piper.phonemize_espeak import EspeakPhonemizer
    except ImportError as e:  # noqa: BLE001
        sys.exit(f"missing dependency ({e}): pip install onnxruntime numpy piper-tts")

    model_dir = Path(args.model_dir)
    model = model_dir / "kokoro-quantized.onnx"
    if not model.exists():
        raise SystemExit(
            f"missing {model}\n"
            "  This generator needs the real graph, because its whole value is deriving fixtures\n"
            "  from actual ONNX output rather than from a formula's own definition.\n"
            "    python3 scripts/fetch-offline-model.py --accept-licence --dest models"
        )

    vocab, maxlen, pattern = vm.load_tokenizer(model_dir / "tokenizer.json")
    ph = EspeakPhonemizer()
    phonemes = " ".join("".join(s) for s in ph.phonemize("en-us", TEXT))
    ids = vm.encode_ids(phonemes, vocab, maxlen, pattern)
    n_tokens = max(len(ids) - 2, 0)

    so = ort.SessionOptions()
    so.intra_op_num_threads = 1
    so.inter_op_num_threads = 1
    sess = ort.InferenceSession(str(model), so, providers=["CPUExecutionProvider"])
    matrix = vm.load_style(model_dir / "voices" / "af_heart.bin")
    style = matrix[n_tokens * STYLE_DIM : (n_tokens + 1) * STYLE_DIM]
    # The raw float32 tensor the graph returned, before any scaling: that is the input Rust gets.
    wave = np.asarray(sess.run(None, {
        "input_ids": np.array([ids], dtype=np.int64),
        "style": np.array([style], dtype=np.float32),
        "speed": np.array([1.0], dtype=np.float32),
    })[0], dtype=np.float32).reshape(-1)
    head = wave[: args.head]
    print(f"graph output: {wave.size} samples, {n_tokens} content tokens, head {head.size}")

    ref_pcm = np.frombuffer(REFERENCE_WAV.read_bytes()[44:], dtype="<i2")
    if ref_pcm.size < head.size:
        raise SystemExit(f"{REFERENCE_WAV} holds {ref_pcm.size} samples, need {head.size}")
    expected = to_pcm16(head)
    if not np.array_equal(expected, ref_pcm[: head.size]):
        diffs = int(np.count_nonzero(expected != ref_pcm[: head.size]))
        worst = int(np.max(np.abs(expected.astype(np.int64) - ref_pcm[: head.size].astype(np.int64))))
        raise SystemExit(
            f"floor rule disagrees with the committed WAV on {diffs}/{head.size} samples (max {worst} LSB).\n"
            "  Either the reference stopped flooring or this audio came from another path; either way,\n"
            "  writing these fixtures would teach the Rust engine a wrong rule. Not writing them."
        )
    print(f"  ✓ clamp(floor(x*32767)) reproduces {REFERENCE_WAV.name} exactly on {head.size} samples")

    if args.dump_wave:
        Path(args.dump_wave).write_bytes(head.astype("<f4").tobytes())
        print(f"  wrote {args.dump_wave}")

    cases = [
        {
            "name": "graph_output_head",
            "note": "the first 512 float32 samples the graph produced for the reference run, verbatim",
            "input_bits": bits_of(head),
            "expected_i16": [int(v) for v in expected],
        },
        {
            "name": "edge_values",
            "note": "sign, exactly one LSB, sub-LSB (must floor to zero), full scale, out of range",
            "input_bits": None,  # filled below, from the same array we convert
        },
    ]
    edge = np.array([0.0, 0.5, 1.0 / 32767.0, 1.0, -1.0, -0.5, 3.0e-8, -3.0e-8,
                     0.99999994, 8.0, -8.0, -1.0000001], dtype=np.float32)
    cases[1]["input_bits"] = bits_of(edge)
    cases[1]["expected_i16"] = [int(v) for v in to_pcm16(edge)]

    loud_cases = []
    for target, ceiling in ((-21.0, 0.98), (-17.5, 0.98), (0.0, 0.95), (-30.0, 0.98)):
        picked = pick_window(wave, target, ceiling, args.head, args.min_margin)
        if picked is None:
            raise SystemExit("graph output is shorter than one window; raise nothing, fix the script")
        off, seg, gain, limited, margin = picked
        if margin < args.min_margin:
            raise SystemExit(
                f"no window for {target:+.1f} dBFS clears the {args.min_margin:g} floor-boundary margin\n"
                "  (best was {margin:.2e}); widen --min-margin only with a reason, or use another sentence."
            )
        loud_cases.append({
            "name": f"target_{abs(target):g}dbfs_ceiling_{ceiling:g}",
            "target_dbfs": target,
            "peak_ceiling": ceiling,
            "head_offset": off,
            "boundary_margin": margin,
            "input_bits": bits_of(seg),
            "expected_i16": [int(v) for v in normalise_to_pcm(seg, target, ceiling)],
            "expected_limited": limited,
        })
        print(f"  loudness {target:+.1f} dBFS, ceiling {ceiling}: samples[{off}:{off + seg.size}], "
              f"gain {20 * np.log10(gain):+.2f} dB, limited_by_ceiling={limited}, margin={margin:.4f}")
    loud_cases.append({
        "name": "silence_is_left_alone",
        "note": "RMS 0 must not divide; the guard returns the input untouched, so does Rust",
        "target_dbfs": -20.0,
        "peak_ceiling": 0.98,
        "input_bits": bits_of(np.zeros(64, dtype=np.float32)),
        "expected_i16": [0] * 64,
        "expected_limited": False,
        "boundary_margin": 1.0,
    })

    doc = {
        "kind": "parity fixtures for vocal-core's DSP rules, derived from real inference output",
        "generated_by": "scripts/make-dsp-parity-fixtures.py",
        "read_this_as": "float32 values are u32 bit patterns; expected PCM is exact, so every Rust assertion is equality, never a tolerance.",
        "provenance": {
            "model": {"file": model.name, "bytes": model.stat().st_size,
                      "sha256": hashlib.sha256(model.read_bytes()).hexdigest()},
            "voice": {"id": "af_heart", "bytes": int(matrix.size * 4),
                      "sha256": hashlib.sha256((model_dir / "voices" / "af_heart.bin").read_bytes()).hexdigest()},
            "text": TEXT,
            "espeak_voice": "en-us",
            "content_tokens": n_tokens,
            "style_row_used": n_tokens,
            "speed": 1.0,
            "reference_wav": {"file": str(REFERENCE_WAV.relative_to(ROOT)),
                              "sha256": hashlib.sha256(REFERENCE_WAV.read_bytes()).hexdigest()},
        },
        "pcm16_cases": cases,
        "loudness_cases": loud_cases,
    }
    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(doc, indent=1) + "\n", encoding="utf-8")
    print(f"wrote {out_path.relative_to(ROOT)} ({out_path.stat().st_size} B)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
