@echo off
rem The ground units in numbers, no window: each testbed for 15 game
rem minutes, printing every game minute what the units are doing and what
rem they cost (ms of real time per game second). About 5 minutes in all.
cd /d "%~dp0"
cargo build --release --bin unitlab
if errorlevel 1 (
  echo The build failed.
  pause
  exit /b 1
)
echo.
echo ==== Sappers on hard rock (test-sappers) ====
set BODY=test-sappers
target\release\unitlab.exe sapper 900
echo.
echo ==== Tunnelers and heat sinks (test-tunnelers) ====
set BODY=test-tunnelers
target\release\unitlab.exe tunneler,heatsink 1500
echo.
echo ==== Foresters and abductors (dev-planet) ====
set BODY=dev-planet
target\release\unitlab.exe forester,abductor 900
pause
