@echo off
setlocal
set "ROOT=%~dp0"
set "EXE=%ROOT%target\debug\canvas.exe"

if exist "%EXE%" (
  "%EXE%" %*
  exit /b %ERRORLEVEL%
)

echo [canvas.cmd] canvas.exe not found, building once...
cargo build --bin canvas
if errorlevel 1 exit /b %ERRORLEVEL%

"%EXE%" %*
exit /b %ERRORLEVEL%
