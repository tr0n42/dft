@echo off
rem Stress test: 500,000 drones on the living world, zoomed out, 20 game
rem minutes (about 3 minutes real). Each report shows real seconds per game
rem minute: under 60 keeps up with 1x speed, under 7.5 with 8x.
cd /d "%~dp0"
cargo build --release --bin stress
if errorlevel 1 (
  echo The build failed.
  pause
  exit /b 1
)
set START=500000
set FREE=500000
set ATTENTION=none
set PLAY_S=1200
set EVERY_S=120
set WALL_S=900
target\release\stress.exe living-world | findstr /B /C:"  t " /C:"           events" /C:"  done"
pause
