#!/usr/bin/env python3
"""Generate `.cvpack` test fixtures under crates/voice-pack/tests/fixtures/.

These are hostile/broken archives used by the Rust integration test
`crates/voice-pack/tests/pack_security.rs`. They are generated rather than
hand-committed so the fixtures stay reproducible and the intent of each one is
documented in code.

Run:  python3 scripts/make-test-fixtures.py
"""

from __future__ import annotations

import hashlib
import json
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "crates" / "voice-pack" / "tests" / "fixtures"

MANIFEST_NAME = "manifest.json"
MODEL = b"CHITI_PLACEHOLDER_MODEL_NO_REAL_ONNX"


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def manifest(files: list[dict], status: str | None = "placeholder", provenance: dict | None = None) -> str:
    m = {
        "schema_version": "1.0.0",
        "id": "fixture",
        "name": "Fixture",
        "version": "1.0.0",
        "author": "Chiti Technologies",
        "license": "Proprietary",
        "description": "test fixture",
        "engine_family": "piper",
        "engine_version_min": "1.0.0",
        "supported_languages": ["en-IN"],
        "files": files,
        "persona": None,
        "provenance": provenance
        if provenance is not None
        else {
            "training_data_statement": "n/a (placeholder)",
            "model_license": "n/a",
            "consent_obtained": None,
            "dataset_attribution": "n/a",
            "build_timestamp": "2026-09-03T00:00:00Z",
            "signature": None,
            "signature_status": "UNSIGNED",
        },
    }
    if status:
        m["status"] = status
    return json.dumps(m, indent=2)


def entry(path: str, data: bytes, file_type: str = "model") -> dict:
    return {
        "path": path,
        "checksum_sha256": sha(data),
        "size_bytes": len(data),
        "file_type": file_type,
    }


def write(path: Path, manifest_json: str, files: dict[str, bytes]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr(MANIFEST_NAME, manifest_json + "\n")
        for name, data in files.items():
            zf.writestr(name, data)


def main() -> None:
    good_model = entry("model.onnx", MODEL)
    good_cfg = entry("model_config.json", b'{"sample_rate": 22050}\n', "config")

    fixtures: dict[str, tuple[str, dict[str, bytes]]] = {}

    # 1. Well-formed placeholder pack: the happy path.
    fixtures["ok.cvpack"] = (manifest([good_model, good_cfg]), {"model.onnx": MODEL, "model_config.json": b'{"sample_rate": 22050}\n'})

    # 2. Zip bomb: manifest is *internally consistent* (correct size + hash) but the
    #    entry expands 1000x from its compressed form. The old loader read it into
    #    memory before validating; the new one must reject it from the central directory.
    bomb = b"\0" * (32 * 1024 * 1024)
    fixtures["zip_bomb.cvpack"] = (manifest([entry("model.onnx", bomb)]), {"model.onnx": bomb})

    # 3. Undeclared extra entry (the loader allowlists against the manifest).
#    Named .txt on purpose so it fails on "undeclared", not on the extension blacklist.
    fixtures["undeclared_entry.cvpack"] = (
        manifest([good_model, good_cfg]),
        {"model.onnx": MODEL, "model_config.json": b'{"sample_rate": 22050}\n', "extra/notes.txt": b"smuggled"},
    )

    # 4. Path traversal declared in the manifest.
    evil = entry("../escape.onnx", MODEL)
    fixtures["path_traversal.cvpack"] = (manifest([evil]), {"../escape.onnx": MODEL})

    # 5. Executable content declared in the manifest.
    sh = entry("hooks/install.sh", MODEL)
    fixtures["executable.cvpack"] = (manifest([sh]), {"hooks/install.sh": MODEL})

    # 6. Tampered payload: same LENGTH as declared, different bytes, so it exercises
    #    the SHA-256 path specifically (validate_files checks size before checksum).
    tampered_model = MODEL[:-1] + bytes([MODEL[-1] ^ 0x20])  # same length, one byte flipped
    assert len(tampered_model) == len(MODEL) and tampered_model != MODEL
    fixtures["tampered.cvpack"] = (manifest([good_model, good_cfg]), {"model.onnx": tampered_model, "model_config.json": b'{"sample_rate": 22050}\n'})

    # 7. Truncated payload: same size field, fewer bytes on disk.
    fixtures["truncated.cvpack"] = (manifest([good_model]), {"model.onnx": b"short"})

    # 8. Declared size 0 — the exact defect that made all three shipped packs unusable.
    zero = dict(good_model)
    zero["size_bytes"] = 0
    fixtures["zero_size.cvpack"] = (manifest([zero]), {"model.onnx": MODEL})

    # 9. Placeholder zero-hash checksum.
    badhash = dict(good_model)
    badhash["checksum_sha256"] = "0" * 64
    fixtures["zero_hash.cvpack"] = (manifest([badhash]), {"model.onnx": MODEL})

    # 10. Real (non-placeholder) pack with no provenance block -> VOICE_INV_008 gate.
    fixtures["real_no_provenance.cvpack"] = (manifest([good_model, good_cfg], status=None, provenance=None), {"model.onnx": MODEL, "model_config.json": b'{"sample_rate": 22050}\n'})

    # 11. Unsupported schema version.
    m = json.loads(manifest([good_model]))
    m["schema_version"] = "2.0.0"
    fixtures["bad_schema.cvpack"] = (json.dumps(m), {"model.onnx": MODEL})

    # 12. Oversized single file vs the embedded profile (200 MB declared, valid hash).
    big_declared = {
        "path": "model.onnx",
        "checksum_sha256": sha(b"\0" * 1024),
        "size_bytes": 200 * 1024 * 1024,
        "file_type": "model",
    }
    fixtures["oversize.cvpack"] = (manifest([big_declared]), {"model.onnx": b"\0" * 1024})

    # 13. Duplicate manifest entries.
    fixtures["duplicate.cvpack"] = (manifest([good_model, good_model]), {"model.onnx": MODEL})

    # 14. Not a zip at all.
    (OUT / "not_a_zip.cvpack").parent.mkdir(parents=True, exist_ok=True)
    (OUT / "not_a_zip.cvpack").write_bytes(b"this is not a zip archive")

    for name, (man, files) in fixtures.items():
        write(OUT / name, man, files)

    print(f"wrote {len(fixtures) + 1} fixtures to {OUT}")
    for name in sorted(list(fixtures) + ["not_a_zip.cvpack"]):
        p = OUT / name
        print(f"  {name:<28} {p.stat().st_size:>9} bytes")


if __name__ == "__main__":
    main()
