#!/usr/bin/env python3
"""Measure every stock voice the model ships, so persona casting is a decision with evidence.

`docs/personas/*.md` describe Tara, Kashi and Bobo in adjectives — "warm", "lower register",
"exaggerated pitch movement", "never rushed". Adjectives cannot be cast against. This turns each
voice into the closest measurable proxies for those words, so the choice is arguable:

    "lower register"      -> f0_median_hz
    "exaggerated movement"-> f0_range_hz (p95 - p5 of voiced F0)
    "never rushed"         -> phonemes_per_s
    "not robotic"          -> voiced_ratio and f0_range (a monotone has neither)
    clipping hazard        -> level_dbfs / peak

Two tables, because cross-language pace/pitch comparisons are meaningless: every voice is
measured on one English sentence (comparable across all of them), and the Hindi voices are
measured again on a Hindi sentence, which is what KASHI would actually speak.

The sentence deliberately contains a number and a currency, because TARA.md promises "handles
numbers, currency, and dates naturally" — that is the thing to look at, not the vowel quality.

    /tmp/pv/bin/python scripts/survey-persona-voices.py --model-dir models --emit /tmp/survey.json

`--only af_heart,am_adam` narrows a re-run; `--lang hi` restricts to the Hindi pass.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import voice_metrics as vm  # noqa: E402

# Kokoro's voice ids are `<language letter><gender letter>_<name>`: a/b = American/British
# English, h = Hindi, e = Spanish, f = French, i = Italian, j = Japanese, p = Portuguese,
# z = Mandarin. Only `en-us` and `hi` are exercised here; the rest are measured on English
# phonemes so that all 54 rows stay comparable.
ESPEAK_BY_PREFIX = {"a": "en-us", "b": "en-us", "h": "hi", "e": "es", "f": "fr",
                    "i": "it", "j": "ja", "p": "pt", "z": "cmn"}
HINDI_PREFIXES = ("h",)


def phonemize(ph, espeak_voice: str, text: str) -> str:
    return " ".join("".join(s) for s in ph.phonemize(espeak_voice, text))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--model-dir", default="models")
    ap.add_argument("--sentence", default="Your total is 240 rupees, and the driver will arrive on Friday.")
    ap.add_argument("--hindi-sentence", default="आपका ड्राइवर शुक्रवार को आ रहा है।")
    ap.add_argument("--lang", choices=("all", "en", "hi"), default="all")
    ap.add_argument("--threads", type=int, default=1)
    ap.add_argument("--only", default=None, help="comma-separated voice ids")
    ap.add_argument("--emit", default=None, help="write the full result JSON here")
    args = ap.parse_args()

    try:
        import numpy as np
        import onnxruntime as ort
        from piper.phonemize_espeak import EspeakPhonemizer
    except ImportError as e:  # noqa: BLE001
        sys.exit(f"missing dependency ({e}): pip install onnxruntime numpy piper-tts")

    model_dir = Path(args.model_dir)
    vocab, maxlen, pattern = vm.load_tokenizer(model_dir / "tokenizer.json")
    so = ort.SessionOptions()
    so.intra_op_num_threads = args.threads
    so.inter_op_num_threads = args.threads
    sess = ort.InferenceSession(str(model_dir / "kokoro-quantized.onnx"), so, providers=["CPUExecutionProvider"])
    ph = EspeakPhonemizer()

    voices = sorted(p.stem for p in (model_dir / "voices").glob("*.bin"))
    if args.only:
        wanted = {v.strip() for v in args.only.split(",")}
        voices = [v for v in voices if v in wanted]
    passes = [("en", args.sentence, "en-us")]
    if args.lang in ("all", "hi"):
        passes.append(("hi", args.hindi_sentence, "hi"))
    if args.lang == "en":
        passes = passes[:1]

    rows: list[dict] = []
    for label, sentence, espeak_voice in passes:
        print(f"\n═══ pass: {label} — “{sentence[:52]}…” ({espeak_voice}) ═══", flush=True)
        header = ["voice", "f0 Hz", "range Hz", "phon/s", "voiced", "dBFS", "peak", "dur s", "infer s"]
        print(vm.fmt_row(header))
        print(vm.fmt_row(["---"] * len(header)))
        for name in voices:
            if label == "hi" and name[:1] not in HINDI_PREFIXES:
                continue
            try:
                phonemes = phonemize(ph, espeak_voice, sentence)
                ids = vm.encode_ids(phonemes, vocab, maxlen, pattern)
                import time
                t0 = time.perf_counter()
                matrix = vm.load_style(model_dir / "voices" / f"{name}.bin")
                pcm = vm.synth(sess, phonemes, matrix, ids, 1.0)
                infer = time.perf_counter() - t0
            except Exception as e:  # noqa: BLE001 - one odd voice must not kill the sweep
                print(f"  {name}: {type(e).__name__}: {str(e)[:80]}")
                continue
            m = vm.measure(pcm, max(len(ids) - 2, 0))
            m.update({"voice": name, "lang": label, "espeak_voice": espeak_voice,
                      "infer_s": round(infer, 2), "lang_family": ESPEAK_BY_PREFIX.get(name[:1], "?")})
            rows.append(m)
            print(vm.fmt_row([name, vm.fmt(m["f0_median_hz"]), vm.fmt(m["f0_range_hz"]),
                              vm.fmt(m["phonemes_per_s"], ".2f"), vm.fmt(m["voiced_ratio"], ".2f"),
                              vm.fmt(m["level_dbfs"]), vm.fmt(m["peak"], ".3f"),
                              vm.fmt(m["duration_s"], ".2f"), vm.fmt(m["infer_s"], ".1f")]), flush=True)

    en = [r for r in rows if r["lang"] == "en" and r["f0_median_hz"]]
    if en:
        lo = min(en, key=lambda r: r["f0_median_hz"])
        hi = max(en, key=lambda r: r["f0_median_hz"])
        wide = max(en, key=lambda r: r["f0_range_hz"] or 0)
        slow = min(en, key=lambda r: r["phonemes_per_s"] or 9e9)
        fast = max(en, key=lambda r: r["phonemes_per_s"] or 0)
        print("\n═══ casting-relevant extremes (English pass) ═══")
        print(f"  lowest register   {lo['voice']:12s} {lo['f0_median_hz']} Hz   (KASHI wants this end)")
        print(f"  highest register  {hi['voice']:12s} {hi['f0_median_hz']} Hz")
        print(f"  widest pitch move {wide['voice']:12s} {wide['f0_range_hz']} Hz   (BOBO wants this end)")
        print(f"  slowest delivery  {slow['voice']:12s} {slow['phonemes_per_s']} phonemes/s")
        print(f"  fastest delivery  {fast['voice']:12s} {fast['phonemes_per_s']} phonemes/s")
        hot = [r["voice"] for r in en if (r["peak"] or 0) > 0.9]
        if hot:
            print(f"  peaks above 0.9 (would clip on a device at max volume): {', '.join(hot)}")

    if args.emit:
        Path(args.emit).write_text(json.dumps({"sentence_en": args.sentence,
                                               "sentence_hi": args.hindi_sentence,
                                               "threads": args.threads, "rows": rows}, indent=2) + "\n",
                                   encoding="utf-8")
        print(f"\nfull data -> {args.emit}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
