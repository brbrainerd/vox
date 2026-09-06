#Requires -Version 5.1
# NOTE: This file is kept in sync with docs-astro/public/voxup.ps1 by the
# documented_install_urls_are_served test. Both must be identical.
# Supported release targets (kept in sync with SUPPORTED_RELEASE_TARGETS in vox-cli):
#   x86_64-pc-windows-msvc
# voxup installer — Windows
# Usage (production):
#   Invoke-WebRequest -Uri https://voxlang.org/voxup.ps1 -OutFile voxup.ps1; .\voxup.ps1
# Usage (local dev):
#   .\scripts\install.ps1
[CmdletBinding()]
param(
    [switch]$NoModifyPath
)

$ErrorActionPreference = 'Stop'

# NOTE: `/releases/latest` excludes pre-releases and 404s while every published
# release is a pre-release. List releases instead and take the newest entry.
$GithubApi = 'https://api.github.com/repos/vox-foundation/vox/releases?per_page=1'
$GithubDl  = 'https://github.com/vox-foundation/vox/releases/download'

function Write-Step([string]$Msg) { Write-Host "voxup: $Msg" -ForegroundColor Cyan }
function Write-Fail([string]$Msg) {
    Write-Host "voxup: error: $Msg" -ForegroundColor Red
    exit 1
}

# ── Platform detection ────────────────────────────────────────────────────────

function Get-VoxupTarget {
    # Only x86_64-pc-windows-msvc is published. ARM64 Windows runs x64 binaries
    # under emulation, so resolve to the x64 target rather than requesting an
    # aarch64-pc-windows-msvc asset that is never built (download would 404).
    if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
        Write-Step "ARM64 Windows detected — using the x86_64 build (runs under emulation)"
    }
    return 'x86_64-pc-windows-msvc'
}

# ── SHA-256 verification ──────────────────────────────────────────────────────

function Assert-Sha256([string]$FilePath, [string]$Expected) {
    $actual   = (Get-FileHash -Path $FilePath -Algorithm SHA256).Hash.ToLower()
    $expected = $Expected.ToLower().Trim()
    if ($actual -ne $expected) {
        Write-Fail "SHA-256 mismatch for $FilePath`n  expected: $expected`n  actual:   $actual"
    }
    Write-Step "Checksum OK"
}

# ── Main ─────────────────────────────────────────────────────────────────────

Write-Step "Detecting platform..."
$Target = Get-VoxupTarget
Write-Step "Target: $Target"

Write-Step "Fetching latest release info..."
try {
    $release = Invoke-RestMethod `
        -Uri $GithubApi `
        -Headers @{ Accept = 'application/vnd.github+json'; 'User-Agent' = 'voxup-install.ps1' } `
        -UseBasicParsing
} catch {
    Write-Fail "Failed to fetch release info from GitHub: $_"
}
# The list endpoint returns an array; take the newest entry.
$Tag = @($release)[0].tag_name
if (-not $Tag) { Write-Fail "Could not determine latest release tag from GitHub API" }
Write-Step "Latest release: $Tag"

$Archive      = "voxup-${Tag}-${Target}.zip"
$ArchiveUrl   = "${GithubDl}/${Tag}/${Archive}"
$ChecksumsUrl = "${GithubDl}/${Tag}/checksums.txt"

$TmpDir = Join-Path $env:TEMP "voxup-install-$(New-Guid)"
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

try {
    Write-Step "Downloading $Archive..."
    Invoke-WebRequest -Uri $ArchiveUrl -OutFile "$TmpDir\$Archive" -UseBasicParsing

    Write-Step "Downloading checksums.txt..."
    Invoke-WebRequest -Uri $ChecksumsUrl -OutFile "$TmpDir\checksums.txt" -UseBasicParsing

    $ChecksumLine = Get-Content "$TmpDir\checksums.txt" |
        Where-Object { $_ -match "  $([regex]::Escape($Archive))$" } |
        Select-Object -First 1
    if (-not $ChecksumLine) {
        Write-Fail "No checksum entry found for '$Archive' in checksums.txt"
    }
    $ExpectedHash = ($ChecksumLine -split '\s+')[0]

    Assert-Sha256 -FilePath "$TmpDir\$Archive" -Expected $ExpectedHash

    Write-Step "Extracting..."
    Expand-Archive -Path "$TmpDir\$Archive" -DestinationPath $TmpDir -Force

    $VoxupExe = "$TmpDir\voxup.exe"
    if (-not (Test-Path $VoxupExe)) {
        Write-Fail "voxup.exe not found after extraction in $TmpDir"
    }

    $installArgs = @('install', 'default')
    if ($NoModifyPath) { $installArgs += '--no-modify-path' }
    Write-Step "Running: voxup $($installArgs -join ' ')"
    & $VoxupExe @installArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Fail "voxup $($installArgs -join ' ') exited with code $LASTEXITCODE"
    }
} finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}
