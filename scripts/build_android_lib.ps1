if (-not $env:ANDROID_NDK_HOME) {
    if (Test-Path "D:\app\Android\Sdk\ndk\30.0.16138531") {
        $env:ANDROID_NDK_HOME = "D:\app\Android\Sdk\ndk\30.0.16138531"
    } elseif ($env:ANDROID_HOME -and (Test-Path "$env:ANDROID_HOME\ndk")) {
        $found = Get-ChildItem "$env:ANDROID_HOME\ndk" | Select-Object -First 1
        if ($found) { $env:ANDROID_NDK_HOME = $found.FullName }
    } elseif ($env:LOCALAPPDATA -and (Test-Path "$env:LOCALAPPDATA\Android\Sdk\ndk")) {
        $found = Get-ChildItem "$env:LOCALAPPDATA\Android\Sdk\ndk" | Select-Object -First 1
        if ($found) { $env:ANDROID_NDK_HOME = $found.FullName }
    }
}
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
$env:PATH = "$cargoBin;$env:PATH"

$targets = @("aarch64-linux-android", "armv7-linux-androideabi", "x86_64-linux-android")
foreach ($t in $targets) {
    Write-Host "---- building $t ----"
    cargo ndk -t $t -o android_app\app\src\main\jniLibs build -p ffi --release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "$t build failed"
        exit 1
    }
}
Write-Host "[OK] android_app\app\src\main\jniLibs built successfully"
