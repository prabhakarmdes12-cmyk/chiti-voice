#!/bin/sh
# TEMPORARY DIAGNOSTIC (cargo test runner) — delete once CI is green.
#
# Cargo's log is unreachable from the auditing environment, so a failing test is
# otherwise invisible. This wraps every test binary: the binary's output is passed
# through untouched, and a summary (failed test names, panic lines, the `test result:`
# line) comes back as check-run annotations, readable via the checks API.
#
# Configured for the Linux target only, so macOS/Windows jobs keep normal behaviour.
# The scratch file is per-invocation because cargo runs the workspace's test binaries
# concurrently; a shared /tmp path made them truncate each other's output.
#
# usage: ci-test-capture.sh <binary> [args...]

scratch=$(mktemp)
bin="$1"
shift
"$bin" "$@" > "$scratch" 2>&1
code=$?
cat "$scratch"

python3 - "$scratch" "$code" <<'PY'
import re, sys

path, code = sys.argv[1], int(sys.argv[2])
raw = open(path, encoding="utf-8", errors="replace").read()

# --nocapture splits "test name ... " from "FAILED" across the panic output, so match
# both the tidy form and any line that merely mentions a failing test.
failed = set(re.findall(r"test (\S+) \.\.\. FAILED", raw))
for block in re.findall(r"---- (\S+) stdout ----", raw):
    failed.add(block)
for line in raw.splitlines():
    m = re.match(r"^test (\S+) \.\.\. *$", line)
    if m and "FAILED" in raw[raw.index(line) : raw.index(line) + 3000]:
        failed.add(m.group(1))

panics = [
    l.strip()[:240]
    for l in raw.splitlines()
    if "panicked at" in l or l.strip().startswith("assertion")
][:5]
result = re.findall(r"test result:.*", raw)
compile_err = [l.strip()[:200] for l in raw.splitlines() if re.match(r"^error(\[E\d+\])?", l.strip())][:5]

if code == 0 and not failed:
    sys.exit(0)

parts = []
if failed:
    parts.append("FAILED_TESTS=" + ", ".join(sorted(failed)[:12]))
if panics:
    parts.append("PANIC: " + " ;; ".join(panics))
if result:
    parts.append("RESULT: " + " ;; ".join(r.strip() for r in result[:5]))
if compile_err:
    parts.append("BUILD: " + " ;; ".join(compile_err))
if not parts:
    parts.append("exit=%d; TAIL: %s" % (code, raw[-800:].replace("\n", "|")))

text = " ;; ".join(parts).replace("\r", " ").replace("%", "25").replace("\n", " ")
for i in range(0, len(text), 1300):
    print("::error::CITEST %s" % text[i : i + 1300], flush=True)
PY

rm -f "$scratch"
exit "$code"
