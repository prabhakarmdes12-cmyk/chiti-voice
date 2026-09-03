# Third-party licences, and what we may therefore ship

> **This is a record of what has been verified inside this repository, not legal advice.**
> `LICENSE` at the repo root opens with the same warning, and means it: a human has to read the
> documents below before any of this ships. The purpose of this file is to make the questions
> answerable -- what we know, how we know it, and which two answers we do not yet have.

Written because `PRD.md` requires the catalogue ("Licenses documented") and because the persona work
produced something that needs the answer: a style vector blended from Kokoro's voice vectors is a
derivative work of files whose licence we have not confirmed. `scripts/verify-doc-claims.py` fails if
this file disappears while `PRD.md` still cites it.

## 1. Verified in this repository

| Component | How it arrives | Licence status | Evidence here | Ship derived artifacts? |
|---|---|---|---|---|
| chiti-voice code (this tree) | authored | draft `LICENSE`, legal review required | `LICENSE` | our own call |
| Kokoro-82M int8 ONNX graph + the 54 `*.bin` voice vectors | `scripts/fetch-offline-model.py`, pinned sha256, refuses to write without `--accept-licence`, records `SOURCE.json` | **the carrier's MIT `LICENSE` covers its code, not its model data; the weights state no licence** | `docs/research/KOKORO_OFFLINE_SPIKE.md:148`, `docs/research/PERSONA_STYLE_VECTORS.md:209` | **No.** `models/persona-*.bin` is gitignored and `VOICE_INV_008` refuses these bytes as a `real` pack |
| espeak-ng data (reached through Python `phonemizer`) | transitive, in the spike's fallback path only | `GPL-3.0-or-later`, per the carrier's own metadata | `docs/research/KOKORO_OFFLINE_SPIKE.md:88` | No, not into a permissive binary. The permissive path below is why that is not a blocker |
| `expo-open-phonemizer@1.0.1` -- 274,927-entry `en_us` lexicon + 61 MB char-level G2P graph | `scripts/extract-open-phonemizer.py` | package is MIT; **the tarball states no licence for the lexicon or the graph weights** | `docs/research/KOKORO_OFFLINE_SPIKE.md:91,94` | Only after the weights question is answered. English personas ran through it with no GPL component in the path |
| ONNX Runtime | not vendored; `ort` is a proposal, commented out in the manifest | MIT upstream, **not verified in this tree** | `crates/vocal-core/Cargo.toml` note above `[dependencies]` | n/a until Step 2 picks an engine |
| Rust crates: `tokio` `serde` `serde_json` `thiserror` `tracing` `tracing-subscriber` `hex` `async-trait` `zip` `sha2` `clap` `anyhow` | `[workspace.dependencies]` | **not audited** -- see §2 | the three `Cargo.toml` files | n/a |

Two rows say "not verified" and one says "no". That is the honest state, and it is the reason
`persona.recipes` ship as JSON with `provenance_status: incomplete by design` rather than as packed
`.cvpack` files: `docs/research/persona-recipes/*.json` are committable, and the vectors they describe
are not.

## 2. Open items, each with the thing that closes it

1. **Kokoro's weight licence.** Read the model card and the upstream repo's own licence text, and ask
   the maintainer what redistribution of derived style vectors requires. Nothing in this repository can
   settle it, and `--accept-licence` in `scripts/fetch-offline-model.py` exists precisely so that a
   human answers before bytes land on disk. Until then: derived vectors stay out of git, and the packs
   stay `status: placeholder`.
2. **Crate licence audit + copyleft check.** `cargo deny` / `cargo license` need the registry index,
   which this sandbox cannot reach, and `Cargo.lock` is not committed (it has to be generated where
   crates.io resolves). The CI job `Dependency Audit - No Cloud/LLM Dependencies` greps *manifests* for
   cloud/LLM names, which is a different question and a narrower one; `docs/architecture/INVARIANTS.md`
   now says so in the enforcement line rather than naming a script that was never written.
3. **GPL boundary in the shipped binary.** If the Hindi path ends up using espeak-ng data, that is a
   copyleft event for the whole artifact, not a dependency detail. `docs/research/PERSONA_STYLE_VECTORS.md`
   §3 records the decision as still open.

## 3. What this document deliberately does not claim

- It does not say the derived blends are shippable. It says the opposite, and why.
- It does not assert a licence for any Rust crate. Naming MIT here without reading each crate's
  `LICENSE` file would be the same kind of claim this repository has spent a branch removing.
- It does not treat "the carrier is MIT" as covering the data the carrier publishes. That inference is
  the trap the whole file exists to keep in front of us.
