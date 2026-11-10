@echo off
rem Benchmark: how fast the simulation runs on this PC, natively (no browser).
rem Same bodies and settings as the viewer's Benchmark button, so the two can be
rem compared. Results go to out\benchmark.txt. Takes about 5 to 10 minutes.
cd /d "%~dp0"
if not exist out mkdir out
echo Building...
cargo build --release --bin stress --bin scaletest
if errorlevel 1 goto :eof
set OUT=out\benchmark.txt
echo Benchmark %date% %time% on %COMPUTERNAME%, %NUMBER_OF_PROCESSORS% threads > %OUT%
echo. >> %OUT%
call :run "Rubble asteroid, 10 game minutes" rubble-asteroid "PLAY_S=600" "EVERY_S=120"
call :run "Volcanic planetoid, 10 game minutes" active-planetoid "PLAY_S=600" "EVERY_S=120"
call :run "Living world, 1 game hour" living-world "PLAY_S=3600" "EVERY_S=600"
call :run "Sulfur world, 5,000 drones" sulfur-world "START=5000" "CAP=50000" "PLAY_S=300" "EVERY_S=60"
call :run "Frozen rogue world, 100,000 drones" rogue-world "START=100000" "CAP=200000" "PLAY_S=180" "EVERY_S=60"
call :run "Living world resized to 1,000 km, glamour 1" living-world "RADIUS_KM=1000" "GLAMOUR=1" "PLAY_S=300" "EVERY_S=100"
echo ==== Generator cost per cell, volcanic planetoid, glamour 0.5 ==== >> %OUT%
set GLAMOUR=0.5
set SKIP_SWARM=1
target\release\scaletest.exe active-planetoid >> %OUT% 2>&1
set GLAMOUR=
set SKIP_SWARM=
echo. >> %OUT%
type %OUT%
echo.
echo Saved to %OUT%
pause
goto :eof

:run
echo Running %~1...
echo ==== %~1 ==== >> %OUT%
setlocal
set START=
set CAP=
set RADIUS_KM=
set GLAMOUR=
set %~3
if not "%~4"=="" set %~4
if not "%~5"=="" set %~5
if not "%~6"=="" set %~6
if not "%~7"=="" set %~7
if not "%~8"=="" set %~8
target\release\stress.exe %2 >> %OUT% 2>&1
endlocal
echo. >> %OUT%
goto :eof
