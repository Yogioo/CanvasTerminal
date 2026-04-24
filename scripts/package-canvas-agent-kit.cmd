@echo off
setlocal
cd /d "%~dp0.."

set "SKILL_NAME=canvas-agent-events"
set "SRC_SKILL=.pi\skills\%SKILL_NAME%"
set "DIST_ROOT=dist"

echo Building canvas CLI (release)...
cargo build --release --bin canvas
if errorlevel 1 exit /b %ERRORLEVEL%

if not exist "%DIST_ROOT%" mkdir "%DIST_ROOT%"
if errorlevel 1 exit /b %ERRORLEVEL%

copy /y "%SRC_SKILL%\SKILL.md" "%DIST_ROOT%\SKILL.md" >nul
if errorlevel 1 exit /b %ERRORLEVEL%
copy /y "target\release\canvas.exe" "%DIST_ROOT%\canvas.exe" >nul
if errorlevel 1 exit /b %ERRORLEVEL%

echo.
echo Packaged agent kit:
echo   %DIST_ROOT%\SKILL.md
echo   %DIST_ROOT%\canvas.exe
