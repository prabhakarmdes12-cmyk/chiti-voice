#!/usr/bin/env python3
"""Measure the offline synthesis path and emit the fixture the Rust engine is tested against.

This is the procedure whose numbers are recorded in
`docs/research/KOKORO_OFFLINE_SPIKE.md`, written down so that anyone can re-measure
them rather than trust them. It does three things:

1. **Synthesises** audio through the graph, with a tokenisation path that mirrors the
   reference implementation exactly (normalise with `tokenizer.json`'s whitelist regex,
   one id per Unicode codepoint, `$` wrapped, truncate to `model_max_length`, pick the
   style row at `n_tokens * 256` out of the voice `.bin`, `float * 32767` floored to
   int16 at 24 kHz). Those choices came from reading `build/kokoro.js` + `build/wav.js`
   of `expo-kokoro@1.1.9` and matching them against a real model -- not from guesswork,
   which is why this file exists.
2. **Runs the falsification controls** (`--controls`): `speed` must stretch duration
   roughly 1/speed, a zeroed style vector must change the output, and pad-only input
   must collapse the duration. A pipeline that passes those is wired to the model
   rather than merely producing sound; RMS by itself cannot tell you that.
3. **Emits the fixture** (`--emit-fixture`) -- phonemes, ids, chosen style row, sample
   count, RMS -- which is what `crates/vocal-core/tests/kokoro_reference.rs` compares
   the Rust engine against.

Phonemisation is deliberately pluggable, because it is the one part with a licence
consequence (espeak-ng is GPL-3.0-or-later). If no phonemiser is available, pass
`--phonemes` with a pre-computed IPA string; the fixture pins the phoneme string, so
the Rust test depends on the *table*, not on a phonemiser being installed.

    pip install onnxruntime numpy            # and: pip install piper-tts  (for espeak-ng)
    python3 scripts/fetch-offline-model.py --accept-licence
    python3 scripts/spike-kokoro-offline.py --model-dir models --controls \
        --text "This sentence was synthesised on a single board, with the network cable unplugged."
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
import wave
from pathlib import Path

SAMPLE_RATE = 24000
STYLE_DIM = 256
MAX_UNITS = 510
PAD = 0  # resolved again from tokenizer.json when present


def load_tokenizer(model_dir: Path) -> tuple[dict[str, int], int, re.Pattern[str]]:
    for cand in (model_dir / "tokenizer.json", Path(__file__).parent.parent / "crates/vocal-core/tests/fixtures/kokoro/tokenizer.json"):
        if cand.exists():
            tok = json.loads(cand.read_text(encoding="utf-8"))
            vocab = tok["model"]["vocab"]
            maxlen = int(tok["config"]["model_max_length"])
            pat = re.compile(tok["normalizer"]["pattern"]["Regex"])
            return vocab, maxlen, pat
    sys.exit(f"tokenizer.json not found next to the model -- run scripts/fetch-offline-model.py")


def encode(text: str, vocab: dict[str, int], maxlen: int, pattern: re.Pattern[str]) -> list[int]:
    pad = vocab.get("$", PAD)
    ids = [vocab.get(c, pad) for c in pattern.sub("", text)]
    return ([pad] + ids + [pad])[:maxlen]


def phonemize_espeak(voice: str, text: str, data_dir: str | None) -> str:
    """espeak-ng -> IPA phonemes, via the piper wheel's binding, else its CLI."""
    try:
        sys.path.insert(0, data_dir or "")
        from piper.phonemize_espeak import EspeakPhonemizer  # type: ignore

        ph = EspeakPhonemizer(Path(data_dir) / "piper/espeak-ng-data") if data_dir else EspeakPhonemizer()
        return " ".join("".join(s) for s in ph.phonemize(voice, text))
    except Exception:  # noqa: BLE001 - deliberate: the CLI fallback is equivalent
        pass
    try:
        out = subprocess.run(["espeak-ng", "-v", voice, "--ipa", "-q", text],
                             capture_output=True, text=True, check=True).stdout
        return out.strip()
    except Exception:  # noqa: BLE001
        sys.exit("no phonemiser available: `pip install piper-tts`, or install espeak-ng, "
                 "or pass --phonemes explicitly")


def synth(session, inputs: dict, ) -> "object":
    return session.run(None, inputs)[0]


