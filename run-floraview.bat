@echo off
rem Flora viewer: browse the plants each world grows.
rem Pick a body type, seed and glamour on the left. Scroll the mouse wheel (or
rem use the arrow keys) to step through plants from that world; Regrow grows
rem another of the same species; Attack (or Space) plays its defence.
rem The first build takes a few minutes.
cd /d "%~dp0game"
cargo run --release --bin floraview -- %*
if errorlevel 1 (
  echo.
  echo If the build failed asking for a newer Rust, run:  rustup update stable
  pause
)
