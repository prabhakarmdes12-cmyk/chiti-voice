#!/usr/bin/env python3
"""
Voice Pack Builder

Creates .cvpack files (ZIP archives) from voice pack directories.
Computes checksums for all files and validates the manifest.
"""

import os
import sys
import json
import zipfile
import hashlib
from pathlib import Path
from typing import Dict, Tuple


def compute_sha256(file_path: Path) -> str:
    """Compute SHA256 checksum of a file."""
    sha256_hash = hashlib.sha256()
    with open(file_path, "rb") as f:
        for byte_block in iter(lambda: f.read(4096), b""):
            sha256_hash.update(byte_block)
    return sha256_hash.hexdigest()


def build_voice_pack(pack_dir: Path, output_path: Path) -> bool:
    """
    Build a voice pack (.cvpack file) from a directory.
    
    Expected directory structure:
    pack_dir/
      manifest.json
      *.onnx (model files)
      *.json (config files)
    
    Returns True if successful, False otherwise.
    """
    print(f"Building voice pack from {pack_dir}...")
    
    # Read and validate manifest
    manifest_path = pack_dir / "manifest.json"
    if not manifest_path.exists():
        print(f"ERROR: {manifest_path} not found!")
        return False
    
    with open(manifest_path, "r") as f:
        manifest = json.load(f)
    
    print(f"  Voice: {manifest.get('name', 'unknown')}")
    
    # Create temporary manifest with checksums
    manifest_copy = manifest.copy()
    
    # Compute checksums for all declared files
    for file_entry in manifest_copy.get("files", []):
        file_path = pack_dir / file_entry["path"]
        if file_path.exists():
            checksum = compute_sha256(file_path)
            size = file_path.stat().st_size
            file_entry["checksum_sha256"] = checksum
            file_entry["size_bytes"] = size
            print(f"    {file_entry['path']}: {size} bytes, {checksum[:8]}...")
        else:
            print(f"  WARNING: File not found: {file_entry['path']}")
            # For Phase 1, we use placeholder checksums
            print(f"    Using placeholder checksum for {file_entry['path']}")
    
    # Create .cvpack ZIP file
    with zipfile.ZipFile(output_path, "w", zipfile.ZIP_DEFLATED) as zf:
        # Add manifest
        manifest_json = json.dumps(manifest_copy, indent=2)
        zf.writestr("manifest.json", manifest_json)
        
        # Add all files from the directory
        for file_entry in manifest_copy.get("files", []):
            file_path = pack_dir / file_entry["path"]
            if file_path.exists():
                zf.write(file_path, arcname=file_entry["path"])
            else:
                # For Phase 1, create dummy files with placeholder data
                print(f"  Creating placeholder for {file_entry['path']}")
                zf.writestr(file_entry["path"], b"PLACEHOLDER_MODEL_DATA_PHASE_1")
    
    print(f"✓ Created {output_path}")
    print(f"  File size: {output_path.stat().st_size} bytes")
    return True


def main():
    """Build all voice packs."""
    project_root = Path(__file__).parent.parent.absolute()
    voice_packs_dir = project_root / "voice-packs"
    
    if not voice_packs_dir.exists():
        print(f"ERROR: {voice_packs_dir} not found!")
        return 1
    
    # Create output directory for .cvpack files
    output_dir = project_root / "voice-packs" / "dist"
    output_dir.mkdir(exist_ok=True)
    
    success_count = 0
    
    # Build each voice pack
    for pack_dir in voice_packs_dir.iterdir():
        if pack_dir.is_dir() and (pack_dir / "manifest.json").exists():
            voice_name = pack_dir.name
            output_path = output_dir / f"{voice_name}.cvpack"
            
            if build_voice_pack(pack_dir, output_path):
                success_count += 1
    
    print(f"\n✓ Built {success_count} voice pack(s)")
    
    if success_count == 0:
        return 1
    
    return 0


if __name__ == "__main__":
    sys.exit(main())
