# Thin wrapper: build/verify .cvpack voice packs on Windows.
#
# This used to be a second, independent implementation of the pack builder. It copied
# manifest.json verbatim (so every pack shipped `"size_bytes": 0` with a zero checksum),
# wrote placeholder model bytes with no verification, and hard-coded the voice list — i.e.
# it could only produce archives the Rust loader would reject.
#
# There is now exactly one implementation: scripts/build-voice-packs.py. Keep it that way;
# do not add logic back here.

$ErrorActionPreference = "Stop"

$script:py = Get-Command python -ErrorAction SilentlyContinue
if (-not $script:py) { $script:py = Get-Command py -ErrorAction SilentlyContinue }
if (-not $script:py) { Write-Error "python was not found on PATH. Install Python 3.11+."; exit 1 }

$builder = Join-Path $PSScriptRoot "build-voice-packs.py"
if (-not (Test-Path $builder)) { Write-Error "missing $builder"; exit 1 }

if ($args.Count -eq 0) {
    & $script:py.Source $builder build
} else {
    & $script:py.Source $builder @args
}

exit $LASTEXITCODE
