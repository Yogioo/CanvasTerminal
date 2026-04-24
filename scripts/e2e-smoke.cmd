@echo off
setlocal enabledelayedexpansion

set CANVAS_API=http://127.0.0.1:4545
set CANVAS_EXE=dist\canvas.exe

if not exist "%CANVAS_EXE%" (
  echo [smoke] missing %CANVAS_EXE%, run scripts\package-all.cmd first.
  exit /b 1
)

echo [smoke] ping canvas app...
"%CANVAS_EXE%" ping >nul 2>nul
if errorlevel 1 (
  echo [smoke] canvas app not running, skip smoke.
  exit /b 0
)

echo [smoke] graph get
"%CANVAS_EXE%" debug graph get --pretty >nul
if errorlevel 1 exit /b 1

echo [smoke] create nodes
for /f "tokens=*" %%i in ('"%CANVAS_EXE%" debug node create --kind text --x 120 --y 100 --text "smoke-a" --jsonpath data.node_id') do set NODE_A=%%i
for /f "tokens=*" %%i in ('"%CANVAS_EXE%" debug node create --kind text --x 380 --y 120 --text "smoke-b" --jsonpath data.node_id') do set NODE_B=%%i

if "%NODE_A%"=="" exit /b 1
if "%NODE_B%"=="" exit /b 1

echo [smoke] connect edge
"%CANVAS_EXE%" debug edge create --from %NODE_A% --to %NODE_B% >nul
if errorlevel 1 exit /b 1

echo [smoke] inject text
"%CANVAS_EXE%" debug inject text --node-id %NODE_A% --mode append --text "\nsmoke" >nul
if errorlevel 1 exit /b 1

echo [smoke] inject terminal command
"%CANVAS_EXE%" debug inject terminal --node-id %NODE_A% --command "echo smoke-terminal" --wait --timeout 5000 >nul
if errorlevel 1 exit /b 1

echo [smoke] restart built-in terminal node
"%CANVAS_EXE%" debug terminal restart --node-id 1 >nul
if errorlevel 1 exit /b 1

echo [smoke] verify graph after
"%CANVAS_EXE%" debug graph get --pretty >nul
if errorlevel 1 exit /b 1

echo [smoke] ok
exit /b 0
