# Chiti Vocal Local Service — HTTP API Reference

> **STATUS: SPECIFICATION ONLY — NOT IMPLEMENTED (2026-09-03).** No daemon and no SDK
> exist in this repository: there is no HTTP server, no listener, no `@chiti/voice-web`
> package, and no TypeScript anywhere. Every symbol below is a design proposal. Earlier
> documents (`ADR-001`) asserted the SDK had been "established" in Phase 1 — that was false.
> See `README.md` for what actually runs, and `docs/ROADMAP_EMBEDDED.md` §2 Step 3.

> **Unresolved spec conflict:** this document uses port **8765**, while `README.md`,
> `PRD.md` and `AGENTS.md` use **7731**. Resolve before implementation — and note the port
> is security-relevant, since loopback binding plus origin checks are the daemon's only
> defence.

Base URL: `http://127.0.0.1:8765`
API Version: v1
Default Port: 8765
Protocol: HTTP/1.1, WebSocket (streaming endpoint)

## Authentication
Local mode: loopback only (127.0.0.1). Origin header is validated against origin allowlist.

## Endpoints

### GET /v1/health
Health check and loaded voice runtime status.
```json
{
  "status": "ok",
  "version": "0.1.0",
  "uptime": 42,
  "voicesLoaded": ["tara"],
  "engine": "piper-adapter"
}
```

### GET /v1/voices
List installed voice packs.
```json
[
  {
    "id": "tara",
    "name": "Tara",
    "version": "1.0.0",
    "languages": ["en-IN", "hi-IN"],
    "capabilities": ["streaming", "styles"]
  }
]
```

### POST /v1/voices/load
Load a voice into memory.
**Request:**
```json
{ "voice": "tara" }
```
**Response:**
```json
{ "loaded": true, "voice": "tara", "durationMs": 320 }
```

### POST /v1/speak
Synthesize audio from text.
**Request:**
```json
{
  "text": "Welcome to Chiti Vocal Runtime.",
  "voice": "tara",
  "intent": "GREETING",
  "format": "wav"
}
```
**Response:** Binary Audio (`Content-Type: audio/wav`) or Base64 JSON payload.

### POST /v1/stop
Cancel active synthesis and playback immediately.
**Request:**
```json
{ "requestId": "opt-req-id" }
```
**Response:**
```json
{ "stopped": true }
```

### GET /v1/capabilities
Engine capabilities & hardware profile.

### WebSocket /v1/stream
Real-time chunked audio streaming.
- Client sends JSON `SynthesisRequest` frame.
- Server sends binary PCM/WAV frames as synthesized.
- Server sends JSON `{"type": "end"}` on completion.
- Client may send `{"type": "cancel"}` to abort.

## Error Response Format
```json
{
  "error": {
    "code": "VOICE_NOT_FOUND",
    "message": "Voice 'xyz' is not installed. Run: chiti-voice install xyz.cvpack",
    "requestId": "req-123"
  }
}
```
