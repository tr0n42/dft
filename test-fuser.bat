@echo off
rem The fuser and sappers' charges: a 5 km lava world, which holds no uranium
rem at all. The fleet brings 200 kg (25 charges); once that is spent, the
rem refinery's fuser makes more from iron and calcium (iron + iron makes
rem tellurium, calcium + calcium makes zirconium, and the two make uranium).
rem F1 opens the developer panel. Watch these lines under the replicator:
rem   "sapper charges now": nuclear, conventional, or none yet
rem   "uranium left in rubble": what nuclear blasts left for the cutters,
rem      and how much the cutters have brought back
rem   "fuser": what each fusion has made, and the energy it took
rem Keys: 1-8 change speed (4 = 10x, 5 = 30x), Home shows the whole body,
rem Space pauses. No fog, no autosave.
cd /d "%~dp0game"
cargo run --release -- --body lava-world --radius-km 5 --no-fog --no-autosave --dev
if errorlevel 1 (
  echo.
  echo The build or the game failed; see the messages above.
  pause
)
