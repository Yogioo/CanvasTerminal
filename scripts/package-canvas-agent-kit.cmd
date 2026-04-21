@echo off
setlocal
cd /d "%~dp0.."

set "SKILL_NAME=canvas-agent-events"
set "SRC_SKILL=.pi\skills\%SKILL_NAME%"
set "DIST_ROOT=dist\canvas_skills\%SKILL_NAME%"
set "DIST_BIN=%DIST_ROOT%\bin"
set "DIST_SCRIPTS=%DIST_ROOT%\scripts"

echo Building canvas CLI (release)...
cargo build --release --bin canvas
if errorlevel 1 exit /b %ERRORLEVEL%

if exist "%DIST_ROOT%" rmdir /s /q "%DIST_ROOT%"
mkdir "%DIST_BIN%"
if errorlevel 1 exit /b %ERRORLEVEL%
mkdir "%DIST_SCRIPTS%"
if errorlevel 1 exit /b %ERRORLEVEL%

copy /y "%SRC_SKILL%\SKILL.md" "%DIST_ROOT%\SKILL.md" >nul
if errorlevel 1 exit /b %ERRORLEVEL%
copy /y "%SRC_SKILL%\scripts\canvas.cmd" "%DIST_SCRIPTS%\canvas.cmd" >nul
if errorlevel 1 exit /b %ERRORLEVEL%
copy /y "target\release\canvas.exe" "%DIST_BIN%\canvas.exe" >nul
if errorlevel 1 exit /b %ERRORLEVEL%

echo.
echo Packaged agent kit:
echo   %DIST_ROOT%
echo.
echo Contents:
echo   %DIST_ROOT%\SKILL.md
echo   %DIST_ROOT%\bin\canvas.exe
echo   %DIST_ROOT%\scripts\canvas.cmd
