$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222 --remote-allow-origins=*"
Start-Process -FilePath "app_test.exe" -WorkingDirectory $PSScriptRoot
Start-Sleep -Seconds 6
$r = Get-NetTCPConnection -LocalPort 9222 -State Listen -ErrorAction SilentlyContinue
if ($r) { Write-Host "9222 LISTEN OK" } else { Write-Host "9222 NOT LISTENING" }
