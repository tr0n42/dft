@echo off
rem Flora tool: grows life on any body and shows it.
rem Writes report.txt, gallery.png (every species) and scenes.png (the body
rem itself) into out\flora\<body>-<seed>, then opens the pictures.
cd /d "%~dp0"
echo Building the flora tool...
cargo build --release --bin floragen || (pause & exit /b 1)

:again
echo.
echo Bodies:
for %%f in (data\bodies\*.toml) do echo   %%~nf
echo.
set BODY=
set SEED=
set RADIUS=
set GLAMOUR=
set LIFE=
set /p BODY=Body [living-world]: 
if "%BODY%"=="" set BODY=living-world
set /p SEED=Seed [press Enter for the body's own]: 
set /p RADIUS=Radius in km [press Enter for the body's own]: 
set /p GLAMOUR=Glamour, 0 to 1 [0.4]: 
if "%GLAMOUR%"=="" set GLAMOUR=0.4
set /p LIFE=Life, 0 to 2 [1]: 
if "%LIFE%"=="" set LIFE=1

set ARGS=--body %BODY% --glamour %GLAMOUR% --life %LIFE%
set TAG=%BODY%
if not "%SEED%"=="" set ARGS=%ARGS% --seed %SEED%
if not "%SEED%"=="" set TAG=%TAG%-%SEED%
if not "%RADIUS%"=="" set ARGS=%ARGS% --radius-km %RADIUS%
if not "%RADIUS%"=="" set TAG=%TAG%-%RADIUS%km
set TAG=%TAG%-g%GLAMOUR%
set OUT=out\flora\%TAG%

target\release\floragen.exe %ARGS% --out "%OUT%"
if errorlevel 1 goto ask
if exist "%OUT%\gallery.png" start "" "%OUT%\gallery.png"
if exist "%OUT%\scenes.png" start "" "%OUT%\scenes.png"

:ask
echo.
set AGAIN=
set /p AGAIN=Grow another? (y/n) [y]: 
if /i "%AGAIN%"=="n" exit /b 0
goto again
