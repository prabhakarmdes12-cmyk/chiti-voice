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
