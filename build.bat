@echo off
REM Wrapper script for build.ps1
REM This allows easy execution without needing to change PowerShell execution policy

echo Building IntuneDeviceDatabaseSynchronization...
powershell.exe -ExecutionPolicy Bypass -File "%~dp0build.ps1" %*

if %ERRORLEVEL% neq 0 (
    echo Build failed with error code %ERRORLEVEL%
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo Build completed successfully!
pause
