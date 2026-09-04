$env:ANDROID_NDK_HOME = "D:\app\Android\Sdk\ndk\30.0.16138531"
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
$env:PATH = "$cargoBin;$env:PATH"

$targets = @("aarch64-linux-android", "armv7-linux-androideabi", "x86_64-linux-android")
foreach ($t in $targets) {
    Write-Host "---- building $t ----"
    cargo ndk -t $t -o android_app\app\src\main\jniLibs build -p lt-ffi --release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "$t build failed"
        exit 1
    }
}
Write-Host "[OK] android_app\app\src\main\jniLibs built successfully"
