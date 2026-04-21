@echo off
setlocal
set "ROOT=%~dp0"
set "EXE_DEBUG=%ROOT%target\debug\canvas.exe"
set "EXE_RELEASE=%ROOT%target\release\canvas.exe"
set "EXE_KIT=%ROOT%dist\canvas_skills\canvas-agent-events\bin\canvas.exe"

if exist "%EXE_DEBUG%" (
  "%EXE_DEBUG%" %*
  exit /b %ERRORLEVEL%
)

if exist "%EXE_RELEASE%" (
  "%EXE_RELEASE%" %*
  exit /b %ERRORLEVEL%
)

if exist "%EXE_KIT%" (
  "%EXE_KIT%" %*
  exit /b %ERRORLEVEL%
)

where cargo >nul 2>nul
if errorlevel 1 (
  echo [canvas.cmd] canvas.exe not found, and cargo is not available in PATH.
  echo [canvas.cmd] expected one of:
  echo   %EXE_DEBUG%
  echo   %EXE_RELEASE%
  echo   %EXE_KIT%
  exit /b 1
)

echo [canvas.cmd] canvas.exe not found, building once...
cargo build --bin canvas
if errorlevel 1 exit /b %ERRORLEVEL%

"%EXE_DEBUG%" %*
exit /b %ERRORLEVEL%
