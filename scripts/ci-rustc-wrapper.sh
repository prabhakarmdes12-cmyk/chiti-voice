#!/usr/bin/env bash
# Temporary CI helper: make `cargo` failures readable from this sandbox.
#
# Why: no Rust toolchain here, no reachable rustup/crates.io mirror, and GitHub's job-log endpoints
# redirect to a blob store this sandbox cannot read — so a compile error arrives as nothing but
# "Process completed with exit code 101". Cargo's rustc-wrapper is a hook that lives in ordinary repo
# files (this repo's GitHub App may not touch .github/workflows), and it lets rustc's diagnostics be
# re-emitted as `::error::` workflow commands, which DO show up as check-run annotations.
#
# Two details that cost a CI cycle each to learn:
#   * cargo captures the wrapper's stdout to parse rustc output, so the annotations must go to the
#     *inherited* log fd (`/proc/$PPID/fd/1`), not to this process's stdout;
#   * GitHub only promotes `::error::` when it starts a line, so each block is flattened onto one line.
#
# Outside CI this is a transparent pass-through, and it is *not* wired into `.cargo/config.toml`
# permanently: that config applies to every OS in the matrix, the Windows runners cannot exec a bash
# script, and wiring it in once turned three green Windows legs red. Use it by hand, or add
# `.cargo/config.toml` for a single cycle when a red job needs reading:
#
#   RUSTC_WRAPPER=./scripts/ci-rustc-wrapper.sh cargo check --workspace --all-targets
#
# Limit worth recording: this channel carries rustc diagnostics only. A *test assertion* failure emits
# no compiler message, so CI says nothing about which test failed -- which is how a set of fixtures that
# never reached the rule they claimed to test stayed red for a while. Mirror the rule locally instead.
#
# One more, learned the hard way: an earlier edit of this file left only the comment header and deleted
# `set -u` onward, which nobody noticed because nothing was executing it. A file whose only consumer is
# a failing build is unverified until it runs; `bash -n` is the cheapest check that catches it.

set -u

if [ "$#" -lt 1 ]; then
  echo "ci-rustc-wrapper: expected 'rustc <args…>'" >&2
  exit 2
fi

if [ "${CI:-}" != "true" ]; then
  exec "$@"
fi

tmp="$(mktemp)"
"$@" >"$tmp" 2>&1
rc=$?
cat "$tmp"

log="/proc/$PPID/fd/1"
if [ -w "$log" ]; then
  # One liveness marker per runner, so a green-looking empty annotation list can be told apart from a
  # wrapper that never ran.
  if [ ! -f /tmp/ci-rustc-wrapper-alive ]; then
    touch /tmp/ci-rustc-wrapper-alive
    printf '::error::rustc-wrapper channel is alive (annotations from this job are CI log excerpts, not test failures)\n' >> "$log"
  fi
  if [ "$rc" -eq 0 ]; then exit 0; fi
  crate="$(printf '%s\n' "$@" | sed -nE 's/^--crate-name$//p;T;N;s/.*\n//p' | head -1)"
  [ -n "$crate" ] || crate="$(printf '%s' "$*" | grep -oE '\-\-crate-name [a-z_0-9]+' | head -1 | awk '{print $2}')"
  block="$(jq -Rr 'fromjson? | select(."$message_type"=="diagnostic") | select(.level=="error")
                  | "\(.spans[0].file_name // "?"):\(.spans[0].line_start // 0):\(.spans[0].line_end // 0) [\(.code.code // "error")] \(.message) :: " +
                    ([(.children[]? | select(.level=="note") | .message)][:2] | join(" ; "))' \
             "$tmp" 2>/dev/null | head -n 12 | tr '\n' '|')"
  if [ -z "$block" ]; then
    # No compiler message at all: then whatever the child DID say is the finding (a cargo-level error,
    # a linker failure, or a rustc that died before emitting JSON). Annotate the tail of it.
    block="crate=${crate:-?} rc=$rc no-compiler-message :: $(tail -c 1200 "$tmp" | tr '\n' '|' | sed -e 's/\x1b\[[0-9;]*m//g')"
  fi
  printf '::error::%s\n' "$(printf '%s' "[$crate] ${block:0:5800}" | sed 's/%/%25/g')" >> "$log"
  fi

rm -f "$tmp"
exit "$rc"
