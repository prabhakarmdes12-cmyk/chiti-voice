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
# Outside CI this is a transparent pass-through. It is NOT wired into `.cargo/config.toml` any more,
# on purpose: that config applies to every OS in the matrix and the Windows runners cannot exec a bash
# script, so wiring it in turned three green Windows legs red. Use it by hand instead:
#
#   RUSTC_WRAPPER=./scripts/ci-rustc-wrapper.sh cargo check --workspace --all-targets
#
# Limit worth recording: this channel carries rustc diagnostics only. A *test assertion* failure emits
# no compiler message, so CI says nothing about which test failed -- which is how a set of fixtures that
# never reached the rule they claimed to test stayed red for a while. Mirror the rule locally instead.
