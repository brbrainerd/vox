# vox-dev.ps1 (Thin Launcher)
# Forward all arguments to vox-cli via cargo run.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$VoxArgs)
$ErrorActionPreference = 'Stop'
if (Get-Command sccache -ErrorAction SilentlyContinue) {
    $env:RUSTC_WRAPPER = 'sccache'
}
$R = Resolve-Path (Join-Path $PSScriptRoot '..\..')
Push-Location $R.Path
try { & cargo run -q -p vox-cli -- $VoxArgs } finally { Pop-Location }
