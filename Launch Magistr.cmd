@echo off
setlocal
where node >nul 2>nul
if errorlevel 1 (
    echo Install Node.js 22 or later from https://nodejs.org, then try again.
    pause
    exit /b 1
)
node "%~dp0scripts\desktop.mjs" open %*
set "launch_result=%errorlevel%"
if not "%launch_result%"=="0" pause
exit /b %launch_result%
