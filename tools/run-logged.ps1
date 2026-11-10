# Runs a command, showing its output live in the window and saving it to a log.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File tools\run-logged.ps1 <log file> "<command>"
param([string]$Log, [string]$Command)
Set-Location (Split-Path $PSScriptRoot -Parent)
"Started $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss'): $Command" | Set-Content -Path $Log -Encoding UTF8
Write-Host "Running: $Command" -ForegroundColor Cyan
Write-Host "(output is also saved to $Log)" -ForegroundColor DarkGray
$sw = [Diagnostics.Stopwatch]::StartNew()
cmd /c "$Command 2>&1" | ForEach-Object { Write-Host $_; Add-Content -Path $Log -Value $_ -Encoding UTF8 }
$code = $LASTEXITCODE
$done = "Finished $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') after {0:N0} s, exit code $code" -f $sw.Elapsed.TotalSeconds
Write-Host $done -ForegroundColor Cyan
Add-Content -Path $Log -Value $done -Encoding UTF8
exit $code
