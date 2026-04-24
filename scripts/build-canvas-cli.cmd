@echo off
setlocal
cd /d "%~dp0.."

echo [deprecated] Use scripts\package-all.cmd for unified packaged workflow.
call scripts\package-all.cmd
exit /b %ERRORLEVEL%
