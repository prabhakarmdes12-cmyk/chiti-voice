# Chiti Vocal Runtime — Audio Lifecycle State Machine

> **Version:** 0.1.0-alpha  
> **Status:** Architecture Definition  
> **Last Updated:** September 2026  
> **Owner:** Chiti Platform Team

---

## Overview

The Chiti Vocal Runtime maintains an explicit state machine governing the lifecycle of audio synthesis and playback. Every state transition is intentional, logged, and validated. The state machine is the source of truth for what the runtime is doing at any point in time.

There is no implicit state. There are no boolean flags scattered across the codebase that together constitute an implicit state. The state is one value from the `AudioLifecycleState` enum.

---

## States

### UNINITIALIZED

**Description:** The runtime has been instantiated but has not yet loaded any voice model or engine. No synthesis is possible. This is the initial state of every runtime instance.

**Valid entry transitions:** — (initial state only)

**Valid exit transitions:**
- → `INITIALIZING` (on call to `initialize()`)

---

### INITIALIZING

**Description:** The runtime is loading the voice model, verifying the voice pack checksum, setting up the ONNX inference session, and warming up the phoneme tables. This may take between 100 ms and 3 seconds depending on model size and storage speed. During this phase, synthesis calls MUST be queued or rejected with `NotReadyError`.

**Valid entry transitions:**
- From `UNINITIALIZED` (first initialization)
- From `RECOVERING` (re-initialization after error recovery)

**Valid exit transitions:**
- → `READY` (model loaded successfully)
- → `ERROR` (model load failure — file missing, checksum mismatch, ONNX session error)

---

### READY

**Description:** The runtime is fully initialized and idle. A voice model is loaded. A synthesis request may be submitted at any time. This is the steady-state between synthesis operations.

**Valid entry transitions:**
- From `INITIALIZING` (successful load)
- From `CANCELLING` (cancellation confirmed)
- From `PLAYING` (playback complete)
- From `RECOVERING` (successful recovery)

**Valid exit transitions:**
- → `SYNTHESIZING` (on call to `synthesize()` or `speak()`)

---

### SYNTHESIZING

**Description:** The runtime is actively running the synthesis pipeline: text normalization → G2P → Persona Runtime → Prosody Planner → Voice Engine → PCM output. Audio chunks are not yet available for playback.

**Valid entry transitions:**
- From `READY` (new synthesis request accepted)

**Valid exit transitions:**
- → `BUFFERING` (first PCM chunk produced, pre-buffer filling)
- → `CANCELLING` (on call to `cancel()` or barge-in signal)
- → `ERROR` (synthesis pipeline error)

---

### BUFFERING

**Description:** The first PCM chunk has been produced by the engine. The audio pipeline is accumulating chunks into the playback buffer until the pre-roll threshold is reached. This minimizes the risk of underrun during playback.

**Valid entry transitions:**
- From `SYNTHESIZING` (first chunk available)

**Valid exit transitions:**
- → `PLAYING` (pre-roll buffer threshold reached)
- → `CANCELLING` (on call to `cancel()` during buffering)
- → `ERROR` (pipeline error during buffering)

---

### PLAYING

**Description:** Audio is being rendered to the output device. The synthesis pipeline may still be producing further chunks (streaming mode) or may have completed (full-buffer mode). The audio worklet/output thread is consuming the buffer. From the user's perspective, speech is audible.

**Valid entry transitions:**
- From `BUFFERING` (pre-roll threshold met)
- From `PAUSED` (on call to `resume()`)

**Valid exit transitions:**
- → `READY` (playback of all produced audio is complete)
- → `PAUSED` (on call to `pause()`)
- → `CANCELLING` (on call to `cancel()` or barge-in signal)
- → `ERROR` (audio output device error)

---

### PAUSED

**Description:** Playback has been explicitly paused. The synthesis pipeline may have completed; buffered audio is held. The engine is idle. Playback will resume from the current position on `resume()`.

**Valid entry transitions:**
- From `PLAYING` (on call to `pause()`)

**Valid exit transitions:**
- → `PLAYING` (on call to `resume()`)
- → `CANCELLING` (on call to `cancel()` while paused — discards buffered audio)

---

### STOPPING

**Description:** A graceful stop has been requested. The runtime is completing the current synthesis sentence (does not cut mid-word), then halting. Differs from CANCELLING which stops immediately. Used when the application wants to stop after the current natural sentence boundary.

**Valid entry transitions:**
- From `PLAYING` (on call to `stopAfterSentence()`)
- From `SYNTHESIZING` (on call to `stopAfterSentence()`)

