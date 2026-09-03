#!/usr/bin/env python3
"""Voice Pack builder — assembles `.cvpack` archives from voice pack source dirs.

A `.cvpack` is a ZIP containing:
    manifest.json      metadata + the authoritative checksum/size list
    model.onnx         the acoustic model (or a marked placeholder)
    *.json / *.data    config, phoneme tables, external data

WHY THIS SCRIPT WAS WRONG BEFORE
--------------------------------
The old version computed checksums for files in the source directory, found that
`model.onnx` did not exist, left the manifest's placeholder `"size_bytes": 0` /
zero-hash in place, and then *added a different 30-byte placeholder blob to the
archive*. Every pack it produced therefore failed its own manifest: all three
`.cvpack` files in `voice-packs/dist/` were unloadable by `voice-pack::PackLoader`.

The invariant this script now enforces: **never write a manifest entry whose
checksum/size were not computed from the exact bytes that go into the archive**,
and **verify the archive after building it**.

Usage
-----
    python3 scripts/build-voice-packs.py build [--packs DIR] [--out DIR]
                                               [--require-real-models]
    python3 scripts/build-voice-packs.py verify [PACK.cvpack ...]

`--require-real-models` (use in release CI) refuses to emit placeholders at all.
Without it, placeholders are allowed but the manifest says so via
`"status": "placeholder"`, so nothing downstream can mistake them for a voice.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import zipfile
from pathlib import Path

SCHEMA_VERSION = "1.0.0"
PLACEHOLDER_SENTINEL = b"CHITI_PLACEHOLDER_MODEL_NO_REAL_ONNX"
PLACEHOLDER_STATUS = "placeholder"

# Must stay in sync with voice_pack::security::PackLimits defaults.
MAX_FILE_BYTES = 512 * 1024 * 1024

MANIFEST_NAME = "manifest.json"


def sha256_of(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


# A Kokoro-class style matrix: 510 rows x 256 float32. The row *is* the voice, so its size is not a
# formality — `crates/voice-pack/src/manifest.rs` refuses any other length for a declared
# `style_vector`, because a truncated matrix still loads and speaks as a different person.
STYLE_VECTOR_BYTES = 522240


def file_type_for(name: str) -> str:
    lower = name.lower()
    if lower.endswith(".onnx"):
        return "model"
    if "vocoder" in lower:
        return "vocoder"
    if lower.endswith((".phon", "phonemes.json")):
        return "phonemes"
    # Checked before the .json rule: HuggingFace's serialised vocabulary is `tokenizer.json`, and
    # labelling it "config" would let a pack ship one the loader never reads.
    if Path(lower).name == "tokenizer.json":
        return "tokenizer"
    if lower.endswith(".bin"):
        return "style_vector"
    if lower.endswith(".json"):
        return "config"
    return "metadata"


def collect_payload(pack_dir: Path, manifest: dict, require_real: bool) -> tuple[dict[str, bytes], bool]:
    """Return (bytes-per-archive-entry, used_placeholder) for one pack."""
    payload: dict[str, bytes] = {}
    used_placeholder = False

    declared = manifest.get("files") or []
    if not declared:
        raise ValueError(f"{pack_dir.name}: manifest declares no files")

    for entry in declared:
        rel = entry["path"]
        src = pack_dir / rel
        if src.exists():
            data = src.read_bytes()
            if not data:
                raise ValueError(f"{pack_dir.name}: {rel} is empty — refusing to build")
            payload[rel] = data
            continue

        if require_real:
            raise FileNotFoundError(
                f"{pack_dir.name}: declared file {rel} not found in {pack_dir} "
                f"(--require-real-models is set; place the real model file there)"
            )

        print(f"    ! {rel} missing -> embedding placeholder (pack will be status=placeholder)")
        payload[rel] = PLACEHOLDER_SENTINEL
        used_placeholder = True

    # Any extra real file in the pack dir must be declared, because the Rust loader
    # rejects undeclared archive entries by design.
    for path in sorted(pack_dir.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(pack_dir).as_posix()
        if rel == MANIFEST_NAME:
            continue
        if rel not in payload:
            raise ValueError(
                f"{pack_dir.name}: {rel} exists on disk but is not declared in "
                f"manifest.json; the loader rejects undeclared entries. Declare it or delete it."
            )

    return payload, used_placeholder


def build_pack(pack_dir: Path, out_path: Path, require_real: bool) -> bool:
    manifest_path = pack_dir / MANIFEST_NAME
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

    if manifest.get("schema_version") != SCHEMA_VERSION:
        print(f"  ERROR: unsupported schema_version {manifest.get('schema_version')!r}")
        return False

    payload, used_placeholder = collect_payload(pack_dir, manifest, require_real)

    # Checksums/sizes computed from the FINAL bytes, in a stable order.
    files = []
    for entry in manifest["files"]:
        rel = entry["path"]
        data = payload[rel]
        if len(data) > MAX_FILE_BYTES:
            print(f"  ERROR: {rel} is {len(data)} bytes, over the loader limit")
            return False
        ftype = entry.get("file_type") or file_type_for(rel)
        if ftype == "style_vector" and len(data) != STYLE_VECTOR_BYTES:
            print(f"  ERROR: {rel} is {len(data)} bytes; a style vector must be exactly "
                  f"{STYLE_VECTOR_BYTES} (510 x 256 float32). The loader would reject this pack, "
                  f"and one row short it would still load as the wrong voice.")
            return False
        files.append(
            {
                "path": rel,
                "checksum_sha256": sha256_of(data),
                "size_bytes": len(data),
                "file_type": entry.get("file_type") or file_type_for(rel),
            }
        )
    manifest["files"] = files

    if used_placeholder:
        manifest["status"] = PLACEHOLDER_STATUS
    else:
        manifest.pop("status", None)
        provenance = manifest.get("provenance") or {}
        missing = [
            key
            for key in ("training_data_statement", "model_license", "dataset_attribution")
            if not (provenance.get(key) or "").strip()
        ]
        if provenance.get("consent_obtained") is not True:
            missing.append("consent_obtained")
        if missing:
            raise ValueError(
                f"{pack_dir.name}: refusing to build a REAL pack with incomplete provenance; "
                f"missing/invalid: {', '.join(missing)}. The Rust validator enforces "
                f"VOICE_INV_008 and would reject this pack at load time."
            )

    out_path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(out_path, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr(MANIFEST_NAME, json.dumps(manifest, indent=2, ensure_ascii=False) + "\n")
        for rel, data in payload.items():
            zf.writestr(rel, data)

    # Build-time self-check: catch a broken artifact here instead of in a demo.
    errors = verify_pack(out_path)
    if errors:
        for err in errors:
            print(f"  BUILD FAILED VALIDATION: {err}")
        return False

    print(f"  ✓ {out_path.name} ({out_path.stat().st_size} bytes, status={manifest.get('status', 'release')})")
    return True


def verify_pack(path: Path) -> list[str]:
    """Re-read a built pack and confirm every manifest claim. Returns error strings."""
    errors: list[str] = []
    try:
        with zipfile.ZipFile(path) as zf:
            names = set(zf.namelist())
            if MANIFEST_NAME not in names:
                return [f"{path.name}: no {MANIFEST_NAME}"]
            manifest = json.loads(zf.read(MANIFEST_NAME).decode("utf-8"))

            declared = {f["path"] for f in manifest.get("files", [])}
            for extra in sorted(names - declared - {MANIFEST_NAME}):
                errors.append(f"{path.name}: undeclared entry {extra}")

            total = 0
            for entry in manifest.get("files", []):
                rel = entry["path"]
                if rel not in names:
                    errors.append(f"{path.name}: declared file {rel} missing from archive")
                    continue
                data = zf.read(rel)
                total += len(data)
                if len(data) != entry.get("size_bytes"):
                    errors.append(
                        f"{path.name}: {rel} size mismatch (manifest {entry.get('size_bytes')}, actual {len(data)})"
                    )
                digest = sha256_of(data)
                if digest != entry.get("checksum_sha256"):
                    errors.append(
                        f"{path.name}: {rel} checksum mismatch (manifest {entry.get('checksum_sha256')[:12]}…, actual {digest[:12]}…)"
                    )
                if entry.get("size_bytes") == 0:
                    errors.append(f"{path.name}: {rel} declared with size 0")

            if total > MAX_FILE_BYTES:
                errors.append(f"{path.name}: total uncompressed size {total} exceeds per-file budget")
    except (zipfile.BadZipFile, json.JSONDecodeError) as exc:
        errors.append(f"{path.name}: unreadable ({exc})")
    return errors


def cmd_build(args: argparse.Namespace) -> int:
    packs_dir: Path = args.packs
    out_dir: Path = args.out
    out_dir.mkdir(parents=True, exist_ok=True)

    pack_dirs = sorted(p for p in packs_dir.iterdir() if (p / MANIFEST_NAME).exists())
    if not pack_dirs:
        print(f"ERROR: no pack source dirs with {MANIFEST_NAME} under {packs_dir}")
        return 1

    failed = 0
    for pack_dir in pack_dirs:
        print(f"Building {pack_dir.name}...")
        try:
            if not build_pack(pack_dir, out_dir / f"{pack_dir.name}.cvpack", args.require_real_models):
                failed += 1
        except (ValueError, FileNotFoundError, KeyError) as exc:
            print(f"  ERROR: {exc}")
            failed += 1

    print(f"\n{len(pack_dirs) - failed}/{len(pack_dirs)} pack(s) built and verified")
    return 1 if failed else 0


def cmd_verify(args: argparse.Namespace) -> int:
    targets: list[Path] = list(args.packs)
    if not targets:
        targets = sorted((Path(__file__).resolve().parent.parent / "voice-packs" / "dist").glob("*.cvpack"))
    if not targets:
        print("no .cvpack files to verify")
        return 1

    failed = 0
    for path in targets:
        errors = verify_pack(path)
        if errors:
            failed += 1
            print(f"✗ {path.name}")
            for err in errors:
                print(f"    {err}")
        else:
            with zipfile.ZipFile(path) as zf:
                manifest = json.loads(zf.read(MANIFEST_NAME).decode("utf-8"))
            size = path.stat().st_size
            status = manifest.get("status", "release")
            marker = "  (placeholder — cannot speak)" if status == PLACEHOLDER_STATUS else ""
            print(f"✓ {path.name}: {size} bytes, {len(manifest.get('files', []))} files{marker}")
    return 1 if failed else 0


def main(argv: list[str] | None = None) -> int:
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_build = sub.add_parser("build", help="build all packs from voice-packs/*/manifest.json")
    p_build.add_argument("--packs", type=Path, default=root / "voice-packs")
    p_build.add_argument("--out", type=Path, default=root / "voice-packs" / "dist")
    p_build.add_argument(
        "--require-real-models",
        action="store_true",
        help="fail instead of embedding placeholder models (use for release builds)",
    )
    p_build.set_defaults(func=cmd_build)

    p_verify = sub.add_parser("verify", help="verify built .cvpack archives")
    p_verify.add_argument("packs", nargs="*", type=Path)
    p_verify.set_defaults(func=cmd_verify)

    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
