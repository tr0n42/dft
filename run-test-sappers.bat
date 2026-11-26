@echo off
rem Sapper testbed: a small world of hard rock under a thin skin, with 20,000
rem free cutters. Within a few minutes the cutters hit granite and the
rem director starts sending sappers (made to order, at most one every 90 s).
rem Watch one land, crawl or drill to 79 m (its 5 kt yield's depth), arm
rem (its light blinking faster) and go off: the ground over it is loosened
rem up to the surface, cutters caught in it die, haulers near it are hurt.
rem F1 shows the sappers, blasts, cutters killed. The first build is slow.
cd /d "%~dp0game"
cargo run --release -- --body test-sappers --units sapper --bench-drones 20000 %*
if errorlevel 1 (
  echo.
  echo If the build failed asking for a newer Rust, run:  rustup update stable
  pause
)
