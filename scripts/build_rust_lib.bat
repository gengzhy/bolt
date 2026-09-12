@echo off
rem ============================================================
rem 构建 Windows 端 Rust 产物：bt_ffi.dll + bolt-cli.exe（Release）
rem 用法：scripts\build_rust_lib.bat
rem ============================================================
setlocal
cd /d "%~dp0.."

cargo build -p ffi -p bolt-cli --release
if errorlevel 1 (
    echo [FAIL] cargo build failed
    exit /b 1
)

if not exist lib\win64 mkdir lib\win64
copy /Y target\release\bt_ffi.dll lib\win64\bt_ffi.dll >nul
copy /Y target\release\bt_ffi.dll.lib lib\win64\bt_ffi.dll.lib >nul 2>nul

echo [OK] lib\win64\bt_ffi.dll
echo [OK] target\release\bolt-cli.exe
endlocal
