#!/usr/bin/env python3
"""Regenerate the `persona` block of each voice pack from its spec doc and its derived recipe.

    python3 scripts/sync-persona-manifests.py            # rewrite in place
    python3 scripts/sync-persona-manifests.py --check    # fail if the packs drifted (CI-able)

Why a script and not hand-editing the manifests: the persona tables in `docs/personas/*.md` are the
spec, `docs/research/persona-recipes/*.json` are what was actually measured off the engine, and
`voice-packs/*/manifest.json` is what ships. Three copies of the same numbers is one copy too many —
this keeps one of them derived.

It also mirrors the Rust validator's rules (`crates/voice-pack/src/manifest.rs`) *before* writing, so a
pack that could never load is caught here instead of in CI:

  * `default_rate` and every intent rate inside 0.5..=1.6 — the band the `speed` input was measured in
  * a non-zero `default_pitch` only with `pitch_baked_into_style: true`; never 1.0 (that is the
    multiplier/offset mix-up); no per-intent pitch at all
  * `energy` as 0..=1, since it becomes a dBFS offset, not a multiplier
  * `loudness` with a target in -40..=-6 dBFS, a peak ceiling below full scale, and a bounded gain
  * `style` exactly one of `source_voice` / `blend` / `embedded_file`, blend weights in 0..=1 summing to 1
  * pronunciation overrides keyed by a single alphanumeric word

The `pitch`/`energy`/`warmth`/`expressiveness` rows the specs also carry are read, and the ones with no
implementation are *not* invented into the manifest: warmth and expressiveness are recorded in the
recipe and the research note instead, because a slot that silently does nothing is worse than an absent
one.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PERSONA_DOC = {"tara": "TARA", "kashi": "KASHI", "bobo": "BOBO"}
# The persona's own name, mispronounced by the permissive graph ("Chiti" -> /tʃiːti/), is a product
# bug, so the fix ships with the pack. Verified encodable in tokenizer.json's 115-symbol IPA table.
CHITI_IPA = "ˈtʃɪti"
# Which recipe a pack's cast comes from. Bobo is the one place the research and the spec disagree on
# purpose: `bobo.json` is the three-way blend, kept as *evidence* that blending attenuates movement,
# while `docs/personas/BOBO.md` concludes the honest cast is the single wide-range voice, which is
# what `bobo-solo.json` describes. The pack ships the conclusion, not the experiment.
RECIPE_FOR = {"bobo": "bobo-solo"}
RATE_BAND = (0.5, 1.6)
PAUSE_BAND = (0.5, 3.0)


def die(msg: str) -> "None":
    raise SystemExit(f"sync-persona-manifests: {msg}")


def base_table(md: str, persona: str) -> dict[str, float]:
    """`| Speed | 1.15 | … |` rows from the persona's Base Personality table."""
    out: dict[str, float] = {}
    for key in ("Speed", "Pitch", "Energy", "Warmth", "Expressiveness"):
        m = re.search(rf"^\|\s*{key}\s*\|\s*([+-]?[\d.]+)\s*\|", md, re.M)
        if not m:
            die(f"{persona}: no `| {key} |` row in docs/personas/{PERSONA_DOC[persona]}.md")
        out[key.lower()] = float(m.group(1))
    return out


def intent_table(md: str, persona: str) -> dict[str, dict[str, float]]:
    body = md.split("## Intent Profiles", 1)
    if len(body) != 2:
        die(f"{persona}: docs/personas/{PERSONA_DOC[persona]}.md has no `## Intent Profiles` section")
    rows: dict[str, dict[str, float]] = {}
    for line in body[1].splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 5 or not re.fullmatch(r"[A-Z][A-Z_]*", cells[0]):
            continue
        try:
            speed, energy = float(cells[1]), float(cells[2])
        except ValueError:
            continue
        rows[cells[0]] = {"speed": speed, "energy": energy}
    if not rows:
        die(f"{persona}: the Intent Profiles table parsed empty")
    return rows


def blend_of(recipe: dict) -> list[dict]:
    sources = recipe["sources"]
    if len(sources) == 1:
        return []
    return [{"voice": s["voice"], "weight": round(float(s["weight"]), 4)} for s in sources]


def check(name: str, cond: bool, msg: str) -> None:
    if not cond:
        die(f"{name}: {msg}")


