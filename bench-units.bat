@echo off
rem "500k cutters with X": the staged unit benchmark. 500,000 cutters on the
rem living world (zoomed out), with every stage up to the one in
rem perf\stage.txt: 1 = cutters + haulers, 2 = + sappers, 3 = + tunnelers,
rem 4 = + heat-sink bots, 5 = + foresters and abductors. About 3 minutes a
rem stage. Shows each unit's cost and the change since the last run on this
rem PC, and adds a row per stage to perf\history.csv.
cd /d "%~dp0"
cargo build --release --bin unitbench
if errorlevel 1 (
  echo The build failed.
  pause
  exit /b 1
)
target\release\unitbench.exe
pause
