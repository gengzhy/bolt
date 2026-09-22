<#
.SYNOPSIS
    Builds and packages Bolt for Windows into the unified directory structure:
    dist/windows/[debug|release]/
      ├── portable/   -> bolt-windows-<version>-x64-portable.exe (Standalone green exe)
      ├── cli/        -> bolt-windows-<version>-x64-cli.exe      (Command line debugging tool)
      ├── nsis/       -> bolt-windows-<version>-x64-setup.exe    (NSIS Setup wizard with LZMA)
      └── msi/        -> bolt-windows-<version>-x64-zh-CN.msi    (WiX MSI enterprise package)

.EXAMPLE
    .\scripts\build_windows_dist.ps1                        # Build all 4 packages (Release)
    .\scripts\build_windows_dist.ps1 -Mode debug            # Build all 4 packages (Debug)
    .\scripts\build_windows_dist.ps1 -Mode all              # Build both Release and Debug
    .\scripts\build_windows_dist.ps1 -Target portable       # Build only portable executable
    .\scripts\build_windows_dist.ps1 -CollectOnly           # Organize already-built outputs without rebuild
#>

param(
    [ValidateSet("release", "debug", "all")]
    [string]$Mode = "release",

    [ValidateSet("all", "portable", "cli", "nsis", "msi")]
    [string]$Target = "all",

    [switch]$CollectOnly
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$TauriAppDir = Join-Path $ProjectRoot "tauri_app"
$TauriConfPath = Join-Path $TauriAppDir "src-tauri\tauri.conf.json"

# Read version dynamically from tauri.conf.json
$tauriConf = Get-Content $TauriConfPath -Raw -Encoding utf8 | ConvertFrom-Json
$version = $tauriConf.version
if (-not $version) { $version = "0.1.0" }

Write-Host "=================================================" -ForegroundColor Cyan
Write-Host "  Bolt Windows Distribution Packaging Pipeline  " -ForegroundColor Cyan
Write-Host "  Version: $version | Mode: $Mode | Target: $Target" -ForegroundColor Cyan
Write-Host "=================================================" -ForegroundColor Cyan

# Terminate running bolt / bolt-cli processes to avoid Windows file locks
Get-Process bolt -ErrorAction SilentlyContinue | Stop-Process -Force
Get-Process bolt-cli -ErrorAction SilentlyContinue | Stop-Process -Force

function Build-And-Package([string]$buildMode) {
    Write-Host "`n>>> Processing [$buildMode] Mode Pipeline..." -ForegroundColor Magenta
    
    $isDebug = ($buildMode -eq "debug")
    $targetTauriDir = Join-Path $TauriAppDir "src-tauri\target\$buildMode"
    $bundleDir = Join-Path $targetTauriDir "bundle"
    $distDir = Join-Path $ProjectRoot "dist\windows\$buildMode"

    # Ensure bundle directories exist
    $portableDir = Join-Path $bundleDir "portable"
    $cliDir = Join-Path $bundleDir "cli"
    $nsisDir = Join-Path $bundleDir "nsis"
    $msiDir = Join-Path $bundleDir "msi"
    
    @($portableDir, $cliDir, $nsisDir, $msiDir, $distDir) | ForEach-Object {
        if (-not (Test-Path $_)) { New-Item -ItemType Directory -Path $_ -Force | Out-Null }
    }

    if (-not $CollectOnly) {
        # 1. Build Frontend
        Write-Host "  [1/4] Building Frontend (Vue 3 + Vite)..." -ForegroundColor Yellow
        Push-Location $TauriAppDir
        try {
            if (-not (Test-Path "node_modules")) {
                Write-Host "  Installing frontend dependencies (npm install)..." -ForegroundColor Yellow
                npm install
            }
            npm run build
            if ($LASTEXITCODE -ne 0) { throw "Frontend build failed" }
        } finally {
            Pop-Location
        }

        # 2. Build bolt-cli
        if ($Target -eq "all" -or $Target -eq "cli") {
            Write-Host "  [2/4] Compiling bolt-cli ($buildMode)..." -ForegroundColor Yellow
            Push-Location $ProjectRoot
            try {
                if ($isDebug) {
                    cargo build -p bolt-cli
                } else {
                    cargo build -p bolt-cli --release
                }
                if ($LASTEXITCODE -ne 0) { throw "bolt-cli build failed" }
            } finally {
                Pop-Location
            }
        }

        # 3. Build Tauri App & Bundles
        Write-Host "  [3/4] Building Tauri App & Bundles ($buildMode)..." -ForegroundColor Yellow
        Push-Location $TauriAppDir
        try {
            $tauriArgs = @("tauri", "build")
            if ($isDebug) { $tauriArgs += "--debug" }

            if ($Target -eq "portable") {
                $tauriArgs += "--no-bundle"
            } elseif ($Target -eq "nsis") {
                $tauriArgs += @("--bundles", "nsis")
            } elseif ($Target -eq "msi") {
                $tauriArgs += @("--bundles", "msi")
            } else {
                $tauriArgs += @("--bundles", "nsis,msi")
            }

            npx @tauriArgs
            if ($LASTEXITCODE -ne 0) { throw "Tauri build failed" }
        } finally {
            Pop-Location
        }
    }

    # 4. Populate and organize bundle/ subdirectories
    Write-Host "  [4/4] Organizing bundle folders..." -ForegroundColor Yellow

    # (a) Portable Executable -> bundle/portable/bolt-windows-<version>-x64-portable.exe
    $rawBolt = Join-Path $targetTauriDir "bolt.exe"
    if (Test-Path $rawBolt) {
        $portableTarget = Join-Path $portableDir "bolt-windows-$version-x64-portable.exe"
        Copy-Item -Path $rawBolt -Destination $portableTarget -Force
        # 兼容旧命名别名
        Copy-Item -Path $rawBolt -Destination (Join-Path $portableDir "bolt_${version}_x64-portable.exe") -Force
        Write-Host "    -> [Portable] $portableTarget" -ForegroundColor Green
    }

    # (b) CLI Tool -> bundle/cli/bolt-windows-<version>-x64-cli.exe
    $rawCli = Join-Path $ProjectRoot "target\$buildMode\bolt-cli.exe"
    if (Test-Path $rawCli) {
        $cliTarget = Join-Path $cliDir "bolt-windows-$version-x64-cli.exe"
        Copy-Item -Path $rawCli -Destination $cliTarget -Force
        # 兼容旧命名别名
        Copy-Item -Path $rawCli -Destination (Join-Path $cliDir "bolt_${version}_x64-cli.exe") -Force
        Write-Host "    -> [CLI]      $cliTarget" -ForegroundColor Green
    }

    # (c) Standardize NSIS installer name -> bolt-windows-<version>-x64-setup.exe
    $rawNsis = Get-ChildItem -Path $nsisDir -Filter "*.exe" | Where-Object { $_.Name -notlike "bolt-windows-*" } | Select-Object -First 1
    if ($rawNsis) {
        $nsisTarget = Join-Path $nsisDir "bolt-windows-$version-x64-setup.exe"
        Copy-Item -Path $rawNsis.FullName -Destination $nsisTarget -Force
        Write-Host "    -> [NSIS]     $nsisTarget" -ForegroundColor Green
    }

    # (d) Standardize MSI installer name -> bolt-windows-<version>-x64-zh-CN.msi
    $rawMsi = Get-ChildItem -Path $msiDir -Filter "*.msi" | Where-Object { $_.Name -notlike "bolt-windows-*" } | Select-Object -First 1
    if ($rawMsi) {
        $msiTarget = Join-Path $msiDir "bolt-windows-$version-x64-zh-CN.msi"
        Copy-Item -Path $rawMsi.FullName -Destination $msiTarget -Force
        Write-Host "    -> [MSI]      $msiTarget" -ForegroundColor Green
    }

    # (e) Sync all bundles to dist/windows/[buildMode]/
    Copy-Item -Path "$bundleDir\*" -Destination $distDir -Recurse -Force

    # Print Summary for this mode
    Write-Host "`n  =================================================" -ForegroundColor Cyan
    Write-Host "       Windows [$buildMode] Bundle Summary       " -ForegroundColor Cyan
    Write-Host "  =================================================" -ForegroundColor Cyan

    $allOutputs = Get-ChildItem -Path $bundleDir -Recurse -File
    foreach ($item in $allOutputs) {
        $relPath = $item.FullName.Substring($bundleDir.Length + 1)
        $sizeMB = [math]::Round($item.Length / 1MB, 2)
        $hash = (Get-FileHash -Path $item.FullName -Algorithm SHA256).Hash.Substring(0, 16)
        Write-Host ("  {0,-42} | {1,7} MB | SHA256: {2}..." -f $relPath, $sizeMB, $hash) -ForegroundColor White
    }
}

if ($Mode -eq "all") {
    Build-And-Package "release"
    Build-And-Package "debug"
} else {
    Build-And-Package $Mode
}

Write-Host "`nAll Windows packages compiled and organized successfully!" -ForegroundColor Green