**Valid exit transitions:**
- → `READY` (current sentence completed, stopped cleanly)
- → `CANCELLING` (user escalates to immediate cancel while STOPPING)
- → `ERROR` (error during graceful stop)

---

### CANCELLING

**Description:** An immediate cancellation has been requested. The runtime is aborting synthesis, flushing the request queue, stopping the audio worklet, and releasing synthesis resources. This is a transient state; it resolves quickly (< 100 ms per VOICE_INV_009).

**Valid entry transitions:**
- From `SYNTHESIZING` (cancel during synthesis)
- From `BUFFERING` (cancel during buffering)
- From `PLAYING` (cancel during playback / barge-in)
- From `PAUSED` (cancel while paused)
- From `STOPPING` (escalate to immediate cancel)

**Valid exit transitions:**
- → `READY` (cancellation confirmed; runtime ready for new request)
- → `ERROR` (unexpected error during cancellation cleanup)

---

### ERROR

**Description:** The runtime encountered an unrecoverable error within a synthesis cycle. The error is logged with full context. The runtime will attempt recovery automatically if the error type is classified as recoverable. If not recoverable, the runtime moves to UNAVAILABLE.

**Valid entry transitions:**
- From any state (errors can occur in any phase)

**Valid exit transitions:**
- → `RECOVERING` (error is recoverable; automatic or manual recovery triggered)
- → `UNAVAILABLE` (error is not recoverable; runtime must be disposed and re-initialized)

---

### RECOVERING

**Description:** The runtime is attempting to recover from an error by re-initializing the engine or reloading the voice pack. Recovery may involve releasing and recreating the ONNX session. Applications should wait for `READY` before retrying synthesis.

**Valid entry transitions:**
- From `ERROR` (if error is classified as recoverable)

**Valid exit transitions:**
- → `INITIALIZING` (recovery involves full re-initialization)
- → `READY` (fast recovery — engine reset without full model reload)
- → `UNAVAILABLE` (recovery attempt failed)

---

### UNAVAILABLE

**Description:** The runtime is in a terminal failure state. The engine cannot be used. No synthesis is possible. The application must call `dispose()` and then create a new runtime instance and call `initialize()` to recover. This state is logged at ERROR level with full diagnostics.

**Valid entry transitions:**
- From `ERROR` (non-recoverable error)
- From `RECOVERING` (recovery failed)

**Valid exit transitions:**
- None. UNAVAILABLE is terminal. The instance must be disposed.

---

## Normal Synthesis Flow

```
┌──────────────┐
│ UNINITIALIZED│
└──────┬───────┘
       │ initialize()
       ▼
┌──────────────┐
│ INITIALIZING │ (model loading, checksum verification)
└──────┬───────┘
       │ model loaded OK
       ▼
┌──────────────┐
│    READY     │ ◄──────────────────────────────────────┐
└──────┬───────┘                                        │
       │ speak(text)                                    │
       ▼                                                │
┌──────────────┐                                        │
│ SYNTHESIZING │ (pipeline: normalize → G2P → engine)  │
└──────┬───────┘                                        │
       │ first PCM chunk produced                       │
       ▼                                                │
┌──────────────┐                                        │
│  BUFFERING   │ (pre-roll accumulation)                │
└──────┬───────┘                                        │
       │ buffer threshold met                           │
       ▼                                                │
┌──────────────┐                                        │
│   PLAYING    │ (audio rendered to output device)      │
└──────┬───────┘                                        │
       │ playback complete                              │
       └────────────────────────────────────────────────┘
                      (returns to READY)
```

---

## Cancellation Flow

Cancellation (barge-in) is possible from `SYNTHESIZING`, `BUFFERING`, `PLAYING`, and `PAUSED`.

```
[SYNTHESIZING]  ─────┐
[BUFFERING]     ─────┤  cancel()
[PLAYING]       ─────┤  or voice activity detected
[PAUSED]        ─────┘
                      │
                      ▼
               ┌────────────┐
               │ CANCELLING │  (abort synthesis task,
               └─────┬──────┘   flush queue,
                     │          stop audio worklet,
                     │          release synthesis resources)
                     │ cancellation confirmed (< 100 ms)
                     ▼
               ┌────────────┐
               │   READY    │  (ready for next speak())
               └────────────┘
```

---

## Error Recovery Flow

