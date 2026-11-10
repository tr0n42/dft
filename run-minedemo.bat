@echo off
rem Runs the M2 mining demo; output shows live and is saved to minedemo-log.txt.
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File tools\run-logged.ps1 minedemo-log.txt "cargo run --release --bin minedemo"
pause
