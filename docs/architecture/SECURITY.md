# Chiti Vocal Runtime — Security Architecture

> **Version:** 0.1.0-alpha  
> **Status:** Architecture Definition  
> **Last Updated:** September 2026  
> **Owner:** Chiti Platform Team

---

## Overview

The Chiti Vocal Runtime operates in a security-sensitive context: it processes text (potentially confidential), runs as a local daemon with an HTTP endpoint, and loads third-party voice pack files. This document defines the threat model, mitigations, and security controls for each component.

Security requirements intersect directly with the system invariants. See [INVARIANTS.md](./INVARIANTS.md), especially VOICE_INV_007 (Local Privacy) and VOICE_INV_008 (Voice Provenance).

---

## Threat Model

### Threat 1 — Unauthorized Website Invoking Local Voice Synthesis

| Field | Detail |
|-------|--------|
| **Category** | CORS / Origin Attack |
| **Vector** | A malicious website (`https://evil.example.com`) makes a cross-origin request to the Chiti Vocal Local Service running on `http://127.0.0.1:45231` |
| **Impact** | The attacker causes the victim's device to synthesize attacker-controlled text and potentially exfiltrate audio or consume resources |
| **Likelihood** | HIGH — any website can make a fetch request to localhost if CORS is misconfigured |

**Mitigation:**

- The Local Service maintains an **origin allowlist** of registered application origins.
- The `Origin` header of every request is validated against the allowlist before the request is dispatched to the synthesis pipeline.
- Unregistered origins receive `403 Forbidden` with no additional information.
- The allowlist is populated at installation time and can only be modified by the local user (not remotely).
- `Access-Control-Allow-Origin: *` is **explicitly forbidden** in the Local Service — see the CORS section below.

---

### Threat 2 — Malicious Voice Pack Containing Path Traversal

| Field | Detail |
|-------|--------|
| **Category** | Path Traversal |
| **Vector** | A `.cvpack` file contains entries with paths like `../../etc/passwd`, `C:\Windows\System32\evil.dll`, or `/root/.ssh/authorized_keys` |
| **Impact** | File overwrite outside the designated voice pack extraction directory |
| **Likelihood** | MEDIUM — requires user to install a malicious pack |

**Mitigation:**

- The manifest is validated **before extraction** of any archive entry.
- All file paths in the manifest are validated against a strict allowlist: relative paths only, no `..` components, no absolute paths, no leading `/`.
- Path validation code:
  ```rust
  fn is_safe_pack_path(path: &Path) -> bool {
      // Must be relative
      if path.is_absolute() { return false; }
      // No parent directory components
      if path.components().any(|c| c == Component::ParentDir) { return false; }
      // No empty components or hidden files at root
      true
  }
  ```
- Extraction target directory is determined before extraction and all extracted paths are checked to be within it.

---

### Threat 3 — Malicious Voice Pack as Zip Bomb

| Field | Detail |
|-------|--------|
| **Category** | Resource Exhaustion |
| **Vector** | A `.cvpack` (ZIP format) file contains highly compressed data that expands to gigabytes when extracted |
| **Impact** | Disk exhaustion, device instability |
| **Likelihood** | LOW — requires user to install a malicious pack |

**Mitigation:**

- Pack loader enforces an **uncompressed size limit** before extraction begins.
- The manifest declares the expected uncompressed size; the loader verifies this matches the ZIP central directory.
- A runtime extraction limit halts extraction if the running total of extracted bytes exceeds the maximum allowed (configurable; default: 500 MB).
- Per-file size limits are enforced independently of total pack size.

---

### Threat 4 — Malicious Voice Pack Containing Executable Files

| Field | Detail |
|-------|--------|
| **Category** | Malware Distribution |
| **Vector** | A `.cvpack` file contains `.exe`, `.sh`, `.py`, `.js`, `.bat`, `.cmd`, `.dll`, `.so`, or other executable file types |
| **Impact** | Arbitrary code execution on the victim's device |
| **Likelihood** | LOW to MEDIUM — voice packs are data; executables are unexpected |

**Mitigation:**

