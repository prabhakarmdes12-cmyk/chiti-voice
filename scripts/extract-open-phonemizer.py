#!/usr/bin/env python3
"""Extract a licence-clean English phonemiser, so the whole synthesis path can avoid GPL-3.

Why this is worth a script
--------------------------
Every mainstream offline TTS path gets phonemes from espeak-ng, which is
GPL-3.0-or-later -- and piper's own Python wheel says `License: GPL-3.0-or-later` in its
metadata precisely because it bundles espeak-ng's data. If this project is to be linked
from an app that is not GPL, the phonemiser is the decision to make early; the ONNX graph
and the voice vectors are not the problem.

`expo-open-phonemizer@1.0.1` (MIT, npm) is the alternative: a 274,927-entry American
English lexicon **plus** a small character-level ONNX graph for out-of-vocabulary words.
Its tarball carries both, but as a `dictionary.ts` / `tokenizer.ts` literal that only
TypeScript can consume, so this script converts them into canonical JSON that Rust can
mmap/serde straight in:

    <dest>/open-phonemizer.onnx         61,553,088 bytes  (input `text` i64[1,64] -> `logits` f32[1,64,64])
    <dest>/phonemizer_tokenizer.json    55 text symbols, 64 phoneme symbols, char_repeats=3
    <dest>/lexicon_en_us.json           ~14 MB word -> IPA map

Two things this script deliberately records instead of hiding
-------------------------------------------------------------
1. **The upstream JS is broken for OOV words and must not be copied.** Its `_phonemize`
   reads `results["output"]`, but the graph's only output is named `logits`, so any word
   missing from the lexicon throws. `--check-names` verifies the graph's real I/O.
2. **The graph emits `char_repeats` slots per input character and uses `_` as blank.** A
   naive decode therefore returns `tt_ʃʃʃˈaː__ɾ__i` for "chiti". The correct sequence is:
   drop `<pad>`/blank (`_`), collapse consecutive duplicates, drop spaces. Anything that
   skips step 2 feeds the synthesiser a string of stuttered phonemes -- which still sounds
   fluent-ish, and would *not* be caught by an RMS check. That is why it is spelled out here.

Usage:
    python3 scripts/extract-open-phonemizer.py --accept-licence
    python3 scripts/spike-kokoro-offline.py --phonemizer open --text "Chiti speaks offline."
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
import tarfile
import time
import urllib.request
from pathlib import Path

PACKAGE = "expo-open-phonemizer"
VERSION = "1.0.1"
TARBALL_URL = f"https://registry.npmjs.org/{PACKAGE}/-/{PACKAGE}-{VERSION}.tgz"
TARBALL_SHA256 = "38bee68f550c7d96dc9917e10397e6ec20b53fe39a65fd2a4c97af8d26026c3c"
TARBALL_BYTES = 62331192
MEMBERS = {
    "package/build/assets/open-phonemizer.onnx": "open-phonemizer.onnx",
    "package/src/tokenizer.ts": "_tokenizer.ts",
    "package/src/dictionary.ts": "_dictionary.ts",
}


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        while block := fh.read(1 << 20):
            h.update(block)
    return h.hexdigest()


def ts_object_literal(src: str, var_regex: str) -> dict:
    """Parse a TS object literal that JSON forbids: bare numeric keys, trailing commas."""
    m = re.search(var_regex, src, re.S)
    if not m:
        sys.exit(f"pattern not found: {var_regex!r} -- upstream source layout changed; pin the version instead of guessing")
    body = re.sub(r"([{,]\s*)(\d+):", r'\1"\2":', m.group(1))
    body = re.sub(r",(\s*[}\]])", r"\1", body)
    return json.loads(body)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--dest", default="models/phonemizer")
    ap.add_argument("--tgz", default=None, help="use a local copy of the tarball (no network)")
    ap.add_argument("--url", default=TARBALL_URL)
    ap.add_argument("--sha256", default=TARBALL_SHA256)
    ap.add_argument("--accept-licence", action="store_true", required=False)
    ap.add_argument("--check-names", action="store_true", help="assert the graph's real tensor names with onnxruntime")
    args = ap.parse_args()

    if not args.accept_licence:
        sys.stderr.write(
            "This exists to *avoid* a copyleft dependency, so don't add another one blindly:\n"
            "  * the npm package's own code: MIT\n"
            "  * the lexicon: derived from a public pronunciation dictionary -- confirm its terms\n"
            "    before shipping (it is 274,927 words of someone else's data, not of yours)\n"
            "  * open-phonemizer.onnx: a trained G2P graph; its weights' licence is not stated in\n"
            "    the tarball, so record where you got it before a pack is signed\n"
            "Re-run with --accept-licence once noted (SOURCE.json will record the acknowledgement).\n"
        )
        return 2

    dest = Path(args.dest).expanduser()
    dest.mkdir(parents=True, exist_ok=True)

    if args.tgz:
        tgz = Path(args.tgz)
        print(f"using local tarball {tgz} ({tgz.stat().st_size:,} bytes)")
    else:
        tgz = dest / f".{PACKAGE}-{VERSION}.tgz.part"
        print(f"GET {args.url}")
        with urllib.request.urlopen(args.url, timeout=300) as resp, tgz.open("wb") as out:
            shutil.copyfileobj(resp, out, 1 << 20)
        print(f"    {tgz.stat().st_size:,} bytes")

    digest = sha256(tgz)
    if digest != args.sha256:
        sys.stderr.write(f"sha256 {digest} != pinned {args.sha256}; refusing to extract\n")
        return 1
    if tgz.stat().st_size != TARBALL_BYTES:
        sys.stderr.write(f"size {tgz.stat().st_size} != expected {TARBALL_BYTES}\n")
        return 1
    print(f"sha256 verified {digest}")

    with tarfile.open(tgz, "r:gz") as tar:
        for member_name, out_name in MEMBERS.items():
            try:
                member = tar.getmember(member_name)
            except KeyError:
                sys.stderr.write(f"member {member_name} missing -- upstream layout changed\n")
                return 1
            with tar.extractfile(member) as src, (dest / out_name).open("wb") as out:
                shutil.copyfileobj(src, out, 1 << 20)

    tokenizer = ts_object_literal(
        (dest / "_tokenizer.ts").read_text(encoding="utf-8"),
        r"const tokenizer: TokenizerConfig = (\{.*?\n\});",
    )
    lexicon = ts_object_literal(
        (dest / "_dictionary.ts").read_text(encoding="utf-8"),
        r"const dictionary: Record<string, Record<string, string>> = (\{.*?\n\});",
    )
    langs = tokenizer.get("languages") or ["en_us"]
    if "en_us" not in lexicon:
        sys.stderr.write(f"lexicon has no en_us block (found {sorted(lexicon)[:5]})\n")
        return 1
    table = {
        "text_symbols": tokenizer["text_symbols"],
        "phoneme_symbols": {str(k): v for k, v in tokenizer["phoneme_symbols"].items()},
        "char_repeats": tokenizer["char_repeats"],
        "languages": langs,
        "max_length": 64,
        "decode_rules": {
            "pad_id": 0,
            "pad_symbol": "_",
            "end_symbol": "<end>",
            "steps": ["drop pad/blank `_`", "collapse consecutive duplicates (graph emits char_repeats slots per char)", "drop spaces"],
        },
        "graph": {"input": "text int64[1,64]", "output": "logits f32[1,64,64]",
                  "warning": "upstream JS reads results['output'] and therefore throws on OOV words; the real name is 'logits'"},
    }
    (dest / "phonemizer_tokenizer.json").write_text(json.dumps(table, indent=2) + "\n", encoding="utf-8")
    (dest / "lexicon_en_us.json").write_text(json.dumps(lexicon["en_us"], separators=(",", ":")) + "\n", encoding="utf-8")
    (dest / "SOURCE.json").write_text(json.dumps({
        "source": {"package": f"{PACKAGE}@{VERSION}", "url": args.tgz or args.url, "sha256": digest},
        "extracted_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "licence_note": "MIT code; lexicon and G2P weights need separate verification",
        "assets": {n: {"bytes": (dest / n).stat().st_size, "sha256": sha256(dest / n)}
                   for n in ["open-phonemizer.onnx", "phonemizer_tokenizer.json", "lexicon_en_us.json"]},
    }, indent=2) + "\n", encoding="utf-8")
    for name in ("open-phonemizer.onnx", "phonemizer_tokenizer.json", "lexicon_en_us.json"):
        print(f"  wrote {dest / name} ({(dest / name).stat().st_size:,} bytes)")

    if args.check_names:
        try:
            import onnxruntime as ort  # noqa: PLC0415
        except ImportError:
            print("  (--check-names skipped: onnxruntime not installed)")
        else:
            s = ort.InferenceSession(str(dest / "open-phonemizer.onnx"), providers=["CPUExecutionProvider"])
            ins = [(i.name, i.type, i.shape) for i in s.get_inputs()]
            outs = [(o.name, o.type, o.shape) for o in s.get_outputs()]
            print(f"  graph inputs : {ins}\n  graph outputs: {outs}")
            assert outs and outs[0][0] == "logits", "output renamed -- the decode in this repo must follow"
    print("\nnow: python3 scripts/spike-kokoro-offline.py --phonemizer open --text \"...\"")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