```
[ANY STATE]
      │ unhandled error / engine crash / pack verification failure
      ▼
┌───────────┐
│   ERROR   │  (error logged with full context and stack)
└─────┬─────┘
      │
      ├── recoverable error? (e.g., ONNX session crash, temp file I/O error)
      │        │
      │        ▼
      │   ┌────────────┐
      │   │ RECOVERING │  (re-initialize engine, reload session)
      │   └─────┬──────┘
      │         │
      │         ├── recovery OK?
      │         │       │
      │         │       ▼
      │         │  ┌──────────────┐
      │         │  │ INITIALIZING │ → READY
      │         │  └──────────────┘
      │         │
      │         └── recovery failed?
      │                 │
      │                 ▼
      │           ┌─────────────┐
      │           │ UNAVAILABLE │  (dispose() required)
      │           └─────────────┘
      │
      └── non-recoverable error? (e.g., model file deleted, manifest corrupted)
               │
               ▼
         ┌─────────────┐
         │ UNAVAILABLE │  (dispose() required)
         └─────────────┘
```

---

## Pause / Resume Flow

```
┌──────────┐
│ PLAYING  │
└────┬─────┘
     │ pause()
     ▼
┌──────────┐
│  PAUSED  │  (audio worklet paused, buffer held)
└────┬─────┘
     │ resume()
     ▼
┌──────────┐
│ PLAYING  │  (resumes from current buffer position)
└──────────┘
```

---

## Implementation Contract

These rules are binding on all implementations of the state machine:

### 1 — State Is Explicit

```typescript
// CORRECT: Single source of truth
private state: AudioLifecycleState = AudioLifecycleState.UNINITIALIZED;

// WRONG: Implicit state from multiple flags
private isSynthesizing: boolean = false;
private isPlaying: boolean = false;
private hasError: boolean = false;
```

There is one `state` field. It is a value from the `AudioLifecycleState` enum. It is never inferred by combining other flags.

### 2 — All Transitions Must Be Logged

Every state transition MUST emit a log at `DEBUG` level:

```
[VocalRuntime] State transition: SYNTHESIZING → BUFFERING (requestId: abc-123)
```

The log entry must include: previous state, new state, and the requestId of the current synthesis operation if applicable.

### 3 — Invalid Transitions Must Throw

An attempt to perform an action that would result in an invalid state transition MUST throw `StateTransitionError`. It MUST NOT silently succeed, silently fail, or log-and-continue.

```typescript
class StateTransitionError extends Error {
  constructor(
    public readonly currentState: AudioLifecycleState,
    public readonly attemptedTransition: string,
    public readonly trigger: string,
  ) {
    super(
      `Invalid state transition: cannot perform '${attemptedTransition}' ` +
      `from state '${currentState}' (triggered by: ${trigger})`
    );
    this.name = 'StateTransitionError';
  }
}
```

### 4 — UI Must Subscribe, Not Poll

The state machine MUST expose an observable/event interface:

```typescript
interface VocalRuntime {
  // Subscribe to state changes
  onStateChange(handler: (prev: AudioLifecycleState, next: AudioLifecycleState) => void): Unsubscribe;

  // Current state (for initial render, not for polling)
  readonly state: AudioLifecycleState;
}
```

UI components subscribe once and react to changes. There is no `setInterval` checking the state.

### 5 — Cancellation Is Always Available

`cancel()` MUST be safe to call from any state. If the state is `READY` or `UNINITIALIZED`, it MUST be a no-op (not an error). If the state is `CANCELLING`, a second `cancel()` call MUST be a no-op.

### 6 — Rapid Speak Calls Must Not Corrupt State

If `speak()` is called while the runtime is in `SYNTHESIZING`, `BUFFERING`, or `PLAYING` state:
- The new request MUST be enqueued (not immediately dispatched)
- The queue MUST be bounded (per VOICE_INV_011)
- The runtime does not abort the current synthesis to process the new request (use `cancel()` first if that is desired)

```
speak("First sentence")   →  SYNTHESIZING (immediately)
speak("Second sentence")  →  enqueued (pending READY)
speak("Third sentence")   →  enqueued or QueueFullError if at limit
```

---

## Barge-In Protocol

Barge-in is the action of stopping the current synthesis/playback immediately to allow a new input or command. It is the canonical "interrupt" operation for conversational interfaces.

**Step-by-step protocol:**

```
1. User initiates stop
   (voice activity detected by upstream VAD, or user presses Stop button in UI)
        │
        ▼
2. playback.cancel() called immediately
   (audio worklet stops rendering audio to speaker — user hears silence NOW)
        │
        ▼
3. synthesis.cancel(currentRequestId) called
   (ongoing engine inference is aborted via cancellation token)
        │
        ▼
4. requestQueue.clear() called
   (any pending speak() calls in the queue are discarded)
        │
        ▼
5. State transitions:
   PLAYING → CANCELLING → READY
   (or SYNTHESIZING → CANCELLING → READY, or BUFFERING → CANCELLING → READY)
        │
        ▼
6. Next synthesis begins fresh from READY
   (the system is now ready to process the new user input or next command)
```

