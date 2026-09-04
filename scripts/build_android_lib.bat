@echo off
rem ============================================================
rem 构建 Android 端 Rust FFI 动态库（liblt_ffi.so）
rem ============================================================
setlocal enabledelayedexpansion
cd /d "%~dp0.."

if not defined ANDROID_NDK_HOME (
    if exist "D:\app\Android\Sdk\ndk\30.0.16138531" (
        set "ANDROID_NDK_HOME=D:\app\Android\Sdk\ndk\30.0.16138531"
    ) else if defined ANDROID_HOME (
        for /d %%d in ("%ANDROID_HOME%\ndk\*") do if not defined ANDROID_NDK_HOME set "ANDROID_NDK_HOME=%%d"
    ) else if defined LOCALAPPDATA (
        for /d %%d in ("%LOCALAPPDATA%\Android\Sdk\ndk\*") do if not defined ANDROID_NDK_HOME set "ANDROID_NDK_HOME=%%d"
    )
)

if not defined ANDROID_NDK_HOME (
    echo [FAIL] 未找到 NDK
    exit /b 1
)

echo [NDK] 使用 NDK: %ANDROID_NDK_HOME%
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

if not defined CARGO_BUILD_JOBS set CARGO_BUILD_JOBS=4
set TARGETS=aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
for %%t in (%TARGETS%) do (
    echo ---- building %%t ----
    cargo ndk -t %%t -o android_app\app\src\main\jniLibs build -p lt-ffi --release
    if errorlevel 1 (
        echo [FAIL] %%t build failed
        exit /b 1
    )
)

echo [OK] android_app\app\src\main\jniLibs\{arm64-v8a,armeabi-v7a,x86_64}\liblt_ffi.so
endlocal
