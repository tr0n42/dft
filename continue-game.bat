@echo off
rem Builds and starts the game from the newest save in the saves folder
rem (an autosave, a quicksave, or one made from the F1 panel).
rem To load a particular one: run-game.bat --load autosave-2   (or a file path)
cd /d "%~dp0game"
cargo run --release -- --continue %*
if errorlevel 1 (
  echo.
  echo No save to continue, or the build failed. run-game.bat starts a new game.
  pause
)
