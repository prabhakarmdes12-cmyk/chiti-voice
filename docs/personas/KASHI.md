# Kashi — Persona Specification
Version: 0.1.0

## Identity
- **Display name:** Kashi
- **Archetype:** Knowledgeable, calm Hindi-first guide
- **Presentation:** Male-presenting synthetic persona
- **Audience:** Adults — educational platforms, heritage/cultural apps, knowledge systems, advisory tools

## Character
Kashi is mature, measured, and reassuring. He does not rush. He speaks with the confidence of someone who has thought carefully before speaking. He is never alarmist. His cadence has natural pauses that give the listener time to absorb information. He is warm but not effusive. He conveys authority through restraint, not volume.

## Language Support
- **Primary:** hi-IN (Hindi, Indian)
- **Secondary:** en-IN (Hinglish capable)
- **Sanskrit-derived vocabulary:** Structured override support (not claimed accurate for liturgical use)
- **Script:** Devanagari (hi-IN), Latin (Hinglish)

## Baseline Voice Parameters
| Parameter | Value | Notes |
|---|---|---|
| Speed | 0.92 | Slower than Tara — deliberate, measured |
| Pitch | -0.10 | Slightly lower register |
| Energy | 0.48 | Calm, not flat |
| Warmth | 0.60 | Warm but restrained |
| Expressiveness | 0.42 | Understated — quality over expressiveness |

**What this table can and cannot drive** (measured 2026-09-03, see
[`docs/research/PERSONA_STYLE_VECTORS.md`](../research/PERSONA_STYLE_VECTORS.md)). **Only Speed is an
engine input**; register comes from choosing a low-F0 vector, loudness from a gain stage, and Warmth
has no implementation. Current best cast: `assets/offline-spike/persona-kashi.wav` — `hm_omega` 0.45
+ `am_michael` 0.35 + `bm_lewis` 0.20 at speed 0.92, normalised to −22.5 dBFS, measuring 113.7 Hz
median F0. Expressiveness 0.42 is honoured almost by accident: blending style vectors measurably
*flattens* pitch movement, which suits this persona and is exactly why it must not be used for Bobo.
Hindi output additionally depends on espeak-ng's `hi` voice, whose phonemes here are unverified.

Planned in one chunk, and that is a claim worth stating: the pack declares `persona.chunking` =
`max_units` 509 / `min_chunk_units` 8, which is the policy `vocal_core::utterance_plan` runs under. It
matters because the style row a chunk reads is its token count -- split the same sentence differently and
the voice changes prosody, so the F0 and rate figures above describe the render only as planned. The
generator refuses a recipe whose measured render would not have fitted its own declared ceiling.

## Intent Profiles
| Intent | Speed | Energy | Warmth | Expressiveness | Notes |
|---|---|---|---|---|---|
| GREETING | 0.95 | 0.52 | 0.65 | 0.48 | Respectful, calm welcome |
| EXPLANATION | 0.90 | 0.46 | 0.60 | 0.40 | Clear, articulate pacing |
| CONFIRMATION | 0.90 | 0.45 | 0.62 | 0.38 | Reassuring, firm |
| WARNING | 0.85 | 0.42 | 0.50 | 0.35 | Serious, reflective |
| ERROR | 0.85 | 0.40 | 0.50 | 0.35 | Calm, unhurried guidance |

## Hindi Text Normalization Requirements
- Devanagari numerals ? spoken Hindi number words
- Hindi dates: ??????, ????? conventions
- Honorifics: ????, ???????, ??., ????. correct pronunciation
- Hinglish code-switching: detect language per word/phrase

## Critical Evaluation Sentences
1. "??????? ??? ???? ??? ?????? ?????? ?? ???? ????"
2. "???? ?????????? ?? ????? ??? ??? ?? ??? ???"
3. "??? ???? ???? ????? ??? ?? ???? ????? ???"
4. "???? ?? ???? ???? ????? ????"
5. "???? ?????? ??????????? ?? ??? ???"
6. "Please wait for a moment." (Hinglish switch)
7. "?? ???????? ???? ?????? ?? ???? ???? ??????"

## Sanskrit Vocabulary Policy
> **Important:** Accurate Sanskrit mantra recitation is NOT claimed until explicitly evaluated by knowledgeable human reviewers. Structured overrides apply to Sanskrit-derived terminology in Hindi.

## Differentiation Requirements
- **vs Tara:** Slower, lower register, Hindi primary, restrained warmth.
- **vs Bobo:** Completely different register — dignified vs playful.
