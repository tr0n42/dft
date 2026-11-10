@echo off
rem Opens the Planet Viewer in your default browser, straight from disk.
rem Rebuild it after changes with: cd viewer, cargo build --release --target wasm32-unknown-unknown, python build_page.py
cd /d "%~dp0"
start "" "viewer\dist\Planet Viewer.html"
