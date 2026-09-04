@echo off
rem ============================================================
rem 清理全部构建产物（cargo clean + target 目录）
rem 用法：scripts\clean_all.bat
rem ============================================================
setlocal
cd /d "%~dp0.."

cargo clean
if exist target rmdir /s /q target
if exist lib\win64 rmdir /s /q lib\win64

echo [OK] 清理完成
endlocal
