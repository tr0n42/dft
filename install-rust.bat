@echo off
rem Installs Rust (stable, via rustup) for this user, then re-runs the Rust check.
rem Uses the Visual Studio 2026 C++ tools already on this PC; no admin rights needed.
cd /d "%~dp0"
echo Downloading the Rust installer (rustup-init.exe)...
powershell -NoProfile -ExecutionPolicy Bypass -Command "$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -Uri 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe' -OutFile \"$env:TEMP\rustup-init.exe\""
if not exist "%TEMP%\rustup-init.exe" (
  echo Download failed. Check the internet connection and try again.
  pause
  exit /b 1
)
echo.
echo Installing Rust stable. This takes a few minutes; progress shows below.
"%TEMP%\rustup-init.exe" -y --default-toolchain stable --profile default
if errorlevel 1 (
  echo Install failed; see the messages above.
  pause
  exit /b 1
)
rem The installer adds Rust to PATH for new windows; add it to this one too.
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
echo.
echo Install done. Checking it now...
powershell -NoProfile -ExecutionPolicy Bypass -File tools\check-rust.ps1
pause
