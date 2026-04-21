@echo off
setlocal
cd /d "%~dp0.."

echo Building canvas CLI...
cargo build --bin canvas
if errorlevel 1 exit /b %ERRORLEVEL%

echo.
echo Done: target\debug\canvas.exe