- Voice packs are **data containers**. The runtime never executes any file contained in a voice pack.
- The manifest validator rejects any pack declaring file types outside the allowlist:
  - Allowed: `.onnx`, `.json`, `.txt`, `.bin`, `.wav`, `.png` (for pack artwork)
  - Rejected: `.exe`, `.sh`, `.py`, `.js`, `.ts`, `.bat`, `.cmd`, `.dll`, `.so`, `.dylib`, `.ps1`, and any other executable or script format
- File type is validated by both declared extension and magic bytes (first 8 bytes). A file named `model.onnx` with a PE header is rejected.

---

### Threat 5 — Pack Checksum Manipulation

| Field | Detail |
|-------|--------|
| **Category** | Integrity Violation |
| **Vector** | An attacker modifies a model file inside a `.cvpack` and updates the ZIP archive but cannot update the SHA-256 checksum in the manifest (or the manifest itself is modified) |
| **Impact** | Tampered model is loaded; could degrade quality, introduce bias, or produce harmful audio |
| **Likelihood** | LOW (local attack vector) but HIGH impact |

**Mitigation:**

- SHA-256 checksums for all model files are declared in `manifest.json`.
- `pack/verify.rs` computes the SHA-256 of each extracted file and compares against the manifest value.
- If any checksum mismatches, the entire pack is rejected with `PackTamperingError`. No partial loads.
- The manifest itself is integrity-protected by the pack signature (future: when signing is implemented; current: manifest checksum stored separately in a pack header field).

---

### Threat 6 — Voice Pack with Forged Provenance

| Field | Detail |
|-------|--------|
| **Category** | Supply Chain |
| **Vector** | A malicious actor creates a `.cvpack` claiming to be an official Chiti voice pack, with a forged publisher name |
| **Impact** | User installs a non-official model believing it to be official |
| **Likelihood** | MEDIUM — until signing is implemented |

**Mitigation (current state — UNSIGNED era):**

- All official Chiti voice packs will carry `signatureStatus: "UNSIGNED"` until the signing infrastructure is deployed.
- The runtime displays a warning when loading unsigned packs in Voice Lab UI.
- No pack is marked `signatureStatus: "VERIFIED"` without a valid cryptographic signature — the field is validated against an enum, not a free string.

**Mitigation (future state — VERIFIED era):**

- Official packs will be signed with an asymmetric private key held by Chiti Technologies.
- The runtime embeds a trust store of official publisher public keys.
- Community packs may be signed by their authors with keys registered in a community key registry.
- `signatureStatus: "TAMPERED"` is set if the signature does not verify against the content.
- `signatureStatus: "REVOKED"` is set if the signing key has been revoked.

---

### Threat 7 — Cross-Site Request Forgery to Local Daemon

| Field | Detail |
|-------|--------|
| **Category** | CSRF |
| **Vector** | A malicious page tricks the user's browser into making state-changing requests to the Local Service (e.g., POST /synthesize) using the user's existing session |
| **Impact** | Unauthorized synthesis, resource consumption |
| **Likelihood** | MEDIUM — browser CORS policies partially mitigate this but do not eliminate it |

**Mitigation:**

- The Local Service requires a `X-Chiti-Origin-Token` header on all synthesis requests.
- The token is generated at daemon startup and provided to registered applications during the pairing handshake.
- Browser requests initiated by a malicious page cannot read the token (same-origin policy) and cannot include the header in a simple cross-origin request.
- The origin allowlist (Threat 1 mitigation) provides defense-in-depth.

---

### Threat 8 — Text Injection Through Voice (Spoken XSS)

| Field | Detail |
|-------|--------|
| **Category** | Content Injection |
| **Vector** | An attacker injects text via an application that uses the runtime, causing the persona to speak attacker-controlled content — potentially impersonating authority, reading out phishing content, or being offensive |
| **Impact** | Reputational, social engineering, accessibility harm |
| **Likelihood** | MEDIUM — depends on the application's input sanitization |

**Mitigation:**

- The Chiti Vocal Runtime is not responsible for sanitizing input text — that is the application's responsibility.
- The runtime documentation explicitly states: **Never synthesize user-controlled text without validation by the application layer.**
- The runtime will not execute any SSML or control sequences that could invoke external resources (no `<audio src="https://..."/>` in SSML support).
- The runtime does not have the concept of "authority" — it is a synthesis engine. Applications must not create UI patterns where the voice itself conveys authority that the text content could subvert.

