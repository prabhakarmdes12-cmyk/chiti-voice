# Chiti Voice Pack (.cvpack) — Format Specification
Version: 1.0 — Draft
Date: September 2026

## Overview
A `.cvpack` file is a versioned, validated, provenance-aware voice package for Chiti Vocal Runtime. Internally it uses ZIP compression, but applications treat it as an opaque voice package. The format is designed for installability, portability, and eventual cryptographic signing.

## Design Principles
- One file = one installable voice identity
- Provenance is mandatory, not optional
- Security validation before extraction
- Schema versioning from day one
- Shared model references allowed (packs may reference a shared model family)
- No executable code inside a pack

## File Extension
`.cvpack` — Chiti Voice Pack

Do not treat as a generic `.zip`. Applications must use the Chiti Pack Loader, not a general-purpose archive library.

## Internal Directory Structure
```
tara.cvpack/
+-- manifest.json          # Required. Pack identity and engine requirements.
+-- persona.json           # Required. Persona behavior configuration.
+-- provenance.json        # Required. Origin, consent, license metadata.
+-- checksums.json         # Required. SHA-256 hashes of all assets.
+-- models/
¦   +-- acoustic.onnx      # Optional if using shared model family.
+-- voices/
¦   +-- tara.bin           # Speaker embedding / voice adapter.
+-- pronunciation/
¦   +-- en-IN.json         # Pronunciation overrides for English (Indian).
¦   +-- hi-IN.json         # Pronunciation overrides for Hindi.
+-- styles/
¦   +-- neutral.json
¦   +-- greeting.json
¦   +-- warning.json
+-- license/
    +-- LICENSE.txt        # Required. Human-readable license.
```

## manifest.json Schema
```json
{
  "$schema": "https://chiti.ai/schemas/cvpack-manifest-v1.json",
  "schemaVersion": "1.0",
  "id": "tara",
  "name": "Tara",
  "version": "1.0.0",
  "publisher": "Chiti Technologies",
  "publishedAt": "2026-09-01T00:00:00Z",
  "languages": ["en-IN", "hi-IN"],
  "description": "Warm, professional business and hospitality persona.",
  "engine": {
    "family": "piper-vits",
    "minimumRuntimeVersion": "0.1.0",
    "sharedModelFamily": "piper-vits-medium"
  },
  "assets": [
    "persona.json",
    "provenance.json",
    "checksums.json",
    "voices/tara.bin",
    "pronunciation/en-IN.json",
    "pronunciation/hi-IN.json",
    "license/LICENSE.txt"
  ],
  "capabilities": {
    "streaming": true,
    "styles": true,
    "streamingLatency": "LOW",
    "styleList": ["neutral", "greeting", "warning"]
  },
  "tags": ["business", "en-IN", "female-presenting", "professional"]
}
```

## persona.json Schema
```json
{
  "id": "tara",
  "version": "1.0.0",
  "identity": {
    "displayName": "Tara",
    "description": "Warm professional business persona",
    "gender": "female-presenting",
    "archetype": "business-assistant",
    "audience": "adults"
  },
  "language": {
    "primary": "en-IN",
    "supported": ["en-IN", "hi-IN"],
    "pronunciationLocales": ["en-IN", "hi-IN"]
  },
  "baseline": {
    "speed": 1.0,
    "pitch": 0.0,
    "energy": 0.55,
    "warmth": 0.72,
    "expressiveness": 0.58
  },
  "intentProfiles": {
    "GREETING": { "speed": 1.05, "energy": 0.65, "warmth": 0.80, "expressiveness": 0.65 },
    "QUESTION": { "speed": 1.0, "energy": 0.55, "warmth": 0.72, "expressiveness": 0.60 },
    "CONFIRMATION": { "speed": 0.95, "energy": 0.50, "warmth": 0.75, "expressiveness": 0.50 },
    "WARNING": { "speed": 0.90, "energy": 0.45, "warmth": 0.60, "expressiveness": 0.45 },
    "ERROR": { "speed": 0.88, "energy": 0.40, "warmth": 0.55, "expressiveness": 0.40 }
  },
  "characterLimits": {
    "maxUtteranceLength": 10000
  }
}
```

## provenance.json Schema
```json
{
  "synthetic": true,
  "humanVoiceDerived": false,
  "consentRecord": null,
  "publisher": "Chiti Technologies",
  "createdAt": "2026-09-01T00:00:00Z",
  "modelSource": "Piper TTS Open Synthetic Foundation",
  "modelLicense": "MIT",
  "datasetDisclosure": "Synthetic clean speech dataset",
  "license": "MIT",
  "allowedPurposes": ["commercial", "personal", "embedded"],
  "signatureStatus": "UNSIGNED",
  "signature": null
}
```

## checksums.json
Format: map of relative file path ? SHA-256 hex digest. All files in the pack must be listed.

## Pronunciation Override Format
```json
{
  "locale": "en-IN",
  "version": "1.0",
  "entries": [
    {
      "text": "Chiti",
      "pronunciation": "CHI-tee",
      "phoneme": "t??ti",
      "caseSensitive": false,
      "domainScope": null
    }
  ]
}
```

