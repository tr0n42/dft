@echo off
rem Checks the Rust install, the Visual Studio linker and Python; writes rust-check.txt.
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File tools\check-rust.ps1
pause
