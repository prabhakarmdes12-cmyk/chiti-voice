#!/bin/sh
# TEMPORARY DIAGNOSTIC (rustdoc wrapper) — delete once CI is green.
#
# Doctests are compiled AND executed by rustdoc, not by rustc, and they are not run
# through cargo's `runner`, so `cargo test --workspace` can fail with a doctest while
# every other diagnostic channel stays silent. This wraps rustdoc via [env] RUSTDOC,
# passes everything through, and turns a doctest failure into an annotation.

case "$*" in
  *--test*) DOCTEST=1 ;;
  *) DOCTEST=0 ;;
esac

scratch=$(mktemp)
rustdoc "$@" > "$scratch" 2>&1
code=$?
cat "$scratch"

if [ "$code" -ne 0 ] && [ "$DOCTEST" -eq 1 ]; then
  python3 - "$scratch" <<'PY'
import re, sys
raw = open(sys.argv[1], encoding="utf-8", errors="replace").read()
tests = re.findall(r"^test (src/[^ ]+) - .* \.\.\. FAILED", raw, re.M)
errs = [l.strip()[:220] for l in raw.splitlines() if re.match(r"^(error|warning)\[?[EW]?\d*\]?!?:", l.strip())]
keep = []
for l in raw.splitlines():
    s = l.strip()
    if re.match(r"^(error|warning)", s) or "FAILED" in s or re.match(r"^\s+--&gt;\s*src/", s) or s.startswith("--> src/") or s.startswith("--> examples/"):
        keep.append(s[:220])
    if len(keep) >= 8:
        break
parts = []
if tests:
    parts.append("FAILED_DOCTESTS=" + ", ".join(tests[:6]))
if keep:
    parts.append(" ;; ".join(keep))
elif errs:
    parts.append(" ;; ".join(errs[:6]))
if not parts:
    parts.append("TAIL: " + raw[-800:].replace("\n", "|"))
text = " ;; ".join(parts).replace("%", "25")[:1300]
print(f"::error::RUSTDOC {text}", flush=True)
PY
fi

rm -f "$scratch"
exit "$code"
