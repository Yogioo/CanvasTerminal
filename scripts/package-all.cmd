@echo off
setlocal
cd /d "%~dp0.."

echo Packaging Canvas app...
call scripts\package-canvas-app.cmd
if errorlevel 1 exit /b %ERRORLEVEL%

echo.
echo Packaging Canvas agent skill kit...
call scripts\package-canvas-agent-kit.cmd
if errorlevel 1 exit /b %ERRORLEVEL%

echo.
echo All packages built successfully.
echo.
echo Outputs:
echo   dist\app\CanvasTerminal.exe
echo   dist\canvas_skills\canvas-agent-events\
