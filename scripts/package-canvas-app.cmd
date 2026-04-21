@echo off
setlocal
cd /d "%~dp0.."

set "APP_BIN=egui_node_graph_mvp"
set "APP_EXE_NAME=CanvasTerminal.exe"
set "DIST_ROOT=dist\app"

echo Building Canvas app (release)...
cargo build --release --bin %APP_BIN%
if errorlevel 1 exit /b %ERRORLEVEL%

if exist "%DIST_ROOT%" rmdir /s /q "%DIST_ROOT%"
mkdir "%DIST_ROOT%"
if errorlevel 1 exit /b %ERRORLEVEL%

copy /y "target\release\%APP_BIN%.exe" "%DIST_ROOT%\%APP_EXE_NAME%" >nul
if errorlevel 1 exit /b %ERRORLEVEL%

if exist "assets\fonts" (
  xcopy /e /i /y "assets\fonts" "%DIST_ROOT%\fonts" >nul
  if errorlevel 1 exit /b %ERRORLEVEL%
)

if exist "assets\starship.toml" (
  copy /y "assets\starship.toml" "%DIST_ROOT%\starship.toml" >nul
  if errorlevel 1 exit /b %ERRORLEVEL%
)

echo.
echo Packaged Canvas app:
echo   %DIST_ROOT%\%APP_EXE_NAME%
if exist "%DIST_ROOT%\fonts" echo   %DIST_ROOT%\fonts\
if exist "%DIST_ROOT%\starship.toml" echo   %DIST_ROOT%\starship.toml
