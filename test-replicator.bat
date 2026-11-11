@echo off
rem The Universal Drone Replicator, for testing every specialist at once: a
rem 5 km living world, which has plants (foresters) and magma (heat-sink
rem bots) as well as rock for sappers and tunnelers. The replicator only
rem builds what a world has work for, and only while there is work.
rem F1 opens the developer panel: its production lines say what each bay is
rem building, how many of each specialist are out, and what they have done.
rem Sappers: drop where the going is hard, crawl down cracks (or drill), and
rem go off deep in the rock. Anything that destroys one sets it off.
rem 1x speed, action cam off, no fog, no autosave. A log goes to the logs folder.
cd /d "%~dp0game"
cargo run --release -- --body living-world --radius-km 5 --no-fog --no-autosave --dev
if errorlevel 1 (
  echo.
  echo The build or the game failed; see the messages above.
  pause
)
