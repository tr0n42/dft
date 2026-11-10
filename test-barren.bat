@echo off
rem A plain 5 km rocky world (no lava, water or air). The mother swarm
rem arrives with 1,000 cutters and builds more from parts made of the
rem elements it is sent, only as many as there is work for, with no cap. The
rem stockpile window (top right) shows the elements, materials and parts. Rest the mouse on the ground for two
rem seconds to see what it is. 1x speed, action cam off, no fog, no autosave.
rem Every run writes a log to the logs folder for Claude to read.
cd /d "%~dp0game"
cargo run --release -- --body barren-world --radius-km 5 --no-fog --no-autosave --dev
if errorlevel 1 (
  echo.
  echo The build or the game failed; see the messages above.
  pause
)
