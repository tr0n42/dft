@echo off
rem Every ground unit at once on the Dev planet, with 20,000 free cutters:
rem sappers (where cutters struggle with hard rock), tunnelers, heat sinks,
rem foresters and abductors.
cd /d "%~dp0game"
cargo run --release -- --body dev-planet --units all --bench-drones 20000 %*
if errorlevel 1 (
  echo.
  echo If the build failed asking for a newer Rust, run:  rustup update stable
  pause
)
