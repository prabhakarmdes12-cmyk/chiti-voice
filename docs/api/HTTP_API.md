# Chiti Vocal Local Service — HTTP API Reference
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