## Validation Rules
1. `manifest.json` is valid JSON matching schema.
2. `schemaVersion` is supported by runtime.
3. `engine.minimumRuntimeVersion` = current runtime version.
4. All files listed in `manifest.assets` exist.
5. All checksums match (SHA-256).
6. No path traversal (no `..` or absolute paths in any filename).
7. Total unpacked size within limit (default 500 MB).
8. No executable files present (`.exe`, `.sh`, `.py`, `.js`, `.dll`, `.so`).
9. `persona.json` is valid and complete.
10. `provenance.json` is valid and complete.
11. `LICENSE.txt` is present.
12. At least one voice asset present.

## Shared Model References
Packs may declare:
```json
"engine": {
  "family": "piper-vits",
  "sharedModelFamily": "piper-vits-medium",
  "minimumRuntimeVersion": "0.1.0"
}
```
This allows the runtime to share one acoustic model among multiple voices — like fonts sharing a rendering engine. Only the small per-speaker embedding file is unique per voice.

## Pack Tool Commands
```bash
cvpack build ./voices/tara    # Build .cvpack from directory
cvpack validate tara.cvpack   # Validate manifest, checksums, schema
cvpack inspect tara.cvpack    # Show manifest, persona, provenance summary
cvpack sign tara.cvpack       # FUTURE: cryptographic signing
```

---

## Implementation status vs. this specification (2026-09-03)

This section exists because the spec above and `crates/voice-pack` had silently drifted.
If you implement to the spec text, you will not match the loader that ships.

| Spec says | Implementation does | Action |
|---|---|---|
| `persona.json` and `provenance.json` as separate files | `persona` and `provenance` are **inline objects in `manifest.json`** (`PackManifest::persona`, `PackManifest::provenance`). No separate files are read. | Pick one. Recommend: keep inline for small persona data; if provenance must grow (consent docs, signatures), split it and update the loader, not the prose. |
| `LICENSE.txt` present is required | Not checked or required. `PackFile.file_type` has no `license` variant. | Either add the requirement to `PackValidator` or delete it here. Currently an unenforced security claim. |
| Total unpacked size limit "default 500 MB" | `PackLimits::default()` = **512 MB per file / 1 GB total**, with profiles `embedded()` (64 MB / 128 MB) and `tiny()` (24 MB / 32 MB) | Numbers above are stale; the code is the source of truth for defaults. Document *why* the profiles differ (device RAM budgets, see `docs/ROADMAP_EMBEDDED.md` §1). |
| "No executable files present" | Enforced by an extension denylist (`.exe .dll .so .dylib .sh .ps1 .bat .js .py .jar …`) at both manifest and entry level | Matches, and is now actually tested (`crates/voice-pack/tests/pack_security.rs`). |
| Nothing about entry allowlisting | **Undeclared archive entries are rejected**, and directory entries are rejected. Packs are copied, never extracted, by the CLI. | Spec must state this: it is the zip-slip defence and a format guarantee (a `.cvpack`'s content set is exactly its manifest). |
| Nothing about compression ratio | Entries whose uncompressed/compressed ratio exceeds the limit are refused **from the central directory, before inflation** (32 desktop / 24 embedded) | Add to spec: this is a normative loader requirement, not an optimization. Implementations in other languages must match it or packs that load in Rust will OOM elsewhere. |
| Nothing about `status` | **`status: "placeholder"`** marks a pack whose model files are placeholders. Placeholder packs are legal, load successfully, and are refused by `build --require-real-models` and by `chiti-voice install` (without `--allow-placeholder`). Provenance completeness is required for every *other* pack. | Add to spec — this is what keeps scaffolding honest and is load-bearing for the licensing gate. |
| `size_bytes: 0` accepted (implicit) | **Rejected**: a declared file with size 0 is refused at manifest validation | This single rule would have prevented every pack in this repo shipping broken. |
| Schema version compatibility "checked" | Exact equality against `"1.0.0"` | Fine for now, but is a hard incompatibility trap for any 1.x revision; state whether the policy is `1.x` compatible or pinned. |

### Manifest field: `status`

```json
{
  "schema_version": "1.0.0",
  "status": "placeholder",
  "files": [ { "path": "model.onnx", "checksum_sha256": "…", "size_bytes": 36, "file_type": "model" } ]
}
```

- **Absent or anything other than `"placeholder"`** → treated as a real pack: provenance
  (`training_data_statement`, `model_license`, `dataset_attribution`, `consent_obtained: true`)
  is mandatory, and a `model.onnx` under 1 MB is an error at `chiti-voice verify` time.
- **`"placeholder"`** → model bytes are stand-ins; provenance may be unknown (fields `null`).
  Such a pack can never synthesize speech, and every tool that touches one says so.
- Unknown fields are ignored by the Rust parser (`serde` default), so forward-compatible
  additions are safe; **removing or renaming a field is not**, and `schema_version` is the
  gate for that.
