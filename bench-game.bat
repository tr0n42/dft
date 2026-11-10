@echo off
rem A repeatable game benchmark: three runs that close by themselves and each
rem leave a log in the logs folder (about 5 minutes in all). Claude reads them
rem with tools\logsum.py. Don't touch the mouse or keyboard while they run.
cd /d "%~dp0game"
cargo build --release
if errorlevel 1 (
  echo The build failed.
  pause
  exit /b 1
)
echo 1 of 3: rubble asteroid, 20000 drones, as fast as possible, 60 s
cargo run --release -- --body rubble-asteroid --drones 20000 --speed 8 --background --quit-after 60
echo 2 of 3: living world 2000 km, 200000 drones, as fast as possible, whole body in view, 150 s
cargo run --release -- --body living-world --radius-km 2000 --drones 200000 --speed 8 --background --quit-after 150
echo 3 of 3: active planetoid, 50000 drones, 30x, 90 s
cargo run --release -- --body active-planetoid --drones 50000 --speed 5 --background --quit-after 90
echo Done. The logs are in the logs folder.
pause
