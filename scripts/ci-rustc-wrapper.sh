#!/usr/bin/env bash
# Temporary CI helper: make `cargo` failures readable from this sandbox.
#
# Why: the dev sandbox has no Rust toolchain and cannot reach any rustup/crates.io mirror, while
# GitHub's job-log endpoints redirect to a blob store that is unreachable from here too. So every
# compile error came back as the single annotation "Process completed with exit code 101" — enough to
# know a build broke, not enough to know why. Cargo invokes this wrapper *instead of* rustc (as
# `wrapper rustc <args…>`), so it can re-emit rustc's own diagnostics as `::error::` workflow commands,
# which DO become check-run annotations readable via `gh api .../check-runs/<id>/annotations`.
#
# Outside CI it is a transparent pass-through, so nobody's local build changes. Delete this file and
# the `[build] rustc-wrapper` line in `.cargo/config.toml` once they are no longer needed — the point is
# a readable oracle for commits written without a compiler, not a permanent gate.

set -u

# Cargo's wrapper protocol: the first argument is the compiler, the rest are its arguments.
if [ "$#" -lt 1 ]; then
  echo "ci-rustc-wrapper: expected 'rustc <args…>', got nothing" >&2
  exit 2
fi

if [ "${CI:-}" != "true" ]; then
  exec "$@"
fi

tmp="$(mktemp)"
"$@" >"$tmp" 2>&1
rc=$?

cat "$tmp"

# Annotate rustc's error headers plus the `--> file:line:col` line under each, which is where the
# path actually lives. Filter to this repo's sources so dependency noise cannot drown the signal.
grep -E -A1 '^error' "$tmp" \
  | grep -E 'crates/|apps/' \
  | head -n 12 \
  | while IFS= read -r line; do
      printf '::error::%s\n' "${line//'%'/'%25'}"
    done

# Fallback for the case where rustc wrote the message and the location on lines that did not both match.
if ! grep -qE '^error.*crates/|^error.*apps/|-->.+(crates|apps)/' "$tmp"; then
  grep -E '^(error|warning: unused|  -->)' "$tmp" | head -n 10 | while IFS= read -r line; do
    printf '::error::%s\n' "${line//'%'/'%25'}"
  done
fi

rm -f "$tmp"
exit "$rc"