---

### Threat 9 — Resource Exhaustion via Large Text

| Field | Detail |
|-------|--------|
| **Category** | Denial of Service |
| **Vector** | A caller submits a synthesis request with very large text (hundreds of thousands of characters) to exhaust CPU, memory, or audio buffer |
| **Impact** | Runtime becomes unresponsive; other callers are blocked |
| **Likelihood** | MEDIUM — the Local Service is accessible to all registered web origins |

**Mitigation:**

- Maximum text length is enforced at the `VocalClient` input validation layer before the request enters the pipeline (per VOICE_INV_011).
- Default maximum: 10,000 characters per request.
- The Local Service enforces a maximum HTTP request body size independently of the text length limit.
- Rate limiting: maximum N requests per second per registered origin (configurable; default: 10 rps).
- Queue is bounded: `QueueFullError` returned when the queue is at capacity rather than blocking indefinitely.

---

### Threat 10 — Model File Tampering

| Field | Detail |
|-------|--------|
| **Category** | Integrity Violation |
| **Vector** | An attacker with local file system access modifies the `.onnx` model file in the model cache directory between voice pack installation and runtime load |
| **Impact** | Tampered model loaded; degraded quality, biased output, or malicious weights |
| **Likelihood** | LOW — requires local attacker with write access to model cache |

**Mitigation:**

- SHA-256 checksum of all model files is verified at every `initialize()` call, not only at installation.
- The model cache directory should have permissions restricting write access to the installing user (platform-dependent; installer sets this up).
- Future: model files signed by publisher; runtime verifies signature before loading into ONNX Runtime session.

---

## Local Daemon Security

The Chiti Vocal Local Service is an HTTP/WebSocket daemon that runs on the local machine and accepts connections from web browsers and local applications.

### Bind Address

```
CORRECT:   127.0.0.1:45231
FORBIDDEN: 0.0.0.0:45231
FORBIDDEN: [::]:45231 (IPv6 wildcard)
```

The daemon MUST bind to `127.0.0.1` (IPv4 loopback) only. Binding to `0.0.0.0` would expose the synthesis endpoint to the local network — any device on the same Wi-Fi network could then invoke synthesis on the victim's machine. This is not configurable to wildcard.

### Origin Allowlist

The daemon maintains a registry of allowed web origins. Only requests from registered origins are processed.

```json
// Example: ~/.chiti/vocal-service/origins.json
{
  "allowedOrigins": [
    "http://localhost:3000",
    "http://localhost:5173",
    "https://my-app.local"
  ],
  "registeredAt": "2026-09-01T00:00:00Z"
}
```

Requests with an `Origin` header not in the allowlist receive:
```
HTTP 403 Forbidden
Content-Type: application/json

{"error": "origin_not_allowed", "message": "Origin not registered with Chiti Vocal Service"}
```

No additional information about the allowlist is disclosed in the error.

### CORS Policy

```
Access-Control-Allow-Origin: [specific registered origin]   ← CORRECT
Access-Control-Allow-Origin: *                              ← FORBIDDEN
```

Wildcard CORS is **explicitly forbidden**. The daemon echoes back only the specific allowed origin from the request's `Origin` header (if it matches the allowlist). A wildcard would allow any website to make credentialed requests to the daemon.

### Request Size Limits

```
Maximum synthesis request body: 64 KB
Maximum text field: 10,000 characters (~40 KB UTF-8 worst case)
Maximum WebSocket message: 256 KB
```

Requests exceeding these limits receive `413 Content Too Large` and are not processed further.

### Rate Limiting

```
Default: 10 synthesis requests per second per origin
Burst: up to 20 requests in a 2-second window
Exceeded: 429 Too Many Requests
```

Rate limits are enforced per registered origin (not per IP, to handle NAT and proxy scenarios). They are configurable in the daemon config file.

### Secure Pairing Mechanism

Before a web application can use the Local Service, it must complete a one-time pairing handshake:

