#!/usr/bin/env python3
"""Fail when a document asserts, in the present tense, something the tree contradicts.

This repository was founded on a defect of exactly this shape: `Cargo.toml` declared an example file
that did not exist, `docs` described a daemon and an SDK that were never written, and a CI job ran
`test -f tara.cvpack` and reported "three voices load successfully". Those were fixed. This script is
what keeps them fixed, because a corrected sentence can regress as easily as the original could.

Four rules, each narrow on purpose -- a broad "docs must match code" checker is unfalsifiable and gets
deleted, which is the second way a gate dies:

1. Every relative markdown link resolves to a real path.
2. A backticked path that looks like it lives in this repo resolves, unless the sentence says it is
   absent, removed, planned or proposed. Naming a missing file *as* missing is honest documentation and
   the loudest lesson from the offline work: `ops/ci/README.md` has to be allowed to say that the
   capture scripts are gone. Naming them as though they run is not.
3. A grep/test command documented as enforcement must point at a path that exists. A check whose target
   is absent passes trivially, which is worse than no check: it manufactures confidence. (INVARIANTS.md
   documented `grep ... crates/chiti-vocal-core/src/` for months; that directory has not existed under
   that name since the crates were renamed.)
4. `docs/api/*.md` must keep their STATUS banner, and `REAL_SYNTHESIS_AVAILABLE` in `vocal-core` must
   agree with what the README's headline row says. The two claims in this repo that a reader is most
   likely to act on are bound to source, not to luck.

Paths that legitimately do not exist yet (planned tests cited by an invariant's verification plan, an
unbuilt SDK) are listed in PLANNED with a reason. That list is checked in both directions: an entry that
starts existing is an error too, so the allowlist cannot silently accumulate.

Run: python3 scripts/verify-doc-claims.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Extensions that name a file in this repo, as opposed to a word that happens to contain a dot.
KINDS = "rs|py|toml|json|md|wav|sh|ps1|bin|cvpack|onnx|txt|lock|ts|yml|yaml|dart|csv"

# A backticked path. Requires a directory separator, or a repo-root filename with an extension that
# only ever appears at the root (Cargo.lock et al. are handled by the negation rule anyway).
PATH_TOKEN = re.compile(r"`([A-Za-z0-9_.~/-]*(?:/[A-Za-z0-9_.~*-]*)+\.(?:" + KINDS + r"))`")
# Any dir-qualified path with a repo extension counts, not just the ones starting at a known
# top-level directory: `persona-recipes/bobo.json` in docs/personas/BOBO.md pointed at the blend
# recipe while the manifest derives from bobo-solo.json, and a top-level-prefix rule cannot see that.
ANY_PATH = re.compile(r"`([A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.~*-]+)+\.(?:" + KINDS + r"))`")
TOP = r"crates|apps|scripts|docs|ops|assets|voice-packs|models|tests|src|packages|examples"
DIR_TOKEN = re.compile(r"`((?:" + TOP + r")/[A-Za-z0-9_./*-]*(?:/[A-Za-z0-9_./*-]*)*)`")
COMMAND_WORDS = re.compile(r"\b(?:grep|test -f|test -e|cat|head|diff|sha256sum)\b")

MD_LINK = re.compile(r"\[[^\]]*\]\(([^)\s]+)")
GREP_TARGET = re.compile(
    r"(?:grep|test -f|test -e|cat|head)\s[^`|&;]*?\s([A-Za-z0-9_./*-]+(?:\.(?:" + KINDS + r")|/(?=[`:\s)])))"
)

# Sentences that speak in these registers are describing intent or history, not asserting existence.
NEGATIONS = (
    "absent", "does not exist", "do not exist", "did not", "doesn't", "don't", "no longer",
    "not exist", "missing", "removed", "deleted", "retired", "gone", "never", "planned", "proposal",
    "proposed", "to be", "not yet", "would", "superseded", "no such", "nothing", "not implemented",
    "has been removed", "were deleted", "was deleted", "is gone", "unbuilt", "unverified",
    "remains open", "ignored", "not committed", "no such", "none of", "neither",
)

# Paths an author has declared as intentionally-not-here, with the reason that makes it legitimate.
PLANNED: dict[str, str] = {
    "tests/determinism_test.rs": "invariant verification plan (VOICE_INV_003); test not yet written",
    "tests/degradation_test.rs": "invariant verification plan (VOICE_INV_005); test not yet written",
    "tests/interruptibility_test.rs": "invariant verification plan (VOICE_INV_009); test not yet written",
    "tests/offline_test.rs": "invariant verification plan (VOICE_INV_001); test not yet written",
    "tests/pack_verify_test.rs": "invariant verification plan (VOICE_INV_008); test not yet written",
    "tests/persona_independence_test.rs": "invariant verification plan; test not yet written",
    "tests/resource_limits_test.rs": "invariant verification plan (VOICE_INV_011); test not yet written",
    "tests/stream_safety_test.rs": "invariant verification plan (VOICE_INV_010); test not yet written",
    "tests/version_compat_test.rs": "invariant verification plan (VOICE_INV_012); test not yet written",
    "packages/chiti-voice-sdk/src/types.ts": "the SDK is specified in docs/api, not built",
    "docs/CVPACK_SPECIFICATION.md": "documented as a planned companion to the .cvpack format",
}

SKIP_DIRS = {".git", "target", "node_modules", ".cargo", "dist"}


def tracked_files() -> list[Path]:
    files = []
    for p in ROOT.rglob("*"):
        if not p.is_file():
            continue
        rel_parts = p.relative_to(ROOT).parts
        if any(part in SKIP_DIRS for part in rel_parts):
            continue
        files.append(p.relative_to(ROOT))
    return files


def has_negation(line: str) -> bool:
    low = line.lower()
    return any(word in low for word in NEGATIONS)


def main() -> int:
    files = tracked_files()
    suffixes = {str(f) for f in files}
    # A doc may cite `tests/dsp_parity.rs` when the file lives at
    # `crates/vocal-core/tests/dsp_parity.rs`: crate-relative shorthand is how these documents are
    # written, so resolve by suffix rather than forcing every citation to spell the whole path.
    resolves = lambda token: token in suffixes or any(s.endswith("/" + token) for s in suffixes)

    problems: list[str] = []
    seen_tokens_per_line: dict = {}

    for f in files:
        if f.suffix != ".md":
            continue
        text = (ROOT / f).read_text(encoding="utf-8", errors="replace")
        lines = text.splitlines()
        # Negation is judged per paragraph, not per line: these documents wrap at ~90 columns, so a
        # sentence routinely spans four lines and the words that make it honest ("no longer in the
        # tree") land in a different one than the path they qualify. A line-scoped rule would flag
        # correct prose until people switched the check off, and a switched-off check is the bug this
        # script exists to prevent.
        para_start, para_text = [], []
        starts = [0] + [i + 1 for i, l in enumerate(lines) if not l.strip() and i + 1 < len(lines)]
        for k, st in enumerate(starts):
            en = starts[k + 1] if k + 1 < len(starts) else len(lines)
            chunk = "\n".join(lines[st:en])
            para_text.append(chunk)
            para_start.append(st)

        def paragraph_of(idx: int) -> str:
            chosen = para_text[0]
            for k, st in enumerate(para_start):
                if st <= idx:
                    chosen = para_text[k]
            return chosen


        # Rule 1: markdown links.
        for i, line in enumerate(lines, 1):
            for m in MD_LINK.finditer(line):
                url = m.group(1)
                if url.startswith(("http://", "https://", "mailto:", "#")):
                    continue
                target = url.split("#")[0]
                if not target:
                    continue
                if not (f.parent / target).resolve().is_relative_to(ROOT.resolve()):
                    problems.append(f"{f}:{i}: markdown link escapes the repo: {url}")
                elif not (ROOT / f.parent / target).exists():
                    problems.append(f"{f}:{i}: broken markdown link: {url}")

        # Rules 2 and 3, as one token scan: any backticked path under a top-level project directory
        # must resolve, whether it names a file or a directory. Directory-shaped tokens matter as much
        # as file-shaped ones -- that is how `crates/chiti-vocal-core/src/` slipped past the first
        # version of this script, and a grep whose target is absent is the exact theatre this repo is
        # trying to stop producing.
        for i, line in enumerate(lines, 1):
            if "`" not in line:
                continue
            for m in list(PATH_TOKEN.finditer(line)) + list(ANY_PATH.finditer(line)) + list(DIR_TOKEN.finditer(line)):
                token = m.group(1)
                if token in seen_tokens_per_line.setdefault((f, i), set()):
                    continue
                seen_tokens_per_line[(f, i)].add(token)
                if token.startswith("~"):
                    continue
                glob = "*" in token
                check = token.split("*")[0].rstrip("/") or "."
                target = ROOT / check
                if glob:
                    ok = target.exists()
                elif target.is_dir():
                    ok = True
                else:
                    ok = token in suffixes or any(s.endswith("/" + token) for s in suffixes)
                if ok:
                    continue
                if has_negation(paragraph_of(i - 1)) or has_negation(line):
                    continue  # naming an absent file as absent is the honest case
                if token in PLANNED:
                    continue
                if COMMAND_WORDS.search(line):
                    problems.append(
                        f"{f}:{i}: a documented check targets `{token}`, which does not exist -- that "
                        "command passes trivially and enforces nothing"
                    )
                else:
                    problems.append(
                        f"{f}:{i}: document cites `{token}` as present, and no such path exists "
                        "(rewrite it in the past/negative tense, or add it to PLANNED with a reason)"
                    )

    # Rule: PLANNED is checked in both directions, so it cannot rot into a hidey-hole.
    for token, why in PLANNED.items():
        if resolves(token):
            problems.append(
                f"PLANNED entry `{token}` now exists in the tree ({why}) -- drop the entry"
            )

    # Rule 4a: the api specs keep their banner.
    api = ROOT / "docs" / "api"
    if api.is_dir():
        for spec in sorted(api.glob("*.md")):
            head = "\n".join(spec.read_text(encoding="utf-8", errors="replace").splitlines()[:12])
            if "STATUS:" not in head or "NOT IMPLEMENTED" not in head:
                problems.append(
                    f"{spec.relative_to(ROOT)}: a spec for surface that is not built must state "
                    "`STATUS: ... NOT IMPLEMENTED` in its first 12 lines"
                )

    # Rule 4b: the headline capability claim is read from source, not from prose.
    lib = ROOT / "crates" / "vocal-core" / "src" / "lib.rs"
    readme = ROOT / "README.md"
    if lib.is_file() and readme.is_file():
        m = re.search(r"REAL_SYNTHESIS_AVAILABLE[^=]*=\s*(?::\s*\w+\s*)?(:?\s*)?(true|false)", lib.read_text(encoding="utf-8"))
        flag = m.group(2) if m else None
        says_false = "REAL_SYNTHESIS_AVAILABLE` is `false" in readme.read_text(encoding="utf-8")
        if flag == "true" and says_false:
            problems.append(
                "README says REAL_SYNTHESIS_AVAILABLE is false, but crates/vocal-core/src/lib.rs "
                "sets it true -- update the headline row in the same commit that flips the constant"
            )

    if problems:
        print(f"verify-doc-claims: {len(problems)} stale or unverifiable claim(s)\n", file=sys.stderr)
        for p in problems:
            print(f"  {p}", file=sys.stderr)
        return 1
    print("verify-doc-claims: every documented path claim resolves, and the checked claims match source")
    return 0


def self_test() -> int:
    """Prove the gate can fail. This script's own history is the reason the flag exists: a rewritten
    version of it lost its file loop and reported a clean tree in under a second, which is the exact
    defect it is supposed to catch -- every other check in this repository has died the same way (a
    grep whose target path had been renamed, an `echo` standing in for an assertion, a wrapper whose
    body was deleted). So the gate ships with a canary: a planted false claim must fail, the same claim
    written honestly must pass, and a stale PLANNED entry must fail. Run it in CI before the real scan."""
    import shutil
    import subprocess
    import tempfile

    here = Path(__file__).resolve()
    planted_bad = (
        "# Notes\n\n- The audit runs `scripts/totally-made-up.sh` on every commit and blocks the build.\n"
    )
    planted_good = (
        "# Notes\n\n- The audit used to run `scripts/totally-made-up.sh`; that script was deleted and\n"
        "  no longer exists, so the check is the CI job itself.\n"
    )
    cases = [
        ("planted false claim is caught", {"docs/notes.md": planted_bad}, 1),
        ("honest past-tense citation is allowed", {"docs/notes.md": planted_good}, 0),
        (
            "stale PLANNED entry is caught",
            {"docs/notes.md": "# Notes\n", "tests/offline_test.rs": "// now it exists\n"},
            1,
        ),
    ]
    failures = 0
    for name, tree, want in cases:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            (root / "scripts").mkdir(parents=True)
            shutil.copy(here, root / "scripts" / here.name)
            for rel, body in tree.items():
                dst = root / rel
                dst.parent.mkdir(parents=True, exist_ok=True)
                dst.write_text(body, encoding="utf-8")
            got = subprocess.run(
                [sys.executable, str(root / "scripts" / here.name)],
                capture_output=True, text=True,
            ).returncode
        if got == want:
            print(f"  self-test ok: {name}")
        else:
            failures += 1
            print(f"  SELF-TEST FAILED: {name}: expected exit {want}, got {got}", file=sys.stderr)
    if failures:
        print(
            "::error::verify-doc-claims.py cannot distinguish a true claim from a false one; "
            "do not trust its other output", file=sys.stderr,
        )
    return 1 if failures else 0


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        sys.exit(self_test())
    sys.exit(main())
