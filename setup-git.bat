@echo off
setlocal
rem Puts The Dark Forest Theory (in Downloads\DarkForestTheory-source) in the GitHub repo "dft".
rem Plain git commands, run where the files are. Safe to run again.

set "SRC=%USERPROFILE%\Downloads\DarkForestTheory-source"
if not exist "%SRC%\Cargo.toml" for /d %%D in ("%SRC%\*") do if exist "%%D\Cargo.toml" set "SRC=%%D"
if not exist "%SRC%\Cargo.toml" (
  echo Can't find the project ^(Cargo.toml^) in %USERPROFILE%\Downloads\DarkForestTheory-source.
  pause
  exit /b 1
)
cd /d "%SRC%"
echo Project folder: %CD%
echo.

set "URL="
set /p "URL=GitHub repo URL (for example https://github.com/YOURNAME/dft.git): "
if "%URL%"=="" (
  echo No URL given.
  pause
  exit /b 1
)

echo.
echo == Ignore rules and line endings
(
  echo target/
  echo logs/
  echo saves/
  echo out/
  echo *-log.txt
  echo viewer/pkg/
  echo viewer/dist/
) > .gitignore
(
  echo * text=auto eol=lf
  echo *.bat text eol=crlf
  echo *.ps1 text eol=crlf
  echo *.png binary
) > .gitattributes

echo.
echo == Commit
if not exist .git git init -b main
git add .
git commit -m "Initial import: The Dark Forest Theory"

echo.
echo == Push
git remote remove origin 2>nul
git remote add origin %URL%
git branch -M main
git push -u origin main
if errorlevel 1 (
  echo.
  echo Push failed: see the message above.
) else (
  echo.
  echo Done: %URL%
)
pause