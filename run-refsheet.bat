@echo off
rem Rebuilds the reference sheets (pictures and a spec sheet per body) in reference\.
cd /d "%~dp0"
cargo run --release --bin refsheet -- --out reference
pause
