# TOESTUB Self-Apply Automation for Vox
# Powershell implementation

$ErrorActionPreference = "Stop"

Write-Host "🚀 Running TOESTUB Architectural Scan..." -ForegroundColor Cyan

# 1. Build the engine
Write-Host "🛠️  Building TOESTUB engine..."
cargo build -p vox-toestub --release

# 2. Run scan
Write-Host "🔍 Scanning codebase for anti-patterns..."
$process = Start-Process -FilePath "cargo" -ArgumentList "run", "-q", "-p", "vox-toestub", "--bin", "toestub" -Wait -NoNewWindow -PassThru

if ($process.ExitCode -eq 0) {
    Write-Host "✅ TOESTUB: No major architectural violations found." -ForegroundColor Green
} else {
    Write-Host "⚠️ TOESTUB: Anti-patterns detected!" -ForegroundColor Yellow
    Write-Host "Please review the output above and refactor accordingly."
    exit $process.ExitCode
}
