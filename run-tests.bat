@echo off
rem Runs every test group with live progress; also saved to test-results.txt.
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File tools\run-tests.ps1
pause
