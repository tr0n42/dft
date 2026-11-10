@echo off
rem Measures how much of every element each body holds (whole body and outer 500 m), writes data\reference\abundance.csv and prints the summary (log: abundance-log.txt).
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File tools\run-logged.ps1 abundance-log.txt "cargo run --release --bin abundance"
pause
