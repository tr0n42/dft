@echo off
rem Builds and starts the game. The first build takes several minutes.
rem Options go after the name, e.g.: run-game.bat --body living-world --radius-km 300 --glamour 0.6
rem Controls: drag with either mouse button (or arrows / WASD) to pan, the wheel or
rem Page Up / Page Down zooms, Home shows the whole body, C turns the action cam on/off
rem (off at the start; starts with it on, --zoom-limit limits zooming out).
rem Space pauses, 1-8 pick a speed (1x 2x 5x 10x 30x 100x 300x max), + and - step it.
rem The real economy: the swarm arrives with 1,000 cutters and builds more from parts, with no cap.
rem Every session writes a performance log to the logs folder (bench-suite.bat runs a fixed set).
rem F1 opens the developer panel (numbers, speed, a body/size/seed/glamour picker, saves).
rem Saves: F5 saves, F9 loads the newest; the game autosaves every 5 minutes and on closing
rem (saves folder, last three autosaves kept). continue-game.bat picks up the newest save.
cd /d "%~dp0game"
cargo run --release -- %*
if errorlevel 1 (
  echo.
  echo If the build failed asking for a newer Rust, run:  rustup update stable
  pause
)
