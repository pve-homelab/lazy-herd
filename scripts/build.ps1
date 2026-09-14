$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)
New-Item -ItemType Directory -Force -Path bin | Out-Null
cargo build --release
$targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path (Get-Location) "target" }
$src = Join-Path $targetRoot "release\lazy-herd.exe"
if (-not (Test-Path $src)) {
    throw "Built binary not found at $src"
}
try {
    Copy-Item -Force $src "bin\lazy-herd.exe"
    Write-Host "Installed bin\lazy-herd.exe"
} catch {
    Write-Warning "Could not overwrite bin\lazy-herd.exe (is the Lazy Herd pane still open?). Built binary is at: $src"
}
