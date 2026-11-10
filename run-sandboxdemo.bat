@echo off
rem Runs the sandbox demo and writes frames to out\sandbox (log: sandboxdemo-log.txt).
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File tools\run-logged.ps1 sandboxdemo-log.txt "cargo run --release --bin sandboxdemo -- --seconds 120 --every 10"
pause