def phonemize_open(text: str, phonemizer_dir: Path, ort) -> str:
    """Lexicon-first, graph-for-OOV English phonemiser with no GPL component.

    Mirrors scripts/extract-open-phonemizer.py's decode rules exactly: the graph emits
    `char_repeats` slots per input character and uses `_` as blank, so the raw argmax string
    looks like `tt_ʃʃʃˈaː__ɾ__i`. Dropping blanks, collapsing consecutive duplicates and
    dropping spaces yields `tʃaɾi`. Skipping the collapse produces a stuttered phoneme
    string that still synthesises -- and still sounds almost right -- which is why it is
    spelled out rather than left to the reader.
    """
    table = json.loads((phonemizer_dir / "phonemizer_tokenizer.json").read_text(encoding="utf-8"))
    lexicon = json.loads((phonemizer_dir / "lexicon_en_us.json").read_text(encoding="utf-8"))
    text_sym = table["text_symbols"]
    phon_sym = {int(k): v for k, v in table["phoneme_symbols"].items()}
    reps, maxlen = int(table["char_repeats"]), int(table["max_length"])
    blank, end = table["decode_rules"]["pad_symbol"], table["decode_rules"]["end_symbol"]
    session = ort.InferenceSession(str(phonemizer_dir / "open-phonemizer.onnx"), providers=["CPUExecutionProvider"])
    vocab = len(phon_sym)

    def encode_word(word: str) -> list[int]:
        ids = [text_sym["<en_us>"]]
        for ch in word.lower().replace(" ", "_").strip():
            code = text_sym.get(ch)
            if code is not None:
                ids += [code] * reps
        ids.append(text_sym["<end>"])
        return (ids + [0] * maxlen)[:maxlen]

    def g2p(word: str) -> str:
        logits = session.run(None, {"text": __import__("numpy").array([encode_word(word)], dtype="int64")})[0]
        logits = __import__("numpy").asarray(logits).reshape(-1, vocab)
        out: list[str] = []
        for idx in (int(i) for i in logits.argmax(axis=1)):
            sym = phon_sym.get(idx, "")
            if sym in (end, "", blank):
                if sym == end:
                    break
                continue
            if sym.startswith("<") and sym.endswith(">"):
                continue
            if not out or out[-1] != sym:
                out.append(sym)
        return "".join(c for c in out if c != " ")

    parts = []
    for token in re.findall(r"\w+|[^\w\s]", text):
        if re.fullmatch(r"\w+", token):
            parts.append(lexicon.get(token.lower()) or g2p(token.lower()))
        else:
            parts.append(token)
    joined = " ".join(p for p in parts if p)
    joined = re.sub(r"\s+([.,!?;:])", r"\1", joined)
    return re.sub(r"\s+([\]\)\}»”’])", r"\1", joined)

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--model-dir", default="models")
    ap.add_argument("--text", default="This sentence was synthesised on a single board, with the network cable unplugged.")
    ap.add_argument("--phonemes", default=None, help="pre-computed IPA; skips the phonemiser")
    ap.add_argument("--espeak-voice", default="en-us")
    ap.add_argument("--phonemizer", choices=("espeak", "open"), default="espeak",
                    help="espeak = piper wheel / espeak-ng CLI (GPL-3 data); open = lexicon + ONNX G2P, no GPL component")
    ap.add_argument("--phonemizer-dir", default="models/phonemizer")
    ap.add_argument("--voice", default="af_heart")
    ap.add_argument("--speed", type=float, default=1.0)
    ap.add_argument("--threads", type=int, default=1, help="1 mimics a single Cortex-A72/A76 core")
    ap.add_argument("--out", default="/tmp/offline-spike.wav")
    ap.add_argument("--piper-data", default=None, help="dir containing piper/espeak-ng-data (from the wheel)")
    ap.add_argument("--controls", action="store_true")
    ap.add_argument("--report-phonemes-only", action="store_true", help="print the phoneme string and exit (fixture debugging)")
    ap.add_argument("--emit-fixture", default=None, help="write reference.json here")
    args = ap.parse_args()

    try:
        import numpy as np
        import onnxruntime as ort
    except ImportError:
        sys.exit("missing deps: pip install onnxruntime numpy")

    model_dir = Path(args.model_dir)
    model = model_dir / "kokoro-quantized.onnx"
    vbin = model_dir / "voices" / f"{args.voice}.bin"
    if not model.exists() or not vbin.exists():
        sys.exit(f"{model} / {vbin} missing -- run scripts/fetch-offline-model.py --accept-licence first")

    vocab, maxlen, pattern = load_tokenizer(model_dir)
    if args.phonemes:
        phonemes = args.phonemes
    elif args.phonemizer == "open":
        phonemes = phonemize_open(args.text, Path(args.phonemizer_dir), ort)
    else:
        phonemes = phonemize_espeak(args.espeak_voice, args.text, args.piper_data)
    ids = encode(phonemes, vocab, maxlen, pattern)
    n_tokens = min(max(len(ids) - 2, 0), MAX_UNITS - 1)
    if args.report_phonemes_only:
        print(f"phonemes: {phonemes}\ntokens:   {len(ids)} (style row {n_tokens})")
        return 0

    so = ort.SessionOptions()
    so.intra_op_num_threads = args.threads
    so.inter_op_num_threads = args.threads
    t_load0 = time.perf_counter()
    session = ort.InferenceSession(str(model), so, providers=["CPUExecutionProvider"])
    t_load = time.perf_counter() - t_load0

    style_all = np.fromfile(vbin, dtype=np.float32)
    assert style_all.size == MAX_UNITS * STYLE_DIM, f"{vbin} has {style_all.size} floats, expected {MAX_UNITS * STYLE_DIM}"

    def run(speed: float, style: "np.ndarray | None" = None, token_ids: list[int] | None = None):
        st = style_all[n_tokens * STYLE_DIM:(n_tokens + 1) * STYLE_DIM] if style is None else style
        t0 = time.perf_counter()
        wave_f = np.asarray(synth(session, {
            "input_ids": np.array([token_ids if token_ids is not None else ids], dtype=np.int64),
            "style": np.array([st if style is None else st], dtype=np.float32),
            "speed": np.array([speed], dtype=np.float32),
        })).reshape(-1)
        dt = time.perf_counter() - t0
        pcm = np.clip(np.floor(wave_f * 32767.0), -32768, 32767).astype("<i2")
        return dt, pcm

    dt, pcm = run(args.speed)
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(out), "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(SAMPLE_RATE)
        w.writeframes(pcm.tobytes())

    dur = len(pcm) / SAMPLE_RATE
    rms = float(np.sqrt(np.mean((pcm / 32768.0) ** 2)))
    peak = float(np.max(np.abs(pcm)) / 32768.0)
    rss = resource_kb()
    print(f"model         {model.name}  {model.stat().st_size / 1048576:.1f} MiB  load {t_load:.2f} s")
    print(f"threads       {args.threads}   (nproc={os.cpu_count()})")
    print(f"phonemes      {phonemes[:100]}{'…' if len(phonemes) > 100 else ''}")
    print(f"tokens        {len(ids)}   style_row {n_tokens}   voice {args.voice}")
    print(f"audio         {dur:.2f} s   {len(pcm)} samples   rms {rms:.4f}   peak {peak:.3f}")
    print(f"inference     {dt:.2f} s   RTF {dt / max(dur, 1e-6):.2f}")
    print(f"process peak  {rss} MiB (python + numpy + onnxruntime; a Rust/ort build is not this)")
    print(f"wrote         {out}")

    if args.controls:
        print("\ncontrols (a pipeline that passes these is wired to the graph, not just making sound):")
        slow_dt, slow = run(0.5)
        fast_dt, fast = run(2.0)
        zero_dt, zero = run(args.speed, style=np.zeros(STYLE_DIM, dtype=np.float32))
        pad_dt, pad_ids = run(args.speed, token_ids=[vocab["$"]] * 16)
        checks = [
            ("speed=0.5 stretches", len(slow) > int(1.6 * len(pcm))),
            ("speed=2.0 compresses", len(fast) < int(0.85 * len(pcm))),
            ("zero style differs", float(np.sqrt(np.mean((zero / 32768.0) ** 2))) > 0 or len(zero) != len(pcm)),
            ("pad-only collapses", len(pad_ids) < int(0.6 * len(pcm))),
            ("not silent", rms > 0.01),
        ]
        for label, ok in checks:
            print(f"  [{'PASS' if ok else 'FAIL'}] {label}")
        if not all(ok for _, ok in checks):
            return 1

    if args.emit_fixture:
        fixture = {
            "description": "Reference output of the offline synthesis path; regenerable with scripts/spike-kokoro-offline.py. Not a licence grant -- see docs/research/KOKORO_OFFLINE_SPIKE.md.",
            "engine": {"model_file": model.name, "model_bytes": model.stat().st_size,
                       "model_sha256": sha256(model),
                       "source": "npm expo-kokoro@1.1.9 build/ (extracted by scripts/fetch-offline-model.py)",
                       "sample_rate": SAMPLE_RATE, "style_dim": STYLE_DIM, "max_phoneme_units": MAX_UNITS,
                       "inputs": {"input_ids": "int64 [1, len]", "style": "float32 [1, 256]", "speed": "float32 [1]"},
                       "output": "waveform float32 [1, n] -> floor(x * 32767) -> int16 PCM"},
            "voice": {"id": args.voice, "bytes": vbin.stat().st_size, "sha256": sha256(vbin)},
            "request": {"text": args.text, "espeak_voice": args.espeak_voice,
                        "phonemes": phonemes, "input_ids": ids, "n_tokens": n_tokens,
                        "style_row": n_tokens, "speed": args.speed},
            "expected": {"samples": len(pcm), "duration_s": round(dur, 4), "rms": round(rms, 4),
                         "peak": round(peak, 4)},
            "measurement_environment": {"intra_op_threads": args.threads, "nproc": os.cpu_count(),
                                        "onnxruntime": __import__("onnxruntime").__version__,
                                        "inference_s": round(dt, 3)},
            "tolerances_for_the_rust_engine": {
                "samples_relative": 0.02,
                "rms_relative": 0.40,
                "why_not_bit_exact": "quantised weights + different ONNX Runtime builds/versions/threads do not "
                                     "reproduce float32 accumulation order; sample-exact equality would assert a "
                                     "property of one runtime, not of the engine",
            },
        }
        Path(args.emit_fixture).write_text(json.dumps(fixture, indent=2) + "\n", encoding="utf-8")
        print(f"\nfixture -> {args.emit_fixture}")
    return 0


def sha256(p: Path) -> str:
    import hashlib
    h = hashlib.sha256()
    with p.open("rb") as fh:
        while block := fh.read(1 << 20):
            h.update(block)
    return h.hexdigest()


def resource_kb() -> float:
    try:
        import resource
        return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024.0
    except Exception:  # noqa: BLE001
        return -1.0


if __name__ == "__main__":
    raise SystemExit(main())
