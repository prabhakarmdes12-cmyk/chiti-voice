#!/usr/bin/env python3
"""Fetch the model assets the offline synthesis spike runs against.

Why this script exists
----------------------
The usual home for a Kokoro/Piper voice is Hugging Face. Several environments this
project has to work in cannot reach it -- restricted CI runners, air-gapped build
boxes, and the sandbox the spike below was measured in (that one blocks
`huggingface.co`, `cdn-lfs.huggingface.co` and `objects.githubusercontent.com`, so
even `gh release download` fails). The npm registry is a plain package mirror and
`expo-kokoro` ships, inside its tarball:

  * `build/kokoro-quantized.onnx` -- the whole acoustic model *including* the
    vocoder, int8-quantized, one graph, 92,361,116 bytes;
  * `build/voices/*.bin` -- 54 style vectors, 522,240 bytes each, i.e. exactly
    510 rows x 256 float32;
  * `build/tokenizer.json` -- the 115-entry character->id table and the
    `model_max_length: 512` that bounds a synthesis unit.

So npm doubles as a model mirror. This downloads it, verifies it against the
pinned sha256, and extracts those three things. Nothing here is committed:
`models/` is gitignored, deliberately -- 88 MB of weights in git is how a repo
becomes unusable, and the pack format exists precisely so model bytes travel
beside a manifest instead of inside history.

Licence duties -- read before shipping a device image
-----------------------------------------------------
The npm package's own MIT licence covers its TypeScript. What it does *not*
settle is the licence of the weights (Kokoro-82M is a community model; its terms
are stated where the weights are published, not here) or the fact that the
phonemizer both this spike and Piper rely on, espeak-ng, is GPL-3.0-or-later and
ships its `espeak-ng-data` inside the piper wheel -- whose own `METADATA` in turn
says `License: GPL-3.0-or-later`. That is a copyleft boundary worth knowing about
before a proprietary app links a synthesiser, so `--accept-licence` is required to
write anything, and this script records what you accepted in `SOURCE.json`.

Usage
-----
    python3 scripts/fetch-offline-model.py                      # -> ./models
    python3 scripts/fetch-offline-model.py --dest ~/.local/share/chiti-voice
    python3 scripts/fetch-offline-model.py --verify-only

Then:
    python3 scripts/spike-kokoro-offline.py --text "Hello from an offline device."
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
import tarfile
import time
import urllib.request
from pathlib import Path

PACKAGE = "expo-kokoro"
VERSION = "1.1.9"
TARBALL_URL = f"https://registry.npmjs.org/{PACKAGE}/-/{PACKAGE}-{VERSION}.tgz"
# Pinned from the release this repo's measurements were taken against
# (see docs/research/KOKORO_OFFLINE_SPIKE.md). Update deliberately, with the
# measurements, not incidentally.
TARBALL_SHA256 = "d4a82900083abcb04903d933527aff37f37c6f01ef79871e5b45448ad07128af"
EXPECTED = {
    "package/build/kokoro-quantized.onnx": (92361116, "fbae9257e1e05ffc727e951ef9b9c98418e6d79f1c9b6b13bd59f5c9028a1478"),
    "package/build/tokenizer.json": (4119, None),
}
VOICE_ROW_LEN = 256
VOICE_MAX_UNITS = 510
VOICE_BYTES = VOICE_ROW_LEN * VOICE_MAX_UNITS * 4  # 522240


def sha256_of(path: Path, chunk: int = 1 << 20) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        while block := fh.read(chunk):
            h.update(block)
    return h.hexdigest()


def fetch(url: str, dest: Path) -> Path:
    print(f"GET {url}")
    with urllib.request.urlopen(url, timeout=300) as resp, dest.open("wb") as out:
        total = 0
        while chunk := resp.read(1 << 20):
            out.write(chunk)
            total += len(chunk)
            if total % (16 << 20) < (1 << 20):
                print(f"    ... {total / 1e6:.0f} MB", flush=True)
    print(f"    {total:,} bytes")
    return dest


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--dest", default="models", help="where to write (gitignored)")
    ap.add_argument("--url", default=TARBALL_URL, help="override only to pin a mirror")
    ap.add_argument("--sha256", default=TARBALL_SHA256, help="pinned tarball checksum")
    ap.add_argument("--voice", default="af_heart", help="which style vector to keep")
    ap.add_argument("--all-voices", action="store_true", help="extract all 54 (28 MB)")
    ap.add_argument("--verify-only", action="store_true", help="check what is on disk, write nothing")
    ap.add_argument("--accept-licence", action="store_true",
                    help="acknowledge the licence duties in this file's docstring")
    args = ap.parse_args()

    dest = Path(args.dest).expanduser()
    model_path = dest / "kokoro-quantized.onnx"
    tok_path = dest / "tokenizer.json"

    if args.verify_only:
        ok = True
        for label, p, want in (("model", model_path, EXPECTED["package/build/kokoro-quantized.onnx"][1]),
                               ("tokenizer", tok_path, None)):
            if not p.exists():
                print(f"  MISSING {label}: {p}")
                ok = False
            elif want:
                got = sha256_of(p)
                status = "OK" if got == want else f"MISMATCH (got {got[:12]}… want {want[:12]}…)"
                print(f"  {label:9s} {p} {status}")
                ok = ok and got == want
            else:
                print(f"  {label:9s} {p} present ({p.stat().st_size:,} bytes, no pinned hash)")
        return 0 if ok else 1

    if not args.accept_licence:
        sys.stderr.write(
            "refusing to write model bytes until the licence duties are acknowledged:\n"
            "  * weights: Kokoro-82M, terms stated where the weights are published -- check them\n"
            "  * phonemizer: espeak-ng is GPL-3.0-or-later, and its data files ship inside piper\n"
            "  * npm package code: MIT (covers its own TypeScript, nothing above)\n"
            "Re-run with --accept-licence once that is settled for your distribution.\n"
        )
        return 2

    dest.mkdir(parents=True, exist_ok=True)
    tmp = dest / f".{PACKAGE}-{VERSION}.tgz.part"
    if not tmp.exists():
        fetch(args.url, tmp)

    digest = sha256_of(tmp)
    if digest != args.sha256:
        tmp.unlink(missing_ok=True)
        sys.stderr.write(
            f"tarball sha256 {digest} does not match the pinned {args.sha256}.\n"
            "Either the upstream release was replaced, or you pointed --url somewhere unexpected.\n"
            "Re-measure and commit a new pin together with updated numbers in\n"
            "docs/research/KOKORO_OFFLINE_SPIKE.md -- do not pass a --sha256 override silently.\n"
        )
        return 1
    print(f"sha256 verified {digest}")

    want_voices = None if args.all_voices else {f"package/build/voices/{args.voice}.bin"}
    extracted: dict[str, dict[str, object]] = {}
    with tarfile.open(tmp, "r:gz") as tar:
        for member in tar.getmembers():
            if not member.isfile():
                continue
            name = member.name
            target: Path | None = None
            if name == "package/build/kokoro-quantized.onnx":
                target = model_path
            elif name == "package/build/tokenizer.json":
                target = tok_path
            elif name.startswith("package/build/voices/") and name.endswith(".bin") and (
                want_voices is None or name in want_voices
            ):
                # --all-voices sets want_voices to None; test that *before* membership, because
                # `name in None` is a TypeError and this branch is only reachable with the flag on.
                target = dest / "voices" / Path(name).name
            else:
                target = None
            if target is None:
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            with tar.extractfile(member) as src, target.open("wb") as out:
                shutil.copyfileobj(src, out, 1 << 20)
            extracted[target.name] = {"bytes": target.stat().st_size, "sha256": sha256_of(target)}
            print(f"  wrote {target} ({target.stat().st_size:,} bytes)")

    model_hash = extracted.get("kokoro-quantized.onnx", {}).get("sha256")
    want_model = EXPECTED["package/build/kokoro-quantized.onnx"]
    if (model_hash, ) != (want_model[1], ):
        sys.stderr.write(f"extracted model hash {model_hash} != expected {want_model[1]}\n")
        return 1
    for name, info in extracted.items():
        if name.endswith(".bin") and info["bytes"] != VOICE_BYTES:
            sys.stderr.write(f"{name}: {info['bytes']} bytes, expected {VOICE_BYTES} (510x256 f32)\n")
            return 1

    (dest / "SOURCE.json").write_text(json.dumps({
        "source": {"package": f"{PACKAGE}@{VERSION}", "url": args.url, "sha256": digest},
        "extracted_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "model_licence_duty": "verify Kokoro-82M terms; espeak-ng data is GPL-3.0-or-later",
        "assets": extracted,
        "layout": {"voices": f"{VOICE_MAX_UNITS} rows x {VOICE_ROW_LEN} float32",
                   "row_selected_by": "number of phoneme tokens, see spike script"},
    }, indent=2) + "\n", encoding="utf-8")
    print(f"\nprovenance -> {dest / 'SOURCE.json'}")
    print(f"next: python3 scripts/spike-kokoro-offline.py --model-dir {dest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
