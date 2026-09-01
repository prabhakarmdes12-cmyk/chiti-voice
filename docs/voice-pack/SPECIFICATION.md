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
