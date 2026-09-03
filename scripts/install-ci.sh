#!/usr/bin/env bash
# Adopt the corrected CI definition into .github/workflows/.
#
# Why this script exists: pushing changes under .github/workflows/ requires the
# `workflows` permission. Tooling that lacks it can prepare the corrected workflow
# (it lives in ops/ci/ci-phase1.yml) but cannot install it. Run this from a checkout
# with your own push rights.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
src="$root/ops/ci/ci-phase1.yml"
dst_rel=".github/workflows/ci-phase1.yml"
dst="$root/$dst_rel"

[ -f "$src" ] || { echo "missing $src" >&2; exit 1; }

if cmp -s "$src" "$dst" 2>/dev/null; then
  echo "already installed (identical): $dst_rel"
  exit 0
fi

cp "$src" "$dst"
echo "installed $dst_rel"

python3 - "$dst" <<'PYEOF'
import sys
try:
    import yaml
except ImportError:
    print("note: pyyaml not installed — skipped YAML validation")
    raise SystemExit(0)
d = yaml.safe_load(open(sys.argv[1]))
print("valid YAML; jobs:", ", ".join(d["jobs"]))
PYEOF

echo
echo "next steps:"
echo "  git add $dst_rel ops/ci"
echo "  git commit -m 'ci: adopt corrected quality gates'"
echo "  git push"
echo
echo "then remove ops/ci/ci-phase1.yml once it is live, or keep it as the reviewable copy."
