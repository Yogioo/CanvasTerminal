@echo off
setlocal
cd /d "%~dp0.."

echo Building canvas CLI (release)...
cargo build --release --bin canvas
if errorlevel 1 exit /b %ERRORLEVEL%

echo.
echo Done: target\release\canvas.exe