```
1. Application navigates user to: http://127.0.0.1:45231/pair
2. Daemon UI presents a confirmation dialog to the user
3. User confirms (grants this origin access)
4. Origin is added to the allowlist
5. Daemon issues a session token to the application
6. Application includes token in X-Chiti-Origin-Token header on future requests
```

This prevents drive-by origin registration from the web. The user must actively confirm pairing. This mechanism will be implemented before the daemon is shipped for public installation.

---

## Voice Pack Security

### Validation Order

The following operations MUST occur in this exact order. Skipping any step or reordering is a security violation.

```
1. Validate manifest schema           (reject before touching any asset)
2. Verify manifest version            (reject incompatible packs)
3. Check total uncompressed size      (reject zip bombs)
4. Enumerate all file paths           (reject path traversal)
5. Validate all file extensions       (reject executables)
6. Extract files to sandboxed dir     (only after all above pass)
7. Verify SHA-256 of each file        (reject after extraction if corrupted)
8. Verify pack signature              (UNSIGNED → warning; TAMPERED → reject)
9. Load model into ONNX session       (only if all above pass)
```

### Sandboxed Extraction Directory

Voice packs are extracted to a dedicated directory:
```
~/.chiti/vocal-packs/extracted/<pack-id>/<pack-version>/
```

This directory is:
- Owned by the local user account that installed the pack
- Not on the system `PATH`
- Not executable by the system (no `+x` on Linux, no execute ACL entries on Windows)

### Forbidden File Types (Comprehensive List)

The following file types are rejected at step 4 (extension check) and step 5 (magic bytes check):

```
.exe  .com  .scr  .bat  .cmd  .ps1  .vbs  .js   .ts   .py
.rb   .pl   .sh   .bash .zsh  .fish .dll  .so   .dylib .jar
.class .war  .ear  .apk  .ipa  .deb  .rpm  .msi  .pkg
```

Allowed types:
```
.onnx  .json  .txt  .bin  .png  .jpg  .webp
```

---

## Provenance and Signing

### Current Status: UNSIGNED

All voice packs in this phase carry `signatureStatus: "UNSIGNED"`. This is an honest declaration, not a silent omission. The field is mandatory in the manifest and must be one of:

```typescript
type SignatureStatus = 
  | 'UNSIGNED'   // No signature present — pack is unverified
  | 'VERIFIED'   // Signature verified against trusted publisher key
  | 'REVOKED'    // Signing key has been revoked; pack must not be trusted
  | 'TAMPERED';  // Content does not match signature; reject immediately
```

**`VALID` is not a valid value.** The runtime will reject manifests with `signatureStatus: "VALID"` — this prevents a naive forgery where an attacker sets the status to a truthy-sounding string.

### Future: Cryptographic Signing Infrastructure

When signing is implemented, the design will be:

```
Publisher private key (held by Chiti Technologies, HSM-stored)
    │
    │ signs hash of:
    │   - manifest.json content
    │   - all model file SHA-256 checksums
    │
    ▼
Pack signature (embedded in manifest.json as base64)

Trust store (embedded in runtime binary):
    - Chiti Technologies public key
    - Community publisher keys (future)

Runtime verification:
    1. Compute hash of manifest content
    2. Verify signature against trust store keys
    3. Set signatureStatus accordingly
```

### Pack Manifest Required Fields

```json
{
  "manifestVersion": "1.0.0",
  "packId": "tara-en-in-v1",
  "packVersion": "1.0.0",
  "displayName": "Tara — Indian English",
  "publisher": "Chiti Technologies",
  "publishedAt": "2026-09-01T00:00:00Z",
  "license": "Proprietary — Chiti Voice License v1",
  "trainingDataset": "Chiti Internal Dataset v1 — consented voice data",
  "signatureStatus": "UNSIGNED",
  "signature": null,
  "files": [
    {
      "path": "model/tara-en-in-v1.onnx",
      "size": 65011712,
      "sha256": "a3f2c1..."
    },
    {
      "path": "persona.json",
      "size": 2048,
      "sha256": "d4e5f6..."
    }
  ]
}
```

---

## Privacy–Security Intersection

Privacy and security reinforce each other in the local runtime design. The following controls serve both goals simultaneously.

