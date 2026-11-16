@echo off
rem An Earth-size world covered in cutters: about 240 million (all its
rem surface can keep at work), at 10x, the camera stepping through: the
rem whole planet while the swarm grows, the whole planet, mid zoom, close
rem up, a pan along the surface, and the whole planet again, with a picture
rem at each (logs\snap-*.png). About 9 minutes. The window opens behind your
rem other windows; you can keep working.
rem Far from the camera the swarm works a kilometre of surface at a time as
rem a count of cutters; where the camera looks it comes down as flocks.
rem At the end it prints, for each camera shot, the speed the simulation
rem kept up ("achieved 9.9x"), the frame rate, and the memory used.
setlocal
cd /d "%~dp0game"
cargo build --release
if errorlevel 1 (
  echo The build failed.
  pause
  exit /b 1
)
target\release\dark-forest.exe --body earth-test --bench-drones 240000000 --speed 4 --camera "grow:400,far:30,snap:2,mid:30,snap:2,near:20,snap:2,pan:30,snap:1,far:20" --background --no-autosave
cd /d "%~dp0"
for /f "delims=" %%f in ('dir /b /o-d logs\game-*.jsonl') do (
  set LAST=%%f
  goto :found
)
:found
python tools\logsum.py logs\%LAST% | findstr /C:"-- " /C:"achieved" /C:"frames:" /C:"peak memory"
pause
