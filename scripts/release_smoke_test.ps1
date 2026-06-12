param(
  [string]$ExePath = "",
  [string]$TxtPath = "",
  [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$DefaultExeCandidates = @(
  (Join-Path $RepoRoot "jisig-bpoint-converter.exe"),
  (Join-Path $RepoRoot "src-tauri\target\release\jisig-bpoint-converter.exe")
)

if (-not $ExePath) {
  $ExePath = $DefaultExeCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}

if (-not $ExePath -or -not (Test-Path $ExePath)) {
  throw "找不到 release exe，请先构建。候选路径: $($DefaultExeCandidates -join ', ')"
}

if (-not $TxtPath) {
  $TxtPath = Join-Path $RepoRoot "test_arcpy\txt_output\plot_000.txt"
}

if (-not (Test-Path $TxtPath)) {
  throw "找不到 smoke 输入 TXT: $TxtPath"
}

if (-not $OutDir) {
  $OutDir = Join-Path $env:TEMP ("jisig-smoke-" + [guid]::NewGuid().ToString("N"))
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$Stem = [System.IO.Path]::GetFileNameWithoutExtension($TxtPath)

$LogPath = Join-Path $OutDir "release_smoke.log"
$Args = @(
  "--smoke-test"
  "--smoke-txt", $TxtPath
  "--smoke-output", $OutDir
)

Write-Host "Exe: $ExePath"
Write-Host "TXT: $TxtPath"
Write-Host "Out: $OutDir"
Write-Host "Log: $LogPath"

$ExpectedGpkg = Join-Path $OutDir ($Stem + ".gpkg")
$ExpectedPreview = Join-Path $OutDir ($Stem + "_preview.txt")
$ExpectedReport = Join-Path $OutDir "release_smoke_report.txt"

$stdout = Join-Path $OutDir "release_smoke.stdout.log"
$stderr = Join-Path $OutDir "release_smoke.stderr.log"

$proc = Start-Process -FilePath $ExePath `
  -ArgumentList $Args `
  -PassThru `
  -Wait `
  -WindowStyle Hidden `
  -RedirectStandardOutput $stdout `
  -RedirectStandardError $stderr

Get-Content $stdout, $stderr -ErrorAction SilentlyContinue | Tee-Object -FilePath $LogPath | Out-Null

if ($proc.ExitCode -ne 0) {
  throw "smoke 模式返回失败，退出码: $($proc.ExitCode)"
}

if (-not (Test-Path $ExpectedGpkg)) {
  throw "未生成 GPKG: $ExpectedGpkg"
}

if (-not (Test-Path $ExpectedPreview)) {
  throw "未生成预览 TXT: $ExpectedPreview"
}

if (-not (Test-Path $ExpectedReport)) {
  throw "未生成 smoke 报告: $ExpectedReport"
}

$ReportText = Get-Content $ExpectedReport -Raw
if ($ReportText -notmatch "SMOKE_OK") {
  throw "smoke 报告不完整: $ReportText"
}

Write-Host "Smoke OK"
Write-Host "GPKG: $ExpectedGpkg"
Write-Host "Preview: $ExpectedPreview"
Write-Host "Report: $ExpectedReport"
