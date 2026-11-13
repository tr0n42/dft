@echo off
rem The Dev planet: a small 5 km test world where every drone in the game has
rem work (cutters, haulers, sappers, tunnelers, heat-sink bots, foresters and
rem abductors). Herds of placeholder animals graze the land; they panic and
rem run (white tail up) when a machine comes near, and abductors snatch them.
rem F1 shows each drone's numbers, and the animals: left, caught, lost.
rem The first build takes several minutes.
cd /d "%~dp0game"
cargo run --release -- --body dev-planet %*
if errorlevel 1 (
  echo.
  echo If the build failed asking for a newer Rust, run:  rustup update stable
  pause
)
