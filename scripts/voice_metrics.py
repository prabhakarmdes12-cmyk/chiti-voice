#!/usr/bin/env python3
"""Shared measurement code for the persona survey and the style-vector derivation.

Kept in one place on purpose: a "measured persona" claim is only worth as much as the identity
between the two scripts' metrics. Both import this module, so `survey-persona-voices.py` and
`derive-persona-style.py` cannot drift into reporting different numbers for the same audio.

What is measured, and what it is *not*:

* ``f0_median_hz`` — median fundamental period estimated per frame by autocorrelation, over
  frames that are loud enough and periodic enough to be voiced. A proxy for register ("lower
  register" in the persona docs), not a pitch track from a DSP library.
* ``f0_range_hz`` — p95 minus p5 of those estimates: *pitch movement*. Bobo's spec asks for
  exaggerated movement and Tara's for "not over-formal", which are the two ends of this number.
* ``phonemes_per_s`` — token count divided by audio duration. Not syllables per second: Kokoro
  is driven by phonemes, and calling it "pace" without saying so would be the kind of small
  dishonesty that turns into a wrong product decision.
* ``level_dbfs`` / ``peak`` — because the spike measured the same sentence landing between 0.50
  and 0.99 peak depending on voice, which is a clipping hazard on a device, not a taste question.
* ``voiced_ratio`` — share of frames with a usable period; low values mean the estimate above is
  thin on evidence, so it is reported rather than hidden.
"""

from __future__ import annotations

import math
import re
from typing import Any

import numpy as np

SAMPLE_RATE = 24000
FRAME = 1024
HOP = 256
F0_LO_HZ, F0_HI_HZ = 70.0, 400.0
FRAME_ENERGY_FLOOR = 0.0025   # ~ -52 dBFS: below this the frame is silence, not quiet voice
AC_RATIO_MIN = 0.30           # periodicity gate; below it the "pitch" is noise dressed as a pitch


def load_tokenizer(tokenizer_json_path) -> tuple[dict[str, int], int, "re.Pattern[str]"]:
    import json
    from pathlib import Path

    tok = json.loads(Path(tokenizer_json_path).read_text(encoding="utf-8"))
    vocab = tok["model"]["vocab"]
    maxlen = int(tok["config"]["model_max_length"])
    pattern = re.compile(tok["normalizer"]["pattern"]["Regex"])
    return vocab, maxlen, pattern


def encode_ids(phonemes: str, vocab: dict[str, int], maxlen: int, pattern: "re.Pattern[str]") -> list[int]:
    """The reference implementation's tokenisation, in one place.

    `$ seq $`, where `$` (id 0) is both pad and wrap, applied *before* truncation — so an
    over-long utterance loses its trailing `$` rather than its leading one. Reproducing this
    exactly is what `crates/vocal-core/tests/kokoro_tokens.rs` checks on the Rust side.
    """
    pad = vocab.get("$", 0)
    kept = [c for c in pattern.sub("", phonemes)]
    ids = [pad] + [vocab[c] for c in kept if c in vocab] + [pad]
    return ids[:maxlen]


def pcm_from_waveform(waveform: np.ndarray) -> np.ndarray:
    """float [-1, 1] -> int16, exactly as the reference export's player does it.

    The upstream JS floors; `crates/vocal-core/src/wav.rs` rounds. The difference is under one
    LSB and is not worth an argument, but it *is* worth stating, because a test that claims
    bit-exactness against a JS reference would then be false.
    """
    return np.clip(np.floor(np.asarray(waveform).reshape(-1) * 32767.0), -32768, 32767).astype("<i2")


def synth(sess, phonemes: str, voice_matrix: np.ndarray, ids: list[int], speed: float,
          style_dim: int = 256, max_units: int = 510) -> np.ndarray:
    n = min(max(len(ids) - 2, 0), max_units - 1)
    style = voice_matrix[n * style_dim:(n + 1) * style_dim]
    out = sess.run(None, {
        "input_ids": np.array([ids], dtype=np.int64),
        "style": np.array([style], dtype=np.float32),
        "speed": np.array([speed], dtype=np.float32),
    })[0]
    return pcm_from_waveform(out)


def f0_track(pcm: np.ndarray) -> tuple[list[float], float]:
    """Frame-wise F0 by autocorrelation. Returns (voiced estimates, voiced ratio)."""
    f = pcm.astype(np.float64) / 32768.0
    if len(f) < FRAME:
        return [], 0.0
    n_frames = 1 + (len(f) - FRAME) // HOP
    frames = np.lib.stride_tricks.as_strided(
        f, shape=(n_frames, FRAME), strides=(f.strides[0] * HOP, f.strides[0])
    ).copy()
    frames -= frames.mean(axis=1, keepdims=True)
    energy = np.sqrt((frames ** 2).mean(axis=1))
    nfft = 1 << (2 * FRAME - 1).bit_length()
    spec = np.fft.rfft(frames, n=nfft)
    ac = np.fft.irfft(spec * np.conj(spec), n=nfft)[:, :FRAME]
    ac = np.where(ac[:, :1] == 0, 1.0, ac / np.maximum(ac[:, :1], 1e-12))
    lo = max(2, int(round(SAMPLE_RATE / F0_HI_HZ)))
    hi = min(FRAME - 1, int(round(SAMPLE_RATE / F0_LO_HZ)))
    band = ac[:, lo:hi]
    best = band.argmax(axis=1) + lo
    strength = band.max(axis=1)
    out: list[float] = []
    voiced = 0
    for i in range(n_frames):
        if energy[i] < FRAME_ENERGY_FLOOR or strength[i] < AC_RATIO_MIN:
            continue
        voiced += 1
        out.append(SAMPLE_RATE / float(best[i]))
    return out, (voiced / n_frames if n_frames else 0.0)


def measure(pcm: np.ndarray, n_tokens: int) -> dict[str, Any]:
    dur = len(pcm) / SAMPLE_RATE
    f = pcm.astype(np.float64) / 32768.0
    track, ratio = f0_track(pcm)
    rms = float(np.sqrt(np.mean(f * f)))
    return {
        "duration_s": round(dur, 3),
        "samples": int(len(pcm)),
        "n_tokens": int(n_tokens),
        "f0_median_hz": round(float(np.median(track)), 1) if track else None,
        "f0_range_hz": round(float(np.percentile(track, 95) - np.percentile(track, 5)), 1) if len(track) > 8 else None,
        "voiced_ratio": round(ratio, 3),
        "phonemes_per_s": round(n_tokens / dur, 2) if dur else None,
        "rms": round(rms, 4),
        "level_dbfs": round(20 * math.log10(rms), 1) if rms > 0 else None,
        "peak": round(float(np.max(np.abs(f))), 3),
    }


def fmt(v: Any, spec: str = ".1f") -> str:
    return "-" if v is None else format(v, spec)


def fmt_row(cells: list[Any]) -> str:
    return "| " + " | ".join(str(c) for c in cells) + " |"


def load_style(path) -> np.ndarray:
    """Read a voice as the full 510 x 256 matrix, not just the row a sentence happens to use.

    Blending must be done across every row: a voice is usable at any utterance length, and a
    blend of only one row would be a voice that changes character the moment a sentence is split
    differently by the caller.
    """
    from pathlib import Path

    m = np.fromfile(Path(path), dtype=np.float32)
    if m.size != 510 * 256:
        raise ValueError(f"{path}: {m.size} floats, expected {510 * 256} (510 rows x 256)")
    return m
