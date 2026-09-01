# Chiti Vocal Runtime — Privacy Architecture

> **Version:** 0.1.0-alpha  
> **Status:** Architecture Definition  
> **Last Updated:** September 2026  
> **Owner:** Chiti Platform Team

---

## Overview

Privacy is a design property of the Chiti Vocal Runtime, not a compliance checkbox. The architecture makes privacy violations structurally impossible in the default configuration — not just policy-discouraged.

This document defines what data the runtime collects, stores, and transmits; what it explicitly does not do; how local-mode guarantees are enforced and tested; and the consent architecture for future voice creation features.

---

## Privacy by Default

All privacy protections are ON by default. No opt-out is required for local-mode operation. No account creation or registration is required to use the runtime.

| Behavior | Default | Override Available | Override Mechanism |
|----------|---------|-------------------|-------------------|
| Telemetry collection | **OFF** | No | No override exists |
| Network analytics | **OFF** | No | No override exists |
| Text logging | **OFF** | Yes (dev mode only) | `CHITI_VOICE_DEV_LOG=true` + `CHITI_VOICE_LOG_LEVEL=TRACE` |
| Audio recording of synthesis output | **OFF** | No | No override exists |
| Cloud synthesis routing | **OFF** | Yes (explicit opt-in) | Application must explicitly configure a cloud engine; no default |
| Text upload to any server | **OFF** | No | No override exists |
| User identifiers / persistent IDs | **Not collected** | No | No override exists |
| Cross-session synthesis history | **Not stored** | No | No override exists |
| Device fingerprinting | **Not performed** | No | No override exists |
| Crash reports | **OFF** | No | No automatic reporting |

---

## Local Mode Guarantee

When the runtime is operating in local mode — meaning no explicit cloud synthesis engine has been configured by the application — the following guarantees are absolute.

### The Runtime MUST NOT:

1. **Transmit any text** — Neither the full synthesis request text, nor fragments, nor phoneme sequences derived from the text, may be sent to any remote server.

2. **Transmit any audio** — The synthesized PCM audio, or any representation of it (WAV, MP3, spectrogram, embedding), may not be transmitted to any remote server.

3. **Transmit any identifier** — No device ID, installation ID, user ID, session ID, or any locally-generated identifier may be sent to any remote server.

4. **Make any outbound network request** — In local mode, the synthesis path MUST be entirely contained within the local process. The only permitted network socket is the loopback interface (`127.0.0.1`) for inter-process communication between the SDK and the Local Service daemon. There are zero outbound TCP/UDP connections to external hosts.

### Formal Statement

```
∀ synthesis request r in local mode:
  network_packets_sent(r) ∩ non_loopback_destinations = ∅
```

For every synthesis request processed in local mode, the set of non-loopback network packets sent during synthesis is empty.

---

## Network Isolation Test

The local mode guarantee is not verified by code review alone. An automated integration test actively monitors the network during synthesis.

### What "It worked without internet" Does NOT Verify

A test that simply disconnects the internet cable and verifies synthesis completes does not prove zero outbound connections. It proves only that synthesis can tolerate the absence of a network response. The runtime could still be sending data that goes unacknowledged.

### Required Test Mechanism

The offline integration test suite MUST use one of the following mechanisms to actively verify zero outbound packets during synthesis:

**Option A — Network Namespace (Linux CI):**
```sh
# Create an isolated network namespace with no external routes
ip netns add chiti-voice-test
ip netns exec chiti-voice-test ip lo set up

# Run synthesis inside the namespace
ip netns exec chiti-voice-test ./run-synthesis-test

# The namespace has no external network interfaces.
# Any attempt to connect to an external host will fail with ENETUNREACH.
# Test passes only if synthesis completes successfully despite this.
```

**Option B — Packet Capture + Assert (All Platforms):**
```sh
# Start tcpdump/Wireshark capture on loopback and all interfaces
tcpdump -i any -w /tmp/chiti-synthesis-capture.pcap &
CAPTURE_PID=$!

# Run synthesis
./run-synthesis-test

# Stop capture
kill $CAPTURE_PID

# Assert: zero packets to non-loopback destinations
./scripts/assert-no-external-packets.py /tmp/chiti-synthesis-capture.pcap
```

**Option C — Mock Network Layer (Windows CI):**

