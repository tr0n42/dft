@echo off
rem The economy on: a plain 5 km rocky world (no lava, water or air). The
rem mother swarm arrives with 1,000 cutters and builds more from the elements
rem it is sent, only as many as there is work for. F1 shows the stockpile and
rem what the cutters are being made of. Rest the mouse on the ground for two
rem seconds to see what it is. 1x speed, action cam off, no fog, no autosave.
rem Every run writes a log to the logs folder for Claude to read.
cd /d "%~dp0game"
cargo run --release -- --body barren-world --radius-km 5 --economy --no-fog --no-autosave --dev
if errorlevel 1 (
  echo.
  echo The build or the game failed; see the messages above.
  pause
)
