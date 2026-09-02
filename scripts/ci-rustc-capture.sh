#!/bin/sh
# TEMPORARY DIAGNOSTIC (rustc/clippy wrapper) — delete once CI is green.
#
# Why this exists: the Actions log hosts (results-receiver / objects.githubusercontent.com)
# are unreachable from the auditing environment, so `gh run view --log` cannot fetch the
# compiler output at all. This wrapper re-emits rustc/clippy diagnostics that point at this
# repository's own sources as `::error::` workflow commands; the runner records those as
# check-run ANNOTATIONS, which are readable through the checks API.
#
# Selection is by file path (`crates/…`, `apps/…`) rather than by --crate-name, because
# integration tests, examples and benches compile under their own names — filtering on the
# name silently hid a whole class of errors.
#
# Every invocation gets its OWN scratch directory. Using a fixed /tmp path made all the
# parallel rustc processes of a cold build write and re-read the same file, so cargo
# received interleaved JSON it could not parse: the build failed in ~15s with exit 101 and
# no diagnostics anywhere. If scratch allocation is impossible, the wrapper gets out of the
# way entirely (`exec "$@"`) rather than risking the real build.

scratch=$(mktemp -d 2>/dev/null) || exec "$@"
argsf="$scratch/args"
outf="$scratch/out"

printf '%s' "$*" > "$argsf"

case "$1" in
  *clippy-driver*) CI_CLIPPY_WARNINGS=1; export CI_CLIPPY_WARNINGS ;;
esac

"$@" > "$outf" 2>&1
code=$?
cat "$outf"

if [ "$code" -ne 0 ]; then
  python3 - "$outf" "$argsf" <<'PY'
import json, os, re, sys

out_path, args_path = sys.argv[1], sys.argv[2]
raw = open(out_path, encoding="utf-8", errors="replace").read()
args = open(args_path, encoding="utf-8", errors="replace").read()

OURS = ("crates/", "apps/", "voice-packs/", "scripts/")
OURS_EXTERN = tuple(f"--extern {n}=" for n in ("vocal_core", "voice_pack", "chiti_voice_cli"))
MATCH = bool(re.search(r"--crate-name[= ](vocal_core|voice_pack|chiti_voice)\b", args))
WANT_WARNINGS = os.environ.get("CI_CLIPPY_WARNINGS") == "1"


def is_ours(d):
    spans = d.get("spans") or []
    if spans:
        # A span is authoritative: a diagnostic inside a registry crate is never ours,
        # even while it is pulled in by one of our compilations.
        return any((s.get("file_name") or "").startswith(OURS) for s in spans)
    # No location at all (E0463 "can't find crate", E0601, link errors). A target that
    # receives one of our crates via --extern is one of ours, even when its --crate-name
    # is `offline_synthesis` / `simple_speak` / `pack_security`.
    return MATCH or any(e in args for e in OURS_EXTERN)


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
        # the wrapped program is clippy-driver, which is when they are enabled here.
        if lvl != "error" and not (lvl == "warning" and WANT_WARNINGS):
            continue
        if not is_ours(d):
            continue
        msg = (d.get("message") or "").replace("\n", " ").strip()
        if not msg or msg.startswith("aborting due to"):
            continue
        code = ((d.get("code") or {}).get("code") or "error")
        spans = [s for s in (d.get("spans") or []) if s.get("is_primary")] or (d.get("spans") or [])
        loc = ""
        if spans:
            s0 = spans[0]
            loc = f"{s0.get('file_name')}:{s0.get('line_start')}:{s0.get('column_start')}"
        key = (code, loc, msg[:150])
        if key in seen:
            continue
        seen.add(key)
        lines.append(f"{code} {loc}: {msg}" if loc else f"{code}: {msg}")
    else:
        # Plain-text mode (build scripts, `--print` probes, older cargo).
        m = re.match(r"^((?:error|warning)(?:\[[EW]\d+\])?!?:?.{0,300})", line)
        if m and (MATCH or WANT_WARNINGS) and m.group(1) not in seen:
            seen.add(m.group(1))
            lines.append(m.group(1))

if not lines:
    if MATCH or any(e in args for e in OURS_EXTERN):
        # One of our targets failed and nothing was parseable: surface the tail.
        lines = ["OUR-TARGET failure, unparsed; stderr tail: " + raw[-900:].replace("\n", "|")]
    else:
        # A dependency failed. Not our source's fault, but it reddens this repo's gate
        # (the live matrix builds rust=[stable,nightly] and has no cache key for the other
        # one), so show the first error line rather than staying silent.
        m = re.search(r"^error(?:\[[EW]\d+\])?!?:? ?(.{0,300})", raw, re.M)
        head = m.group(1).strip() if m else ""
        if not head and "error" not in raw.lower():
            raise SystemExit(0)
        crate = re.search(r"--crate-name[= ](\S+)", args)
        lines = [f"DEPENDENCY {crate.group(1) if crate else '?'}: {head or 'no error line'}; "
                 f"tail: {raw[-700:]}".replace("\n", "|")]

# GitHub keeps at most 10 annotations per check run, so pack the list into a few long
# chunks instead of emitting one annotation per diagnostic.
text = " ;; ".join(t.replace("\r", " ").replace("%", "25").replace("\n", " ") for t in lines)
chunks = [text[i:i + 1300] for i in range(0, len(text), 1300)] or ["none"]
for i, c in enumerate(chunks[:8]):
    print(f"::error::RUSTC[{i + 1}/{len(chunks)}] {c}", flush=True)
print(f"::notice::RUSTC count={len(lines)} chunks={len(chunks)} clippy={int(WANT_WARNINGS)}", flush=True)
PY
fi

rm -rf "$scratch"
exit "$code"
