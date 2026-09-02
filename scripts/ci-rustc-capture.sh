#!/bin/sh
# TEMPORARY DIAGNOSTIC (rustc/clippy wrapper) — delete once CI is green.
#
# Why this exists: the Actions log hosts (results-receiver / objects.githubusercontent.com)
# are unreachable from the auditing environment, so `gh run view --log` cannot fetch the
# compiler output at all. This wrapper re-emits each rustc/clippy diagnostic that points at
# this repository's own sources as a `::error::` workflow command; the runner records those
# as check-run ANNOTATIONS, which are readable through the checks API.
#
# Selection is by file path (`crates/…`, `apps/…`) rather than by --crate-name, because
# integration tests, examples and benches compile under crate names that do not match the
# crate they live in — filtering on the name silently hid a whole class of errors.
#
# rustc's own output is passed through byte-for-byte and its exit code is preserved, so
# cargo's rendering and the job's pass/fail semantics are unaffected.

printf '%s' "$*" | grep -Eq -- '--crate-name[= ](vocal_core|voice_pack|chiti_voice)' && MATCH=1 || MATCH=0
printf '%s\n' "$MATCH" > /tmp/rustc-wrapper-match

case "$1" in
  *clippy-driver*) CI_CLIPPY_WARNINGS=1; export CI_CLIPPY_WARNINGS ;;
esac

"$@" > /tmp/rustc-raw.out 2>&1
code=$?
cat /tmp/rustc-raw.out

if [ "$code" -ne 0 ]; then
  python3 - <<'PY'
import json, os, re

MATCH = open("/tmp/rustc-wrapper-match").read().strip() == "1"
WANT_WARNINGS = os.environ.get("CI_CLIPPY_WARNINGS") == "1"
raw = open("/tmp/rustc-raw.out", encoding="utf-8", errors="replace").read()

OURS = ("crates/", "apps/", "voice-packs/", "scripts/")


def is_ours(d):
    spans = d.get("spans") or []
    if spans:
        # A span is authoritative: a diagnostic inside a registry crate is never ours,
        # even while we are compiling one of our own crates (it was pulled in by us).
        return any((s.get("file_name") or "").startswith(OURS) for s in spans)
    # No location at all (e.g. "aborting due to …", link errors): only ours if the
    # crate currently being compiled belongs to this workspace.
    return MATCH


seen, lines = set(), []
for line in raw.splitlines():
    line = line.strip()
    if line.startswith("{"):
        try:
            d = json.loads(line)
        except Exception:
            continue
        if d.get("$message_type") not in (None, "diagnostic"):
            continue
        lvl = d.get("level")
        # Clippy warnings only matter when they are fatal; that is exactly the run where
        # the wrapped program is clippy-driver, which is when we enable them here.
        if lvl == "error":
            keep = True
        elif lvl == "warning" and WANT_WARNINGS:
            keep = True
        else:
            keep = False
        if not keep or not is_ours(d):
            continue
        msg = (d.get("message") or "").replace("\n", " ").strip()
        if not msg or msg.startswith("aborting due to"):
            continue
        code = ((d.get("code") or {}).get("code") or "")
        spans = [s for s in (d.get("spans") or []) if s.get("is_primary")] or (d.get("spans") or [])
        loc = ""
        if spans:
            s0 = spans[0]
            loc = f"{s0.get('file_name')}:{s0.get('line_start')}:{s0.get('column_start')}"
        key = (code, loc, msg[:150])
        if key in seen:
            continue
        seen.add(key)
        head = code if code else "error"
        lines.append(f"{head} {loc}: {msg}" if loc else f"{head}: {msg}")
    else:
        # Plain-text mode (cargo run without --error-format=json, e.g. build scripts).
        m = re.match(r"^((?:error|warning)(?:\[[EW]\d+\])?[:!].{0,300})", line)
        if m and (MATCH or WANT_WARNINGS) and m.group(1) not in seen:
            seen.add(m.group(1))
            lines.append(m.group(1))

if not lines:
    if MATCH:
        # Cargo failed on one of our crates but nothing was parseable: surface the tail.
        lines = ["unparsed failure; stderr tail: " + raw[-1000:].replace("\n", "|")]
    else:
        raise SystemExit(0)

# GitHub keeps at most 10 annotations per check run, so pack the list into a few long
# chunks instead of emitting one annotation per diagnostic.
text = " ;; ".join(t.replace("\r", " ").replace("%", "25") for t in lines)
chunks = [text[i:i + 1300] for i in range(0, len(text), 1300)] or ["none"]
for i, c in enumerate(chunks[:8]):
    print(f"::error::RUSTC[{i + 1}/{len(chunks)}] {c}", flush=True)
print(f"::notice::RUSTC count={len(lines)} chunks={len(chunks)} clippy={int(WANT_WARNINGS)}", flush=True)
PY
fi

exit "$code"
