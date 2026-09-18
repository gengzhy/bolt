<#
.SYNOPSIS
    Builds and packages Bolt for Android matching the unified directory structure:
    dist/android/
      ├── release/
      │     └── bolt_<version>_universal.apk
      └── debug/
            └── bolt_<version>_universal-debug.apk

.EXAMPLE
    .\scripts\build_android_dist.ps1                 # Build Release APK
    .\scripts\build_android_dist.ps1 -Mode debug     # Build Debug APK
    .\scripts\build_android_dist.ps1 -Mode all       # Build both Release & Debug APKs
#>

param(
    [ValidateSet("release", "debug", "all")]
    [string]$Mode = "release"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$AndroidAppDir = Join-Path $ProjectRoot "android_app"
$TauriConfPath = Join-Path $ProjectRoot "tauri_app\src-tauri\tauri.conf.json"

# Read version dynamically
$tauriConf = Get-Content $TauriConfPath -Raw -Encoding utf8 | ConvertFrom-Json
$version = $tauriConf.version
if (-not $version) { $version = "0.1.0" }

Write-Host "=================================================" -ForegroundColor Cyan
Write-Host "   Bolt Android Distribution Packaging Pipeline  " -ForegroundColor Cyan
Write-Host "   Version: $version | Mode: $Mode               " -ForegroundColor Cyan
Write-Host "=================================================" -ForegroundColor Cyan

# 1. Ensure JNI libraries exist
$arm64Lib = Join-Path $AndroidAppDir "app\src\main\jniLibs\arm64-v8a\libbt_ffi.so"
if (-not (Test-Path $arm64Lib)) {
    Write-Host "`n[1/3] Building Rust JNI Libraries (cargo-ndk)..." -ForegroundColor Yellow
    Push-Location $ProjectRoot
    try {
        powershell.exe -ExecutionPolicy Bypass -File scripts\build_android_lib.ps1
        if ($LASTEXITCODE -ne 0) { throw "Rust JNI build failed" }
    } finally {
        Pop-Location
    }
} else {
    Write-Host "`n[1/3] Rust JNI Libraries already built." -ForegroundColor Green
}

function Build-Android([string]$buildMode) {
    Write-Host "`n[2/3] Building Android APK ($buildMode)..." -ForegroundColor Yellow
    $gradleTask = if ($buildMode -eq "release") { "assembleRelease" } else { "assembleDebug" }
    
    Push-Location $AndroidAppDir
    try {
        cmd.exe /c ".\gradlew.bat $gradleTask"
        if ($LASTEXITCODE -ne 0) { throw "Gradle $gradleTask failed" }
    } finally {
        Pop-Location
    }
}

if ($Mode -eq "all") {
    Build-Android "release"
    Build-Android "debug"
} else {
    Build-Android $Mode
}

# 3. Summary Report
Write-Host "`n=================================================" -ForegroundColor Cyan
Write-Host "            Android Packaging Summary            " -ForegroundColor Cyan
Write-Host "=================================================" -ForegroundColor Cyan

$distAndroid = Join-Path $ProjectRoot "dist\android"
$allOutputs = Get-ChildItem -Path $distAndroid -Recurse -File -Filter "*.apk"
foreach ($item in $allOutputs) {
    $relPath = $item.FullName.Substring($distAndroid.Length + 1)
    $sizeMB = [math]::Round($item.Length / 1MB, 2)
    $hash = (Get-FileHash -Path $item.FullName -Algorithm SHA256).Hash.Substring(0, 16)
    Write-Host ("  {0,-42} | {1,7} MB | SHA256: {2}..." -f $relPath, $sizeMB, $hash) -ForegroundColor White
}
Write-Host "`nAndroid APK packages ready in: $distAndroid" -ForegroundColor Green
