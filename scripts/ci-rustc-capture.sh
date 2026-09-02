#!/bin/sh
# TEMPORARY DIAGNOSTIC (rustc wrapper) — delete once CI is green.
#
# Why: the Actions log hosts are unreachable from the auditing environment, so
# `gh run view --log` cannot fetch compiler output. This wrapper re-emits each rustc
# error as its own `::error::` workflow command; the runner records one check-run
# ANNOTATION per error, and annotations are readable via the checks API.
#
# Only this workspace's own crates are annotated. Exit codes are preserved and rustc's
# own output is passed through untouched, so cargo's rendering is unaffected.

printf '%s' "$*" | grep -Eq -- '--crate-name[= ](vocal_core|voice_pack|chiti_voice)' && MATCH=1 || MATCH=0

"$@" > /tmp/rustc-raw.out 2>&1
code=$?
cat /tmp/rustc-raw.out

if [ "$code" -ne 0 ] && [ "$MATCH" -eq 1 ]; then
  python3 - <<'PY'
import json, os, re, sys

raw = open("/tmp/rustc-raw.out", encoding="utf-8", errors="replace").read()
seen, emitted = set(), 0
lines = []
for line in raw.splitlines():
    line = line.strip()
    if line.startswith("{"):
        try:
            d = json.loads(line)
        except Exception:
            continue
        if d.get("level") != "error":
            continue
        msg = (d.get("message") or "").replace("\n", " ")
        code = ((d.get("code") or {}).get("code") or "")
        spans = [s for s in (d.get("spans") or []) if s.get("is_primary")] or (d.get("spans") or [])
        loc = ""
        if spans:
            s0 = spans[0]
            loc = f"{s0.get('file_name')}:{s0.get('line_start')}:{s0.get('column_start')}"
        key = (code, loc, msg[:120])
        if key in seen:
            continue
        seen.add(key)
        lines.append(f"{code} {loc}: {msg}" if loc else f"{code}: {msg}")
    else:
        m = re.match(r"^(error(?:\[E\d+\])?[:!].{0,300})", line)
        if m and m.group(1) not in seen:
            seen.add(m.group(1))
            lines.append(m.group(1))

if not lines:
    lines = ["no diagnostic parsed; raw tail: " + raw[-900:].replace("\n", "|")]

# GitHub keeps at most 10 annotations per check run, so one line per error gets
# truncated silently. Pack them into a few long chunks instead.
joined = [t.replace("\r", " ").replace("%", "25").replace("\n", " ") for t in lines]
chunks, cur = [], ""
for t in joined:
    if len(cur) + len(t) + 3 > 1300 and cur:
        chunks.append(cur); cur = t
    else:
        cur = t if not cur else cur + " ;; " + t
if cur:
    chunks.append(cur)

for i, c in enumerate(chunks[:6]):
    print(f"::error::RUSTC[{i + 1}/{len(chunks)}] {c}", flush=True)
    emitted += 1
print(f"::notice::RUSTC summary chunks={len(chunks)} total_errors={len(lines)}", flush=True)
PY
fi

exit "$code"
