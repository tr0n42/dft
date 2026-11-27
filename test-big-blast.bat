@echo off
rem A test blast on the Dev planet: a buried charge of the size you type goes
rem off under the middle of the view, 32 game seconds in (fog of war off), to
rem see how a bigger sapper charge would look and how the game holds up.
rem Sappers are 10 kt today. Since the 27 Nov blast work (see
rem docs/explosions-research.md) up to 10000 kt holds about 60 fps on this PC.
rem Scroll to zoom out for the big ones.
set kt=100
set /p kt=How many kilotons? (Enter for 100):
cd /d "%~dp0game"
cargo run --release -- --body dev-planet --no-fog --view 0,5130,0.6 --test-blast %kt%
if errorlevel 1 (
  echo.
  echo If the build failed asking for a newer Rust, run:  rustup update stable
  pause
)