| Control | Privacy Benefit | Security Benefit |
|---------|-----------------|------------------|
| No outbound network in synthesis path | Text never transmitted to third parties | No exfiltration vector |
| No text logging in production | Spoken content not stored | Log files cannot be stolen/leaked |
| No audio upload | Audio cannot be repurposed | Audio data cannot be intercepted |
| Local synthesis only | Data never leaves device | No man-in-the-middle possible |
| Loopback-only daemon bind | Local network cannot access daemon | Attack surface reduced to local machine |
| Origin allowlist | No tracking of browsing behavior | Unauthorized origins cannot invoke synthesis |

### Production Logging Policy

In production builds:
- The text being synthesized is **never logged** at any log level.
- The synthesized audio is **never written to disk** unless explicitly requested by the application via the `outputFormat: 'wav'` option.
- `requestId` values are logged (for tracing) but are locally generated UUIDs with no user identity.

### Developer Mode Logging

Developer logging requires **both** of the following environment variables to be set:

```sh
CHITI_VOICE_LOG_LEVEL=TRACE
CHITI_VOICE_DEV_LOG=true
```

When both are set, the runtime logs synthesis text at TRACE level and emits a startup warning:

```
[WARN] [chiti-vocal-runtime] Developer logging is ENABLED.
       Synthesized text may appear in logs.
       Do NOT enable this in production.
       Set by: CHITI_VOICE_DEV_LOG=true
```

`CHITI_VOICE_DEV_LOG` defaults to `false` and is never set in shipped configuration files or default environment setups.

---

## Security Checklist Before Public Release (v1.0)

The following items MUST all be confirmed before the Chiti Vocal Runtime v1.0 is shipped for public installation.

### Local Service Daemon

- [ ] Daemon binds to `127.0.0.1` only — verified by automated test attempting `0.0.0.0` bind
- [ ] Origin allowlist enforced — test: request from unlisted origin returns 403
- [ ] `Access-Control-Allow-Origin: *` absent from all responses — verified by response header test
- [ ] Secure pairing mechanism implemented and tested
- [ ] `X-Chiti-Origin-Token` header validation implemented
- [ ] Rate limiting implemented and tested (429 response verified)
- [ ] Request body size limits implemented and tested (413 response verified)
- [ ] All endpoints return JSON errors — no HTML error pages that could leak server info
- [ ] HTTP server does not expose version information in `Server` header

### Voice Pack Security

- [ ] Manifest-first validation implemented — test: corrupt archive with valid manifest is rejected
- [ ] Path traversal prevention implemented and fuzz-tested
- [ ] Zip bomb protection implemented — test: oversized compressed content rejected
- [ ] Executable file rejection implemented — magic bytes check confirmed
- [ ] SHA-256 verification runs unconditionally — test: single bit flip in model file rejected
- [ ] `signatureStatus` enum validated — `"VALID"` rejected as not a defined value
- [ ] Sandboxed extraction directory confirmed — paths outside sandbox rejected

### Synthesis Pipeline

- [ ] No outbound network calls in synthesis path — network isolation test passes (VOICE_INV_001)
- [ ] No text logged in production build — verified by log capture test with TRACE level
- [ ] `CHITI_VOICE_DEV_LOG=false` by default — verified in default config
- [ ] Resource limits enforced — text length, queue depth, memory (VOICE_INV_011)
- [ ] Cancellation safe from all states — verified by state machine test (VOICE_INV_009)

### Dependency Audit

- [ ] `cargo audit` — zero known CVEs in dependency tree
- [ ] `npm audit` — zero high/critical CVEs in SDK dependencies
- [ ] No HTTP client dependencies in `chiti-vocal-core` crate
- [ ] No LLM dependencies in `chiti-vocal-core` crate
- [ ] ONNX Runtime version pinned to a specific audited release

### Documentation

- [ ] Privacy policy published describing local-only data handling
- [ ] Security disclosure policy documented (`SECURITY.md` at repo root)
- [ ] Voice pack license requirements documented for third-party publishers
- [ ] Known limitations documented (e.g., signing not yet implemented)

---

*Document maintained by the Chiti Platform Team. For privacy controls, see [PRIVACY.md](./PRIVACY.md). For the provenance invariant, see VOICE_INV_008 in [INVARIANTS.md](./INVARIANTS.md).*
