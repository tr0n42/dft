@echo off
rem Stress tests: big swarms, big bodies, long runs. Results also in out\stress.txt.
cd /d "%~dp0"
if not exist out mkdir out
cargo build --release --bin stress
(
set START=20000& set CAP=50000& set PLAY_S=180& set EVERY_S=30& target\release\stress.exe active-planetoid
set START=100000& set CAP=200000& set PLAY_S=180& set EVERY_S=60& target\release\stress.exe rogue-world
set START=& set CAP=& set RADIUS_KM=1000& set PLAY_S=300& set EVERY_S=100& target\release\stress.exe living-world
set RADIUS_KM=& set PLAY_S=3600& set EVERY_S=600& target\release\stress.exe living-world
) > out\stress.txt 2>&1
type out\stress.txt
pause
