@echo off
rem The benchmark suite: short scripted runs over bodies and swarms of growing
rem size, each at full speed with the camera stepping through every kind of
rem shot (see game\src\bench.rs):
rem   grow (whole body while the swarm grows), far, sweep (zoom all the way in
rem   and out), mid (1.5 m a pixel: loose matter simulated on screen), near
rem   (single drones), save (the whole game, to
rem   saves\bench.dft), load (it back again), far.
rem Every run writes a log to the logs folder; Claude reads them all with
rem tools\logsum.py --suite. About 18 minutes. Closing a window just skips
rem to the next run. (bench-life.bat runs the living world only.)
rem Each window opens behind your other windows without taking focus, so
rem you can keep working (it still runs and logs normally).
rem (The plans are quoted: a batch file splits arguments at commas.)
setlocal
cd /d "%~dp0game"
cargo build --release
if errorlevel 1 (
  echo The build failed.
  pause
  exit /b 1
)
set PLAN=grow:20,far:8,sweep:12,mid:10,near:10,save:5,load:10,far:5
set BIG=grow:35,far:8,sweep:12,mid:10,near:10,save:15,load:30,far:8
call :run 1 rubble-asteroid 2.5 5000 "%PLAN%"
call :run 2 rubble-asteroid 2.5 20000 "%PLAN%"
call :run 3 active-planetoid 30 20000 "%PLAN%"
call :run 4 active-planetoid 30 100000 "%PLAN%"
call :run 5 living-world 300 50000 "%PLAN%"
call :run 6 living-world 2000 200000 "%PLAN%"
call :run 7 active-planetoid 300 200000 "%BIG%"
call :run 8 living-world 6000 500000 "%BIG%"
call :run 9 living-world 6000 1000000 "%BIG%"
echo.
echo Done: 9 runs. The logs are in the logs folder.
pause
exit /b 0

:run
echo %1 of 9: %2, %3 km, %4 drones
cargo run --release -- --body %2 --radius-km %3 --drones %4 --speed 8 --background --camera "%~5"
exit /b 0
