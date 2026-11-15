@echo off
rem Big swarm stress test in the real game: 300,000 cutters on the living
rem world (35 km), at full speed, the camera stepping through: the whole
rem planet while the swarm grows, the whole planet, mid zoom, close up, and
rem the whole planet again. About 8 minutes. The window opens behind your
rem other windows; you can keep working.
rem At the end it prints, for each camera shot, the speed the simulation
rem kept up ("achieved 9.7x") and the frame rate. Claude reads the same log
rem from the logs folder.
setlocal
cd /d "%~dp0game"
cargo build --release
if errorlevel 1 (
  echo The build failed.
  pause
  exit /b 1
)
target\release\dark-forest.exe --body living-world --bench-drones 300000 --speed 8 --camera "grow:300,far:40,mid:30,near:30,far:30" --background --no-autosave
cd /d "%~dp0"
for /f "delims=" %%f in ('dir /b /o-d logs\game-*.jsonl') do (
  set LAST=%%f
  goto :found
)
:found
python tools\logsum.py logs\%LAST% | findstr /C:"-- " /C:"achieved" /C:"frames:"
pause
