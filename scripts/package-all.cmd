@echo off
setlocal
cd /d "%~dp0.."

if exist "dist" rmdir /s /q "dist"
mkdir "dist"
if errorlevel 1 exit /b %ERRORLEVEL%

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
echo   dist\CanvasTerminal.exe
echo   dist\canvas.exe
echo   dist\SKILL.md
if exist "dist\fonts" echo   dist\fonts\
if exist "dist\starship.toml" echo   dist\starship.toml