**Timing requirements:**
- Step 2 (audio silence): MUST occur within one audio buffer frame (< 10 ms at 22050 Hz with typical buffer sizes)
- Step 5 (CANCELLING → READY): MUST complete within 100 ms (VOICE_INV_009)
- Step 6 (first new synthesis chunk): target < 200 ms after barge-in (user perceives this as responsiveness)

---

## Invalid Transition Table

Attempting any of the following transitions MUST throw `StateTransitionError`.

| Current State | Attempted Action | Why It Is Invalid | Error Message |
|--------------|------------------|-------------------|---------------|
| `UNINITIALIZED` | `speak()` | Model not loaded | `Cannot speak: runtime not initialized` |
| `UNINITIALIZED` | `pause()` | Nothing playing | `Cannot pause: no active playback` |
| `UNINITIALIZED` | `resume()` | Nothing paused | `Cannot resume: not in PAUSED state` |
| `INITIALIZING` | `speak()` | Model load in progress | `Cannot speak: initialization in progress` |
| `SYNTHESIZING` | `resume()` | Not in PAUSED state | `Cannot resume: not in PAUSED state` |
| `SYNTHESIZING` | `pause()` | Cannot pause during synthesis — only during playback | `Cannot pause: pause is only valid during PLAYING` |
| `BUFFERING` | `resume()` | Not in PAUSED state | `Cannot resume: not in PAUSED state` |
| `BUFFERING` | `pause()` | Cannot pause during buffering | `Cannot pause: pause is only valid during PLAYING` |
| `PAUSED` | `speak()` (direct dispatch) | Already have paused audio; must resume or cancel first | `Cannot start new synthesis while PAUSED; call resume() or cancel() first` |
| `CANCELLING` | `speak()` (direct dispatch) | Cancellation in progress; wait for READY | `Cannot speak: cancellation in progress` |
| `UNAVAILABLE` | `speak()` | Terminal failure state | `Cannot speak: runtime is UNAVAILABLE; dispose and reinitialize` |
| `UNAVAILABLE` | `initialize()` | Must dispose first | `Cannot reinitialize: call dispose() first` |
| `ERROR` | `speak()` | Error state; must wait for recovery | `Cannot speak: runtime is in ERROR state; waiting for recovery` |
| `READY` | `resume()` | Nothing paused | `Cannot resume: not in PAUSED state` |
| `READY` | `stopAfterSentence()` | Nothing playing | `Cannot stop: no active synthesis` |

---

## TypeScript State Enum Reference

```typescript
export enum AudioLifecycleState {
  UNINITIALIZED = 'UNINITIALIZED',
  INITIALIZING  = 'INITIALIZING',
  READY         = 'READY',
  SYNTHESIZING  = 'SYNTHESIZING',
  BUFFERING     = 'BUFFERING',
  PLAYING       = 'PLAYING',
  PAUSED        = 'PAUSED',
  STOPPING      = 'STOPPING',
  CANCELLING    = 'CANCELLING',
  ERROR         = 'ERROR',
  RECOVERING    = 'RECOVERING',
  UNAVAILABLE   = 'UNAVAILABLE',
}

/** States from which synthesis output is being actively produced or queued */
export const ACTIVE_STATES = new Set([
  AudioLifecycleState.SYNTHESIZING,
  AudioLifecycleState.BUFFERING,
  AudioLifecycleState.PLAYING,
]);

/** States from which cancel() has a meaningful effect */
export const CANCELLABLE_STATES = new Set([
  AudioLifecycleState.SYNTHESIZING,
  AudioLifecycleState.BUFFERING,
  AudioLifecycleState.PLAYING,
  AudioLifecycleState.PAUSED,
  AudioLifecycleState.STOPPING,
]);

/** States indicating the runtime is not usable and requires action */
export const TERMINAL_STATES = new Set([
  AudioLifecycleState.ERROR,
  AudioLifecycleState.UNAVAILABLE,
]);
```

---

*Document maintained by the Chiti Platform Team. For the invariants this state machine enforces, see [INVARIANTS.md](./INVARIANTS.md), specifically VOICE_INV_009 (Interruptibility) and VOICE_INV_010 (Streaming Safety).*
