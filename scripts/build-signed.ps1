# Signed build: read private key + prompt password (masked) + tauri build
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

$keyPath = Join-Path $env:USERPROFILE '.tauri\bpoint-converter.key'
if (-not (Test-Path $keyPath)) {
    Write-Host "ERROR: private key not found: $keyPath" -ForegroundColor Red
    exit 1
}

$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyPath -Raw
$sec = Read-Host 'Enter signing password' -AsSecureString
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [System.Net.NetworkCredential]::new('', $sec).Password

Write-Host ""
Write-Host "Starting signed release build (5-10 min)..." -ForegroundColor Cyan
Write-Host ""
npm run tauri build
if ($LASTEXITCODE -eq 0) {
    $sig = Get-ChildItem 'src-tauri\target\release\bundle\nsis\*.sig' -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($sig) {
        Write-Host ""
        Write-Host "BUILD OK - signature generated:" $sig.Name -ForegroundColor Green
    } else {
        Write-Host ""
        Write-Host "WARNING: build OK but no .sig found!" -ForegroundColor Yellow
    }
} else {
    Write-Host ""
    Write-Host "BUILD FAILED (exit $LASTEXITCODE)" -ForegroundColor Red
}
