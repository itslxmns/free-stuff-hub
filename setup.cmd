@echo off
setlocal EnableDelayedExpansion
title Free Stuff Hub - Setup
cd /d "%~dp0"

set "APP=%LOCALAPPDATA%\Free Stuff Hub\free-stuff-hub.exe"
set "BUNDLE=src-tauri\target\release\bundle\nsis"

rem ---- 1. Already installed: just start it --------------------------------
if exist "%APP%" if /i not "%~1"=="--rebuild" (
  echo Starting Free Stuff Hub...
  start "" "%APP%"
  exit /b 0
)

echo.
echo  Free Stuff Hub setup
echo  ====================
echo.

rem ---- 2. A built installer is already here: run it -----------------------
if /i not "%~1"=="--rebuild" (
  call :newest
  if defined NEWEST (
    echo Installing from !NEWEST! ...
    "%BUNDLE%\!NEWEST!" /S
    goto :launch
  )
)

rem ---- 3. Build from source --------------------------------------------------
where node >nul 2>nul || (
  echo [x] Node.js is not installed. Get it from https://nodejs.org ^(LTS^), then run setup.cmd again.
  start "" https://nodejs.org
  goto :fail
)
where cargo >nul 2>nul || (
  echo [x] Rust is not installed. Get it from https://rustup.rs, then run setup.cmd again.
  echo     Rust also needs "Desktop development with C++" from Visual Studio Build Tools.
  start "" https://rustup.rs
  goto :fail
)

echo [1/3] Installing npm packages...
call npm install --no-fund --no-audit || goto :fail

echo [2/3] Building the app ^(first build takes 5-10 minutes^)...
call npm run tauri build || goto :fail

echo [3/3] Installing...
call :newest
if defined NEWEST (
  "%BUNDLE%\!NEWEST!" /S
  goto :launch
)
echo [x] Build finished but no installer was found in %BUNDLE%.
goto :fail

:launch
if not exist "%APP%" (
  echo [x] Install did not complete.
  goto :fail
)
echo Done. Free Stuff Hub is installed ^(desktop + Start menu shortcuts^) and starting now.
start "" "%APP%"
ping -n 3 127.0.0.1 >nul
exit /b 0

:fail
echo.
echo Setup failed. See the messages above.
pause
exit /b 1

rem Sets NEWEST to the most recently built installer file name (if any).
:newest
set "NEWEST="
for /f "delims=" %%F in ('dir /b /o-d "%BUNDLE%\*-setup.exe" 2^>nul') do (
  if not defined NEWEST set "NEWEST=%%F"
)
exit /b 0
