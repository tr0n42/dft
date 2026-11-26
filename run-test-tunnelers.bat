@echo off
rem Tunneler and heat-sink testbed: ore stocks in a granite crust and a magma
rem layer about 450 m down, 20,000 free cutters. Tunnelers fly down, bore
rem curving shafts (their crews carrying the spoil off the mouth in a
rem plume), reach the magma in about 10 minutes and cap it; heat sinks come
rem down the shaft and freeze a passage through it (their crews digging out
rem the middle); then a tunneler goes on down through the passage.
rem F1 shows the tunnelers, caps, heat sinks and passages.
cd /d "%~dp0game"
cargo run --release -- --body test-tunnelers --units tunneler,heatsink --bench-drones 20000 %*
if errorlevel 1 (
  echo.
  echo If the build failed asking for a newer Rust, run:  rustup update stable
  pause
)
