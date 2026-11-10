@echo off
rem Lava test with half a million drones: the same small lava world as
rem test-lava.bat (5 km), with the swarm grown to 500,000 drones,
rem action cam off, no fog, no autosave, F1 panel open. Just double-click.
rem Every run writes a log to the logs folder for Claude to read.
cd /d "%~dp0game"
cargo run --release -- --body lava-world --radius-km 5 --drones 500000 --no-fog --no-autosave --dev
if errorlevel 1 (
  echo.
  echo The build or the game failed; see the messages above.
  pause
)
