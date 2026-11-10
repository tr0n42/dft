@echo off
rem Plays five asteroids to launch; output shows live and is saved to asteroidloop-log.txt.
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File tools\run-logged.ps1 asteroidloop-log.txt "cargo run --release --bin asteroidloop -- --seeds 5 --limit-min 90"
pause