The `chiti-vocal-core` crate uses a compile-time feature flag in tests (`#[cfg(test)]`) to replace the OS networking layer with a mock that:
1. Allows loopback connections
2. Fails all non-loopback connection attempts immediately with an error
3. Records any attempt to connect to a non-loopback address
4. After the test, asserts the recorded list is empty

**CI Requirement:**

At least one of Option A (Linux) or Option C (all platforms) MUST run on every pull request. The test result is a hard gate — it is not informational.

---

## Developer Mode Logging

Developer logging allows engineers to inspect the synthesis pipeline during development. It is explicitly restricted to prevent accidental production enablement.

### Activation Requirements

Developer mode logging is only active when **ALL** of the following are simultaneously true:

1. The environment variable `CHITI_VOICE_LOG_LEVEL` is set to `TRACE`
2. The environment variable `CHITI_VOICE_DEV_LOG` is set to `true` (case-insensitive)
3. The binary was built with the `dev-logging` Cargo feature (not included in `--release` builds by default)

Meeting any two of three conditions does not activate developer logging. All three must be present.

### Startup Warning

When developer logging is active, the runtime emits the following warning to `stderr` before any synthesis occurs:

```
╔══════════════════════════════════════════════════════════════╗
║  ⚠  CHITI VOCAL RUNTIME — DEVELOPER LOGGING ENABLED         ║
║                                                              ║
║  Synthesized text may appear in log output.                  ║
║  This mode is ONLY for local development.                    ║
║  NEVER enable this in a production or shared environment.    ║
║                                                              ║
║  Activated by: CHITI_VOICE_DEV_LOG=true                      ║
║  Disable by: unset CHITI_VOICE_DEV_LOG                       ║
╚══════════════════════════════════════════════════════════════╝
```

This warning is emitted regardless of log level configuration. It cannot be suppressed.

### What Developer Logging Exposes

When active, the following may appear in log output at `TRACE` level:

- The full text of each synthesis request
- The resolved persona parameters (speaking rate, pitch, energy, style vector)
- Phoneme sequences produced by the G2P layer
- Synthesis timing breakdowns (normalization time, G2P time, inference time, audio pipeline time)
- State machine transitions with requestId
- Engine health diagnostics

### What Developer Logging Never Exposes

Even in developer mode:
- Synthesized audio is never written to log files
- `requestId` values are UUIDs generated locally — they contain no user identity
- No network requests are made to transmit log data

### Production Build Guarantee

The `--release` build profile excludes the `dev-logging` Cargo feature. Even if `CHITI_VOICE_DEV_LOG=true` is set in the environment, a production binary will ignore it. The startup warning will not appear. Text will not be logged.

This is enforced by:
```toml
# Cargo.toml
[features]
default = []
dev-logging = []  # Explicitly NOT in default features

[profile.release]
# dev-logging feature is never enabled in release builds
```

CI verifies that the release binary does not log synthesis text even when the environment variables are set.

---

## Data Minimization

The runtime is designed to handle requests without creating any persistent user-associated data.

### Request Identifiers

`requestId` values are:
- Generated locally using a random UUID v4
- Not derived from any user identity, device ID, or session
- Not persisted after the synthesis request completes
- Not transmitted to any server (even in local service mode — the ID is for in-process tracking)

Two synthesis requests from the same user, on the same device, at different times, produce `requestId` values that are cryptographically unrelated.

### No Persistent User Profiles

In local mode, the runtime stores:
- Voice pack files (user-installed models)
- Persona configuration files (user-configured personas)
- Origin allowlist (Local Service daemon configuration)

The runtime does NOT store:
- A history of synthesized texts
- A log of which persona was used when
- Timing or usage patterns
- Any user profile or preference data derived from synthesis behavior

### Voice Lab Session Data

The Voice Lab (developer workbench) may store session data for developer convenience (e.g., last-used test sentences, persona tuning state). This data:
- Stays entirely on the local machine
- Is stored in `~/.chiti/voice-lab/session/` (not synced)
- Can be deleted by the user at any time
- Contains no synthesis audio (only text and parameter values)

### Benchmark Data

When the built-in benchmarking tool runs:
- It records: machine specs (CPU model, core count, RAM, OS), inference timing (RTF, first-chunk latency), MOS scores (if automated evaluation is enabled)
- It does NOT record: the evaluation sentences, any synthesis output audio, any user identity
- Benchmark results, if shared with Chiti Technologies for research purposes, contain only machine specs and timing/quality numbers — never text content

