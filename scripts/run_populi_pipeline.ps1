param(
    [string]$DataDir = "target/dogfood",
    [string]$OutputDir = "populi/runs/v1",
    [string]$Backend = "vulkan",
    [switch]$SkipTrain,
    [switch]$StrictGate
)

$ErrorActionPreference = "Stop"

function Invoke-Stage {
    param(
        [string]$Name,
        [scriptblock]$Action
    )
    Write-Host "==> $Name"
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    & $Action
    $sw.Stop()
    Write-Host ("<== {0} ({1:n1}s)" -f $Name, $sw.Elapsed.TotalSeconds)
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $repoRoot

$releaseVox = Join-Path $repoRoot "target/release/vox.exe"
$debugVox = Join-Path $repoRoot "target/debug/vox.exe"

if (Test-Path $releaseVox) {
    $vox = $releaseVox
} elseif (Test-Path $debugVox) {
    $vox = $debugVox
} else {
    throw "vox binary not found. Build first: cargo build -p vox-cli"
}

New-Item -ItemType Directory -Force -Path "populi/data", $DataDir, $OutputDir | Out-Null

Invoke-Stage "Corpus extract examples" { & $vox corpus extract examples/ -o populi/data/validated.jsonl }
Invoke-Stage "Corpus extract docs" { & $vox corpus extract docs/ -o populi/data/validated.jsonl }
Invoke-Stage "Corpus validate" { & $vox corpus validate populi/data/validated.jsonl --no-recheck -o populi/data/validated.jsonl }
Invoke-Stage "Corpus pairs" {
    & $vox corpus pairs populi/data/validated.jsonl -o "$DataDir/train.jsonl" --docs docs/src/ --docs docs/src/research/ --docs docs/src/adr/
}
# Skip rustdoc merge: rustdoc pairs have Rust prose as response, not Vox code; they pollute parse rate.

Invoke-Stage "Corpus eval" { & $vox corpus eval "$DataDir/train.jsonl" -o "$OutputDir/eval_results.json" }

if (-not $SkipTrain) {
    $env:VOX_BACKEND = $Backend
    $env:VOX_BENCHMARK = "1"
    if ($StrictGate) {
        $env:VOX_EVAL_STRICT = "1"
        $env:VOX_BENCHMARK_MIN_PASS_RATE = "0.80"
    } else {
        $env:VOX_EVAL_STRICT = "0"
        $env:VOX_BENCHMARK_MIN_PASS_RATE = "0.0"
    }
    Invoke-Stage "Native train" { & $vox train --native --data-dir $DataDir --output-dir $OutputDir }
}

Write-Host "Pipeline complete."
