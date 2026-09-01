# Build voice packs using PowerShell
# Creates .cvpack files (ZIP archives) for each voice

$ErrorActionPreference = "Stop"

# Load required assemblies
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$projectRoot = Split-Path -Parent $PSScriptRoot
$voicePacksDir = Join-Path $projectRoot "voice-packs"
$outputDir = Join-Path $voicePacksDir "dist"

if (-not (Test-Path $voicePacksDir)) {
    Write-Error "Voice packs directory not found: $voicePacksDir"
    exit 1
}

# Create output directory
if (-not (Test-Path $outputDir)) {
    New-Item -ItemType Directory -Path $outputDir | Out-Null
}

# Build each voice pack
foreach ($packName in @("tara", "kashi", "bobo")) {
    $packDir = Join-Path $voicePacksDir $packName
    $manifestPath = Join-Path $packDir "manifest.json"
    $outputPath = Join-Path $outputDir "$packName.cvpack"
    
    if (-not (Test-Path $manifestPath)) {
        Write-Warning "Manifest not found: $manifestPath"
        continue
    }
    
    Write-Host "Building $packName..."
    
    # Remove existing pack if present
    if (Test-Path $outputPath) {
        Remove-Item $outputPath -Force
    }
    
    # Create ZIP archive
    # Read manifest and add it
    $zip = [System.IO.Compression.ZipFile]::Open($outputPath, 1)  # 1 = Create mode
    
    # Add manifest.json
    $manifestContent = Get-Content -Path $manifestPath -Raw
    $entry = $zip.CreateEntry("manifest.json")
    $stream = $entry.Open()
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($manifestContent)
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Dispose()
    
    # Add placeholder model files
    foreach ($fileName in @("model.onnx", "model_config.json")) {
        $entry = $zip.CreateEntry($fileName)
        $stream = $entry.Open()
        $bytes = [System.Text.Encoding]::UTF8.GetBytes("PLACEHOLDER_MODEL_DATA_PHASE_1")
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Dispose()
    }
    
    $zip.Dispose()
    
    $fileSize = (Get-Item $outputPath).Length
    Write-Host "  Created $outputPath ($fileSize bytes)"
}

Write-Host "`nVoice pack building complete"
