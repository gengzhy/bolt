<#
.SYNOPSIS
    Builds and packages Bolt for Windows into the unified flat directory structure:
    dist/[debug|release]/
      ├── bolt-v<version>-windows-amd64-portable.zip (Standalone green zip containing bolt.exe)
      ├── bolt-cli-v<version>-windows-amd64.zip      (Command line debugging tool zip)
      ├── bolt-v<version>-windows-amd64-setup.exe    (NSIS Setup wizard with LZMA)
      └── bolt-v<version>-windows-amd64.msi          (WiX MSI enterprise package)

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

    [ValidateSet("all", "portable", "cli", "nsis", "msi", "msix")]
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
$rawVersion = $tauriConf.version
if (-not $rawVersion) { $rawVersion = "0.1.0" }
$version = $rawVersion.TrimStart('v')

Write-Host "=================================================" -ForegroundColor Cyan
Write-Host "  Bolt Windows Distribution Packaging Pipeline  " -ForegroundColor Cyan
Write-Host "  Version: v$version | Mode: $Mode | Target: $Target" -ForegroundColor Cyan
Write-Host "=================================================" -ForegroundColor Cyan

# Terminate running bolt / bolt-cli processes to avoid Windows file locks
Get-Process bolt -ErrorAction SilentlyContinue | Stop-Process -Force
Get-Process bolt-cli -ErrorAction SilentlyContinue | Stop-Process -Force

