@echo off
setlocal
cd /d "%~dp0\.."
echo ===================================================
echo   Bolt Windows One-Click Packaging Pipeline
echo ===================================================
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build_windows_dist.ps1" %*
if errorlevel 1 (
    echo.
    echo [ERROR] Packaging failed with exit code %errorlevel%.
    pause
    exit /b %errorlevel%
)
echo.
echo [SUCCESS] All packages ready in dist\release\
pause
