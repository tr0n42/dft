@echo off
rem Lava test, the fastest way: a small lava world (5 km, magma just under a
rem thin glassy skin), the real economy (1,000 cutters to start, no cap),
rem running at 1x with the action cam off (press C to have the camera jump to lava
rem breaches, drones melting and fires). No fog, no autosave; the F1 panel opens by itself.
rem Watch the "heat" line in the panel: cutters melted, cells burned away.
rem Keys: 1-8 change speed (4 = 10x, 5 = 30x), C toggles the action cam,
rem Home shows the whole body, Space pauses.
cd /d "%~dp0game"
cargo run --release -- --body lava-world --radius-km 5 --no-fog --no-autosave --dev %*
if errorlevel 1 (
  echo.
  echo The build or the game failed; see the messages above.
  pause
)
