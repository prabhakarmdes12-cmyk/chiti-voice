#!/bin/sh
# TEMPORARY DIAGNOSTIC (cargo test runner) — delete once CI is green.
#
# Cargo's log output is unreachable from the auditing environment, so a failing test
# is invisible. This wraps each test binary: it runs it, passes the output through
# untouched, and re-emits the failure summary as check-run annotations, which are
# readable through the checks API. Only configured for the Linux target, so macOS and
# Windows jobs are unaffected.
#
# usage: ci-test-capture.sh <binary> [args...]

bin="$1"
shift
"$bin" "$@" > /tmp/test-out.txt 2>&1
code=$?
cat /tmp/test-out.txt

if [ "$code" -ne 0 ]; then
  python3 - <<'PY'
import re
raw = open("/tmp/test-out.txt", encoding="utf-8", errors="replace").read()

failed = re.findall(r"^(?:test )?(\S+) \.\.\. (?:FAILED|failed)\s*$", raw, re.M)
panics = re.findall(r"^(?:\s*)thread '.*? panicked.*$", raw, re.M)
result = re.findall(r"^test result:.*$", raw, re.M)
compilation = re.findall(r"^(?:error|warning)\[?E?\w*\]?.*$", raw, re.M)

parts = []
if failed:
    parts.append("FAILED_TESTS=" + ", ".join(sorted(set(failed))[:12]))
if panics:
    parts.append("PANIC: " + " ;; ".join(p.strip()[:260] for p in panics[:4]))
if result:
    parts.append("RESULT: " + " ;; ".join(r.strip() for r in result[:4]))
if not failed and not result and compilation:
    parts.append("BUILD: " + " ;; ".join(c.strip()[:200] for c in compilation[:5]))
if not parts:
    parts.append("TAIL: " + raw[-900:].replace("\n", "|"))

text = " ;; ".join(parts).replace("\r", " ").replace("%", "25")
for i in range(0, len(text), 1300):
    print(f"::error::CITEST {text[i:i+1300]}", flush=True)
PY
fi

exit "$code"