def build_persona(persona: str, md: str, recipe: dict, existing: dict) -> dict:
    base, intents = base_table(md, persona), intent_table(md, persona)
    style = recipe["sources"]
    loud = recipe.get("loudness") or {}

    check(persona, RATE_BAND[0] <= base["speed"] <= RATE_BAND[1], f"base Speed {base['speed']} outside the measured rate band")
    check(persona, -40.0 <= loud.get("target_dbfs", -20.0) <= -6.0, "loudness target outside -40..=-6 dBFS")
    check(persona, 0.0 < loud.get("peak_ceiling", 0.98) <= 0.999, "peak ceiling must leave headroom")
    check(persona, 0.0 < loud.get("max_gain_db", 12.0) <= 24.0, "max gain must be bounded")
    check(persona, base["pitch"] != 1.0, "pitch 1.0 is the multiplier/offset mix-up; neutral is 0.0")
    check(persona, all(0.0 <= v["energy"] <= 1.0 for v in intents.values()), "energy must be 0..=1")
    check(persona, all(RATE_BAND[0] <= v["speed"] <= RATE_BAND[1] for v in intents.values()), "an intent rate is outside 0.5..=1.6")
    total = sum(float(s["weight"]) for s in style)
    check(persona, abs(total - 1.0) <= 1e-3, f"blend weights sum to {total}, not 1.0")
    check(persona, 1 <= len(style) <= 8, "a cast needs 1..=8 source voices")
    check(persona, len({s["voice"] for s in style}) == len(style), "a voice appears in the blend twice")

    # A register the spec asks for that the graph cannot take as input is only honest when the cast
    # itself carries it. `pitch_baked_into_style` says which of the two situations the pack is in.
    pitch_baked = abs(base["pitch"]) > 1e-9
    block: dict = {
        "id": persona,
        "display_name": existing.get("display_name") or PERSONA_DOC[persona].title(),
        "description": existing.get("description", ""),
        "language": existing.get("language", "en-IN"),
        "default_rate": round(base["speed"], 3),
        "default_pitch": round(base["pitch"], 3),
        "pitch_baked_into_style": pitch_baked,
        "style": {
            # Exactly one of these. A lone stock voice is a `source_voice`; a mix is a `blend`;
            # `embedded_file` is for a pack that carries the 522,240-byte vector itself.
            **({"blend": blend_of(recipe)} if blend_of(recipe) else {"source_voice": style[0]["voice"]}),
        },
        "loudness": {
            "target_dbfs": round(float(loud.get("target_dbfs", -20.0)), 1),
            "peak_ceiling": round(float(loud.get("peak_ceiling", 0.98)), 3),
            "max_gain_db": round(float(loud.get("max_gain_db", 12.0)), 1),
        },
        "pronunciation_overrides": {"chiti": CHITI_IPA},
        "intent_profiles": {
            name: {
                "rate": round(v["speed"], 3),
                # No per-intent pitch: the engine has no pitch input, so a spec's "raise the pitch
                # 40 cents on WARNING" cannot be honoured and must not be written down as if it could.
                "pitch": 0.0,
                "energy": round(v["energy"], 3),
                # The spec tables carry no pause column, so this stays at the neutral value rather
                # than being invented from the "Notes" prose.
                "pause_factor": 1.0,
            }
            for name, v in intents.items()
        },
    }
    return block


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--packs", default="voice-packs", help="directory holding <persona>/manifest.json")
    ap.add_argument("--recipes", default="docs/research/persona-recipes")
    ap.add_argument("--check", action="store_true", help="exit non-zero if any pack's persona block is stale")
    args = ap.parse_args()

    packs_dir, recipes_dir = Path(args.packs), Path(args.recipes)
    if not packs_dir.is_absolute():
        packs_dir, recipes_dir = ROOT / args.packs, ROOT / args.recipes

    changed = 0
    for manifest_path in sorted(packs_dir.glob("*/manifest.json")):
        persona = manifest_path.parent.name
        if persona not in PERSONA_DOC:
            continue  # fixture/demo packs are not persona specs
        md = (ROOT / "docs" / "personas" / f"{PERSONA_DOC[persona]}.md").read_text(encoding="utf-8")
        recipe_path = recipes_dir / f"{RECIPE_FOR.get(persona, persona)}.json"
        if not recipe_path.exists():
            die(f"{persona}: no {recipe_path}; run scripts/derive-persona-style.py first")
        recipe = json.loads(recipe_path.read_text(encoding="utf-8"))
        doc = json.loads(manifest_path.read_text(encoding="utf-8"))
        block = build_persona(persona, md, recipe, doc.get("persona") or {})

        # A pack that ships a persona must also ship the thing the persona points at. Placeholder
        # packs get that rule waived, so this script can run before the models exist.
        # `source_voice` / `blend` resolve against the *model directory*, so a pack naming stock
        # voices declares no files for them; only `embedded_file` has to be in `files` (the Rust
        # validator enforces that). This is a reminder, not a defect.
        if doc.get("persona", {}).get("style", {}).get("embedded_file"):
            declared = {f["path"] for f in doc.get("files", [])}
            emb = doc["persona"]["style"]["embedded_file"]
            if emb not in declared:
                die(f"{persona}: embedded style file {emb!r} is not in files[] — the loader would reject the pack")

        if doc.get("persona") == block:
            print(f"  {persona:6} persona block already derived from spec+recipe ✓")
            continue
        doc["persona"] = block
        changed += 1
        if args.check:
            print(f"  {persona:6} STALE — regenerate without --check", file=sys.stderr)
            continue
        manifest_path.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"  {persona:6} persona block rewritten ({len(block['intent_profiles'])} intents, "
              f"{len(block['style'].get('blend', [])) or 1} style source(s))")

    print(f"{'would rewrite' if args.check and changed else 'rewrote'} {changed} manifest(s)" if changed else "all packs in sync")
    return 1 if (args.check and changed) else 0


if __name__ == "__main__":
    sys.exit(main())