function Build-And-Package([string]$buildMode) {
    Write-Host "`n>>> Processing [$buildMode] Mode Pipeline..." -ForegroundColor Magenta
    
    $isDebug = ($buildMode -eq "debug")
    $targetTauriDir = Join-Path $TauriAppDir "src-tauri\target\$buildMode"
    $bundleDir = Join-Path $targetTauriDir "bundle"
    $distDir = Join-Path $ProjectRoot "dist\$buildMode"

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
        if ($Target -ne "cli") {
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
        if ($Target -ne "cli" -and $Target -ne "msix") {
            Write-Host "  [3/5] Building Tauri App & Bundles ($buildMode)..." -ForegroundColor Yellow
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

        # 4. Build Microsoft Store MSIX Package
        if ($Target -eq "all" -or $Target -eq "msix") {
            Write-Host "  [4/5] Building Microsoft Store MSIX Bundle ($buildMode)..." -ForegroundColor Yellow
            Push-Location $TauriAppDir
            try {
                $msixArgs = @("tauri:windows:build", "--", "--runner", "npm")
                if ($isDebug) { $msixArgs += "--debug" }
                npm run @msixArgs
                if ($LASTEXITCODE -ne 0) { throw "MSIX build failed" }
            } finally {
                Pop-Location
            }
        }
    }

    # 5. Populate and organize bundle/ subdirectories
    Write-Host "  [5/5] Organizing bundle folders..." -ForegroundColor Yellow

    # (a) Portable GUI ZIP -> bundle/portable/bolt-v<version>-windows-amd64-portable.zip (解压即为纯净 bolt.exe)
    $rawBolt = Join-Path $targetTauriDir "bolt.exe"
    if (Test-Path $rawBolt) {
        $portableTargetZip = Join-Path $portableDir "bolt-v$version-windows-amd64-portable.zip"
        if (Test-Path $portableTargetZip) { Remove-Item -Path $portableTargetZip -Force }
        Compress-Archive -Path $rawBolt -DestinationPath $portableTargetZip -Force
        # 清理目录中非规范可执行文件，确保仅保留规范 zip 产物
        Get-ChildItem -Path $portableDir -Filter "*.exe" | Remove-Item -Force
        Write-Host "    -> [Portable] $portableTargetZip" -ForegroundColor Green
    }

    # (b) CLI Tool ZIP -> bundle/cli/bolt-cli-v<version>-windows-amd64.zip (解压即为纯净 bolt-cli.exe)
    $rawCli = Join-Path $ProjectRoot "target\$buildMode\bolt-cli.exe"
    if (Test-Path $rawCli) {
        $cliTargetZip = Join-Path $cliDir "bolt-cli-v$version-windows-amd64.zip"
        if (Test-Path $cliTargetZip) { Remove-Item -Path $cliTargetZip -Force }
        Compress-Archive -Path $rawCli -DestinationPath $cliTargetZip -Force
        # 清理目录中非规范可执行文件，确保仅保留规范 zip 产物
        Get-ChildItem -Path $cliDir -Filter "*.exe" | Remove-Item -Force
        Write-Host "    -> [CLI]      $cliTargetZip" -ForegroundColor Green
    }

    # (c) Standardize NSIS installer name -> bolt-v<version>-windows-amd64-setup.exe
    $rawNsis = Get-ChildItem -Path $nsisDir -Filter "*.exe" | Where-Object { $_.Name -notlike "bolt-v*" } | Select-Object -First 1
    if ($rawNsis) {
        $nsisTarget = Join-Path $nsisDir "bolt-v$version-windows-amd64-setup.exe"
        Copy-Item -Path $rawNsis.FullName -Destination $nsisTarget -Force
        Get-ChildItem -Path $nsisDir -Filter "*.exe" | Where-Object { $_.Name -ne "bolt-v$version-windows-amd64-setup.exe" } | Remove-Item -Force
        Write-Host "    -> [NSIS]     $nsisTarget" -ForegroundColor Green
    }

    # (d) Standardize MSI installer name -> bolt-v<version>-windows-amd64.msi
    $rawMsi = Get-ChildItem -Path $msiDir -Filter "*.msi" | Where-Object { $_.Name -notlike "bolt-v*" } | Select-Object -First 1
    if ($rawMsi) {
        $msiTarget = Join-Path $msiDir "bolt-v$version-windows-amd64.msi"
        Copy-Item -Path $rawMsi.FullName -Destination $msiTarget -Force
        Get-ChildItem -Path $msiDir -Filter "*.msi" | Where-Object { $_.Name -ne "bolt-v$version-windows-amd64.msi" } | Remove-Item -Force
        Write-Host "    -> [MSI]      $msiTarget" -ForegroundColor Green
    }

    # (e) Sync MSIX packages -> dist/[buildMode]/
    $rawMsixDir = Join-Path $TauriAppDir "src-tauri\target\msix"
    if (Test-Path $rawMsixDir) {
        $msixBundle = Get-ChildItem -Path $rawMsixDir -Filter "*.msixbundle" | Select-Object -First 1
        if ($msixBundle) {
            Copy-Item -Path $msixBundle.FullName -Destination $distDir -Force
            Write-Host "    -> [MSIXBUNDLE] $($msixBundle.Name)" -ForegroundColor Green
        }
        $rawMsix = Get-ChildItem -Path $rawMsixDir -Filter "*_x64.msix" | Select-Object -First 1
        if ($rawMsix) {
            $specMsix = Join-Path $distDir "bolt-v$version-windows-amd64.msix"
            Copy-Item -Path $rawMsix.FullName -Destination $specMsix -Force
            Write-Host "    -> [MSIX]       bolt-v$version-windows-amd64.msix" -ForegroundColor Green
        }
    }

    # (f) Sync all bundles directly to dist/[buildMode]/ (flat structure, no subfolders)
    if (-not (Test-Path $distDir)) { New-Item -ItemType Directory -Path $distDir -Force | Out-Null }
    foreach ($sub in @($portableDir, $cliDir, $nsisDir, $msiDir)) {
        if (Test-Path $sub) {
            Copy-Item -Path "$sub\*" -Destination $distDir -Force
        }
    }

    # Print Summary for this mode
    Write-Host "`n  =================================================" -ForegroundColor Cyan
    Write-Host "       Windows [$buildMode] Bundle Summary       " -ForegroundColor Cyan
    Write-Host "  =================================================" -ForegroundColor Cyan

    $targetNames = @(
        "bolt-v$version-windows-amd64-portable.zip",
        "bolt-cli-v$version-windows-amd64.zip",
        "bolt-v$version-windows-amd64-setup.exe",
        "bolt-v$version-windows-amd64.msi",
        "bolt-v$version-windows-amd64.msix",
        "Bolt 闪传_0.1.0.0.msixbundle"
    )
    foreach ($name in $targetNames) {
        $tf = Join-Path $distDir $name
        if (Test-Path $tf) {
            $item = Get-Item $tf
            $sizeMB = [math]::Round($item.Length / 1MB, 2)
            $hash = (Get-FileHash -Path $item.FullName -Algorithm SHA256).Hash.Substring(0, 16)
            Write-Host ("  {0,-42} | {1,7} MB | SHA256: {2}..." -f $item.Name, $sizeMB, $hash) -ForegroundColor White
        }
    }
}

if ($Mode -eq "all") {
    Build-And-Package "release"
    Build-And-Package "debug"
} else {
    Build-And-Package $Mode
}

Write-Host "`nAll Windows packages compiled and organized successfully!" -ForegroundColor Green
