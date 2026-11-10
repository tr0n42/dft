@echo off
rem A plain world with no lava, water or air: the barren world at 5 km with a
rem swarm of 10,000 drones (economy off), at 1x, action cam off, no fog, no
rem autosave, F1 panel open. Just double-click.
rem Every run writes a log to the logs folder for Claude to read.
cd /d "%~dp0game"
cargo run --release -- --body barren-world --radius-km 5 --drones 10000 --no-fog --no-autosave --dev
if errorlevel 1 (
  echo.
  echo The build or the game failed; see the messages above.
  pause
)
