@echo off
rem Plant life cycle test: a 5 km living world.
rem Watch the ground behind the drones. A few minutes after plants are cut,
rem eaten or burned, ground cover creeps back and new plants sprout and grow
rem (often a different kind than before). Felled trees die (they go grey-brown
rem as deadwood), then rot to leaf litter and finally to dark humus soil.
rem F1 opens the developer panel: at the bottom of the production lines, "Life:"
rem counts places waiting to grow back, plants growing and cells rotting.
rem Speed keys help: regrowth takes about 4 minutes, a tree 15 minutes to grow.
rem 1x speed, action cam off, no fog, no autosave. A log goes to the logs folder.
cd /d "%~dp0game"
cargo run --release -- --body living-world --radius-km 5 --no-fog --no-autosave --dev
if errorlevel 1 (
  echo.
  echo The build or the game failed; see the messages above.
  pause
)
