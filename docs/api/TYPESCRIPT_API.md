# Chiti Voice Web SDK — TypeScript API Reference
Package: `@chiti/voice-web`
Version: 0.1.0

## Installation
```bash
npm install @chiti/voice-web
```

## Quick Start
```ts
import { ChitiVoice } from '@chiti/voice-web';

const tara = await ChitiVoice.load({ voice: 'tara' });
await tara.speak('Welcome. How may I help you?');
```

## ChitiVoice.load()
```ts
static load(options: VoiceLoadOptions): Promise<VoiceInstance>
```

### VoiceLoadOptions
```ts
interface VoiceLoadOptions {
  voice: string;
  execution?: 'auto' | 'local' | 'browser';
  localServiceUrl?: string;
  timeout?: number;
  fallback?: boolean;
}
```

## VoiceInstance API
- `speak(text: string, options?: SpeakOptions): Promise<void>`
- `stop(): void`
- `pause(): void`
- `resume(): void`
- `isSpeaking(): boolean`
- `setStyle(style: string): void`
- `setIntent(intent: VoiceIntent): void`
- `getState(): AudioState`
- `getCapabilities(): EngineCapabilities`
- `addPronunciation(entry: PronunciationEntry): void`
- `destroy(): Promise<void>`

## SpeakOptions
```ts
interface SpeakOptions {
  intent?: VoiceIntent;
  style?: string;
  controls?: {
    speed?: number;       // 0.5–2.0
    pitch?: number;       // -1.0 to 1.0
    energy?: number;      // 0.0–1.0
    warmth?: number;      // 0.0–1.0
    expressiveness?: number; // 0.0–1.0
  };
  interruptible?: boolean;
  requestId?: string;
}
```

## VoiceIntent Enum
`NEUTRAL`, `GREETING`, `QUESTION`, `EXPLANATION`, `ACKNOWLEDGEMENT`, `CONFIRMATION`, `WARNING`, `ERROR`, `THINKING`, `CELEBRATION`, `GOODBYE`, `CEREMONIAL`, `WHISPER`, `ANNOUNCEMENT`, `NAVIGATION`

## Events
```ts
tara.on('ready', () => {})
tara.on('start', (requestId: string) => {})
tara.on('chunk', (chunk: AudioChunk) => {})
tara.on('end', (requestId: string) => {})
tara.on('error', (error: ChitiVoiceError) => {})
tara.on('state', (state: AudioState) => {})
```

## React Hook Integration Example
```tsx
import { ChitiVoice, VoiceInstance } from '@chiti/voice-web';
import { useEffect, useState } from 'react';

export function useChitiVoice(voiceName: string) {
  const [voice, setVoice] = useState<VoiceInstance | null>(null);
  const [speaking, setSpeaking] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    let instance: VoiceInstance;
    ChitiVoice.load({ voice: voiceName })
      .then(v => {
        instance = v;
        v.on('start', () => setSpeaking(true));
        v.on('end', () => setSpeaking(false));
        v.on('error', setError);
        setVoice(v);
      })
      .catch(setError);

    return () => { instance?.destroy(); };
  }, [voiceName]);

  return { voice, speaking, error };
}
```
