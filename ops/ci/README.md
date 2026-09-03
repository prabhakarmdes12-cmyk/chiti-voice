# ops/ci — staged CI definitions

CI files that could not be written directly into `.github/workflows/`: creating or
updating a workflow requires the `workflows` permission, which the automation that
prepared these changes does not have. The push of `.github/workflows/*` is rejected
outright, so the corrected definition lives here for a human to adopt.

| File | Status | Adopt with |
|---|---|---|
| `ci-phase1.yml` | ready, unapplied | `scripts/install-ci.sh` |

## Why a replacement is needed

`.github/workflows/ci-phase1.yml` as committed presents itself as seven quality gates
("Phase 1 Quality Gates: PASSED"), and the repo's docs cite those gates as proof that
Phase 1 completed. In practice its checks could not fail:

- `Build`/`Test` — have been **red on `main` since this workflow landed** (missing
  `crates/vocal-core/examples/simple_speak.rs` broke manifest parsing for the whole
  workspace), yet the docs reported "compiles cleanly ✅".
- `offline-synthesis` — runs a test suite that contains no network code, then
  `echo "VOICE_INV_001 validated"`. No network is actually removed, so the project's
  headline invariant is unenforced while claimed as a gate.
- `dependency-audit` — greps `Cargo.toml` and a `Cargo.lock` that did not exist, and ends
  with `cargo audit ... || echo "Audit check (info only)"`, so it can never fail.
- `phase1-quality-gates` — asserts voices exist via `test -f voice-packs/dist/*.cvpack`.
  All three failed their own checksums, so "exists" ≠ "loads", and the final report was a
  sequence of `echo` lines printing "PASSED" unconditionally.
- `actions/cache@v3` (deprecated) keyed on `hashFiles('**/Cargo.lock')` — empty, since no
  lockfile was committed.

The replacement adds: `--all-targets` builds, cross-platform tests, real isolation
(`sudo unshare -rn` plus a self-check that fails if isolation did not apply), pack
verification and drift detection, resolved-graph dependency audit, a provenance-fabrication
check, and a `docs-truth` job that keeps README claims tied to
`vocal_core::REAL_SYNTHESIS_AVAILABLE` (and checks documented directories exist, links
resolve, and text files are clean UTF-8).

One job is deliberately `continue-on-error`: "real model readiness", because there is no
model in this repo yet and a hard gate would keep CI permanently red. It surfaces as a
visible warning with the reason. Make it blocking in the PR that lands the first real model.


## Four more defects — each found by *executing* a gate, never by reading it (2026-09-03)

The first three were found by watching real runs; the fourth by running the step locally.
Both beats reading the YAML, which is how each of these looked correct.

1. **`--all-features` in a lint job is not "more coverage", it is a different build.**
   Because `ort` was declared `optional = true`, cargo created an implicit `ort` feature,
   so `cargo clippy --all-targets --all-features` enabled it and ran its `download-binaries`
   build script. That job then failed *inside a dependency*, in ~15 s, having compiled none
   of this workspace — meaning it could neither lint our code nor explain itself. The fix is
   not to pass `--all-features`; name the features you mean (see the `clippy (piper feature)`
   step). Long term: when inference lands, either vendor ONNX Runtime or declare
   `ort = { default-features = false, features = ["load-dynamic"] }` so no build step needs
   a network, and only then is `--all-features` safe to use.

2. **A `rust: [stable, nightly]` matrix as a *build gate* makes other people's regressions
   yours.** Six build jobs, two per OS; a nightly-only break in a transitive dependency turns
   the repo's own gate red with nothing in this workspace to change. Nightly is a useful
   heads-up and a terrible blocker: this workflow builds/tests on stable, and if nightly
   coverage is wanted it belongs in a `continue-on-error` advisory job, like the "real model
   readiness" one already here.

3. **`actions/cache@v3` keyed on `hashFiles('**/Cargo.lock')` with no committed lockfile
   produces a constant key**, so the cache is a stale grab-bag shared by every branch and
   every toolchain — which is also how a "green" build can hide a cold-cache failure. Keep
   the key on the lockfile *and* commit the lockfile (`cargo generate-lockfile` on a
   networked machine), and include `matrix.rust` in the key if nightly ever comes back.

4. **A gate that cannot parse its own input skips silently and reports success.** The
   `docs-truth` job extracts `REAL_SYNTHESIS_AVAILABLE` to decide whether the stale-claim scan
   applies, and extracted it with `grep 'NAME: (true|false)'` — while the source declares
   `NAME: bool = false`. Empty variable, `[ "$AVAIL" = "false" ]` false, every check below
   skipped, on every branch, forever, green the whole time. Fixed by matching the declaration as
   written *and* by making an unreadable flag a hard error: a check that cannot determine
   whether it should run must fail, not proceed. Verified in both directions — the scan passes
   on this tree, and a planted affirmative claim in `AGENTS.md` makes it fire.

## Why some debugging commits in this branch look strange

This repository's CI logs were unreadable from the auditing environment: the log endpoints
(`results-receiver.actions.githubusercontent.com`, `objects.githubusercontent.com`) are
network-blocked there, so `gh run view --log` cannot fetch output, and check-run
**annotations** were the only channel back. `scripts/ci-rustc-capture.sh`,
`scripts/ci-test-capture.sh` and `scripts/ci-rustdoc-capture.sh` are wrappers that turn
rustc/clippy/test-binary failures into `::error::` annotations for exactly that purpose; the
first two were wired up through `.cargo/config.toml`.

None of those three scripts is in the tree any more, and neither is the `.cargo/config.toml` that
wired them: they have been removed in the same slice as each fix they served, twice now. Read the
following as instructions for resurrecting them, not as a description of this checkout.

They are **scaffolding, not product**: `build.rustc-wrapper` makes non-POSIX platforms fail to
build at all, and a wrapper that mishandles concurrent invocations will corrupt cargo's JSON
diagnostics (it did: a shared /tmp file turned a healthy workspace into eight silent
101-exit jobs). They must not survive — see the "remove the CI diagnostics" commit. If you
need them again, copy them back from history, use them for one push, and delete them.
