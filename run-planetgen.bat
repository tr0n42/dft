@echo off
rem Builds every body in data\bodies and writes previews and reports to out (log: planetgen-log.txt).
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File tools\run-logged.ps1 planetgen-log.txt "cargo run --release --bin planetgen -- --exact"
pause
