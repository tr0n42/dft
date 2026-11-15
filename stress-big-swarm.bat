@echo off
rem Big swarm stress test in the real game: the living world (35 km) covered
rem in cutters (1.5 million, about all its surface can keep at work), at
rem 10x, the camera stepping through: the whole planet while the swarm
rem grows, the whole planet, mid zoom, close up, a pan along the surface,
rem and the whole planet again, with a picture at each (logs\snap-*.png).
rem About 9 minutes. The window opens behind your other windows; you can
rem keep working.
rem At the end it prints, for each camera shot, the speed the simulation
rem kept up ("achieved 9.9x") and the frame rate. Claude reads the same log
rem from the logs folder.
setlocal
cd /d "%~dp0game"
cargo build --release
if errorlevel 1 (
  echo The build failed.
  pause
  exit /b 1
)
target\release\dark-forest.exe --body living-world --bench-drones 1500000 --speed 4 --camera "grow:300,far:30,snap:2,mid:20,snap:2,near:20,snap:2,pan:30,snap:1,far:20" --background --no-autosave
cd /d "%~dp0"
for /f "delims=" %%f in ('dir /b /o-d logs\game-*.jsonl') do (
  set LAST=%%f
  goto :found
)
:found
python tools\logsum.py logs\%LAST% | findstr /C:"-- " /C:"achieved" /C:"frames:"
pause
