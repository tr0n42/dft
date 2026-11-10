@echo off
rem The life benchmark: the benchmark suite on the living world only (plants
rem being eaten, cut trees falling), over swarms of growing size, each at
rem full speed with the camera stepping through every kind of shot (see game\src\bench.rs):
rem   grow (whole body while the swarm grows), far, sweep (zoom all the way in
rem   and out), mid (1.5 m a pixel: loose matter simulated on screen), near
rem   (single drones), save (the whole game, to
rem   saves\bench.dft), load (it back again), far.
rem Every run writes a log to the logs folder; Claude reads them all with
rem tools\logsum.py --suite. About 12 minutes. Closing a window just skips
rem to the next run.
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
call :run 1 living-world 35 20000 "%PLAN%"
call :run 2 living-world 300 50000 "%PLAN%"
call :run 3 living-world 2000 200000 "%PLAN%"
call :run 4 living-world 6000 500000 "%BIG%"
call :run 5 living-world 6000 1000000 "%BIG%"
echo.
echo Done: 5 runs. The logs are in the logs folder.
pause
exit /b 0

:run
echo %1 of 5: %2, %3 km, %4 drones
cargo run --release -- --body %2 --radius-km %3 --drones %4 --speed 8 --background --camera "%~5"
exit /b 0
