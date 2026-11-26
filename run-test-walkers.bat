@echo off
rem Foresters and abductors on the Dev planet, with 20,000 free cutters.
rem They fly folded from the mother swarm like UFOs (past its edge if need
rem be), drop, unfold and stride to the thickest life. Foresters shear
rem trees into their bags and set the ground cover alight (fire spreads);
rem abductors snatch animals alive and laser the ones that spit (green
rem spines) or run, bursting them. When building a new one would get to
rem the next work sooner, one powers down and the couriers reclaim it.
cd /d "%~dp0game"
cargo run --release -- --body dev-planet --units forester,abductor --bench-drones 20000 %*
if errorlevel 1 (
  echo.
  echo If the build failed asking for a newer Rust, run:  rustup update stable
  pause
)
