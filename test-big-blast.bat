@echo off
rem A test blast on the Dev planet: a charge of the size you type goes off
rem under the middle of the view (or on the ground, or over it), 32 game
rem seconds in (fog of war off), to see how a blast would look and how the
rem game holds up. Sappers are 10 kt today, buried, nuclear. Since the 27 Nov
rem blast work (see docs/explosions-research.md) up to 10000 kt holds about
rem 60 fps on this PC. Scroll to zoom out for the big ones.
rem How deep: Enter for as deep as a sapper buries it; 0 on the ground; a
rem minus number for that many metres up in the air (-150).
rem Nuclear (Enter) or conventional (type conv): see docs/explosion-sequence.md.
set kt=100
set /p kt=How many kilotons? (Enter for 100):
set depth=
set /p depth=How deep, in metres? (Enter for a sapper's depth, 0 on the ground, -150 for 150 m up):
set kind=
set /p kind=Nuclear or conventional? (Enter for nuclear, or type conv):
set arg=%kt%,%depth%
if /i "%kind%"=="conv" set arg=%kt%,%depth%,conv
cd /d "%~dp0game"
cargo run --release -- --body dev-planet --no-fog --view 0,5130,0.6 --test-blast %arg%
if errorlevel 1 (
  echo.
  echo If the build failed asking for a newer Rust, run:  rustup update stable
  pause
)
