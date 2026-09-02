#!/bin/sh
# TEMPORARY DIAGNOSTIC (rustc wrapper) — delete once CI is green.
#
# Why: this environment can reach api.github.com but not the Actions log host
# (objects/results-receiver.actions.githubusercontent.com are blocked), so `gh run
# view --log` cannot fetch compiler output. A RUSTC_WRAPPER can re-emit rustc's stderr
# as a `::error::` workflow command, which the runner records as a check-run
# ANNOTATION — and annotations are readable through the checks API.
#
# Configured by .cargo/config.toml. Only annotates for this workspace's own crates, so
# third-party build noise stays quiet.

printf '%s' "$*" | grep -Eq -- '--crate-name[= ](vocal_core|voice_pack|chiti_voice)' && MATCH=1 || MATCH=0

out=$("$@" 2>&1)
code=$?

if [ "$code" -ne 0 ] && [ "$MATCH" -eq 1 ]; then
  snippet=$(printf '%s\n' "$out" | awk '
    /^error(\[E|:)/ { shown = 1 }
    shown {
      printf "%s | ", $0
      n++
      if (n > 26) { exit }
    }' | cut -c1-1700)
  if [ -n "$snippet" ]; then
    printf '::error::RUSTCERR %s\n' "$snippet"
  else
    fallback=$(printf '%s' "$out" | tr '\n' '|' | cut -c1-1200)
    printf '::error::RUSTCERR exit=%s no-error-line: %s\n' "$code" "$fallback"
  fi
fi

printf '%s\n' "$out"
exit "$code"