---

## Consent Architecture for Future Voice Creation (Chiti Voice Foundry)

The Chiti Voice Foundry is a planned future product that enables the creation of new voice personas from recorded speech data. When it is built, the following consent architecture MUST be implemented before any voice data is collected.

### Principles

1. **Explicit consent only.** No implicit consent from use of a product. Voice data collection requires a dedicated, unambiguous consent action.

2. **Informed consent.** The voice owner must understand exactly what their voice data will be used for before consenting.

3. **Granular consent.** Consent for one purpose (e.g., "create a personal voice") does not imply consent for another purpose (e.g., "train a general voice model").

4. **Revocable consent.** Voice owners may withdraw consent at any time. Withdrawal triggers a defined data deletion process.

5. **Auditable consent.** Every consent event is logged with a timestamp, the identity of the consenting party, the specific purposes consented to, and the version of the consent disclosure shown.

### Required Consent Record Fields

```json
{
  "consentId": "uuid-v4",
  "recordedAt": "2026-09-01T00:00:00Z",
  "voiceOwnerId": "anonymized-owner-id",
  "consentVersion": "1.0.0",
  "disclosureShownVersion": "1.0.0",
  "allowedPurposes": [
    "personal-voice-clone",
    "chiti-internal-model-training"
  ],
  "disallowedPurposes": [
    "third-party-commercial-licensing",
    "public-distribution"
  ],
  "licenseType": "chiti-voice-creation-license-v1",
  "revocationPolicy": "delete-within-30-days",
  "datasetProvenance": {
    "recordingDate": "2026-09-01",
    "recordingDevice": "redacted",
    "sentenceSetVersion": "v2.1",
    "totalSentences": 500
  },
  "revoked": false,
  "revokedAt": null,
  "deletionConfirmedAt": null
}
```

### Revocation Process

When a voice owner revokes consent:

```
1. Revocation request received
       │
       ▼
2. consentRecord.revoked = true
   consentRecord.revokedAt = now()
       │
       ▼
3. Voice model derived from this data flagged for deletion
       │
       ▼
4. Deletion executed within 30 days
       │
       ▼
5. consentRecord.deletionConfirmedAt = now()
       │
       ▼
6. Voice owner notified of deletion confirmation
```

The consent record itself (without the voice data) is retained for audit purposes with a retention period defined by applicable data protection law.

### Dataset Provenance Logging

Every dataset used to train or fine-tune a voice model MUST have a provenance record that traces:
- Which consent records authorize this dataset's use
- The exact sentence set version used
- The date of recording
- The purpose for which the dataset was collected
- The resulting model(s) trained from this dataset

This chain of provenance enables: auditing model lineage, responding to data subject access requests, and executing consent revocations (identifying all models trained on the revoked data).

### No Implicit Consent — Examples

The following do NOT constitute valid consent for voice data collection:

- Accepting general terms of service that include a clause about voice data ❌
- Using a product feature that incidentally records voice as a side effect ❌
- Clicking "I agree" on a general privacy policy update ❌
- Failing to object to a pre-ticked checkbox ❌

Valid consent requires:
- A specific, standalone consent action (a dedicated consent screen) ✅
- Clear language describing the specific purposes ✅
- An unambiguous affirmative action (not a pre-ticked checkbox) ✅
- The ability to review and withdraw consent at any time ✅

---

## Summary: Privacy Guarantees by Mode

| Mode | Text Leaves Device? | Audio Leaves Device? | User ID Collected? | Logging? |
|------|--------------------|--------------------|-------------------|---------|
| **Local (default)** | Never | Never | Never | Never (prod) |
| **Local + Dev Mode** | Never (only to log file) | Never | Never | Text to local log only |
| **Local Service** | Never (loopback only) | Never | Never | Never (prod) |
| **Cloud (opt-in)** | Yes — to configured cloud provider | Optional | Depends on provider | Depends on provider |

Cloud mode is an explicit opt-in that applications must configure. It is never activated by default. When cloud mode is used, the privacy properties of the cloud provider apply and must be disclosed to end users by the application developer.

---

*Document maintained by the Chiti Platform Team. For security controls enforcing these privacy properties, see [SECURITY.md](./SECURITY.md). For the offline independence invariant, see VOICE_INV_001 in [INVARIANTS.md](./INVARIANTS.md).*
