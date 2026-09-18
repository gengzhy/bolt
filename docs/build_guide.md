# Bolt 编译与构建指南

> 本指南汇总 Bolt 项目的全平台编译构建环境准备、代码质量门禁、Windows 桌面端与 Android 移动端的全量/单项打包流程及常见问题排障。

---

## 🛠 1. 环境准备

### 基础开发环境
- **Rust**：≥ 1.85（建议使用最新稳定版 1.98+）
  - 安装 MSVC 工具链（Windows）：`rustup default stable-x86_64-pc-windows-msvc`
  - 添加 Android 跨编译 Target：
    ```bash
    rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
    ```
- **Node.js**：≥ 18.0.0（推荐使用 LTS 版本，用于构建 Tauri 前端）
- **Git**：建议在 Windows 下配置支持 Git Bash（用于执行冒烟测试脚本 `smoke_loopback.sh`）

### 移动端（Android）专用工具链
- **Android SDK & NDK**：
  - 建议通过 Android Studio SDK Manager 安装 "NDK (Side by side)"（推荐 25.x ~ 27.x）以及 CMake。
  - 配置环境变量 `ANDROID_HOME`（指向 Android SDK 根目录）或 `ANDROID_NDK_HOME`（指向具体 NDK 版本目录）。
- **cargo-ndk**：
  ```bash
  cargo install cargo-ndk
  ```

---

## 📋 2. 代码质量门禁与验证

在提交代码或合并分支前，必须通过以下四道自动化质量门禁：

```bash
# 1. 代码格式化校验
cargo fmt --all -- --check

# 2. 门禁级静态分析（零警告容忍）
cargo clippy --workspace --all-targets -- -D warnings

# 3. 工作区全量单元测试与集成测试
cargo test --workspace

# 4. 本地双节点回环冒烟测试（验证 QUIC + TCP + TOFU 证书 + BLAKE3 完整性校验）
bash scripts/smoke_loopback.sh
```

### 质量红线
1. **纯局域网与零隐私外泄**：代码中严禁引入任何外网上报、云端探针或未告知的遥测 SDK。
2. **测试自隔离**：集成测试必须在独立带 PID 的临时目录下运行，不得相互干扰或依赖外网端口。
3. **未完成文件防污染**：传输中文件必须使用 `.bttmp` 后缀就地落盘，哈希通过后方可重命名。

---

## 💻 3. Windows 桌面端编译与打包

### 产物结构规范 (`dist/windows/bundle/`)
Windows 端打包脚本将生成四类标准形态：
```text
dist/windows/bundle/
├── portable/ -> bolt_0.1.0_x64-portable.exe   # 单文件绿色便携版（免安装，即开即用）
├── cli/      -> bolt_0.1.0_x64-cli.exe        # 命令行联调工具（终端交互与自动化测试）
├── nsis/     -> bolt_0.1.0_x64-setup.exe      # NSIS 安装引导程序（含创建快捷方式与卸载）
└── msi/      -> bolt_0.1.0_x64_zh-CN.msi      # WiX MSI 企业级静默部署包
```

### 一键全量打包（推荐）
Bolt 提供了全自动化打包脚本，自动构建上述四类形态并归档至 `dist/windows/bundle/`：

```powershell
# 1. 构建 Release 正式发布包（带 strip/LTO/z 压缩优化）
powershell.exe -ExecutionPolicy Bypass -File scripts\build_windows_dist.ps1 -Mode release
# 或在 tauri_app 目录下执行：
cd tauri_app && npm run build:all

# 2. 构建 Debug 调试包（保留符号信息，快速迭代）
powershell.exe -ExecutionPolicy Bypass -File scripts\build_windows_dist.ps1 -Mode debug
# 或在 tauri_app 目录下执行：
cd tauri_app && npm run build:debug
```

### 单独打包指定产物
如果仅需产出特定形态，可进入 `tauri_app` 目录执行指定命令：
```bash
cd tauri_app
npm run build:portable    # 仅打包绿色便携版 -> bundle/portable/bolt_0.1.0_x64-portable.exe
npm run build:cli         # 仅编译命令行工具 -> bundle/cli/bolt_0.1.0_x64-cli.exe
npm run build:nsis        # 仅打包 NSIS 安装包 -> bundle/nsis/bolt_0.1.0_x64-setup.exe
npm run build:msi         # 仅打包 MSI 安装包 -> bundle/msi/bolt_0.1.0_x64_zh-CN.msi
```

### 本地开发与实时调试
```bash
cd tauri_app
npm install              # 安装前端依赖
npm run tauri dev        # 启动热重载开发服务器与桌面调试窗口
```

### 独立编译 FFI 动态库
```cmd
scripts\build_rust_lib.bat
# 产物输出于：lib/win64/bt_ffi.dll 及 target/release/bolt-cli.exe
```

---

## 📱 4. Android 移动端编译与打包

### 产物结构规范 (`dist/android/`)
Android 端产物命名与 Windows 保持统一规范：`<应用名>_<版本号>_<架构>-<变体>.<扩展名>`：
```text
dist/android/
├── bolt_0.1.0_universal-release.apk   # Release 发布包（R8 深度优化代码与资源，体积约 15.5 MB）
└── debug/
    └── bolt_0.1.0_universal-debug.apk # Debug 调试包（含调试日志与符号表，体积约 25 MB）
```

### 一键全量打包（推荐）
一键脚本会自动检测 NDK、交叉编译 3 大架构（`arm64-v8a`、`armeabi-v7a`、`x86_64`）的 `libbt_ffi.so`、同步至 `jniLibs`，并调用 Gradle 执行打包与重命名归档：

```powershell
# 1. 一键构建 Release 正式版 APK：
powershell.exe -ExecutionPolicy Bypass -File scripts\build_android_dist.ps1 -Mode release

# 2. 一键构建 Debug 调试版 APK：
powershell.exe -ExecutionPolicy Bypass -File scripts\build_android_dist.ps1 -Mode debug
```

### 手动分步构建流程
```bash
# 步骤 1：跨平台编译 Rust 动态库（自动覆盖 aarch64, armv7, x86_64）
powershell.exe -ExecutionPolicy Bypass -File scripts\build_android_lib.ps1
# 或执行批处理：scripts\build_android_lib.bat

# 步骤 2：构建 Release APK
cd android_app
./gradlew.bat assembleRelease
# 输出：android_app/app/build/outputs/apk/release/bolt_0.1.0_universal-release.apk

# 步骤 3：（可选）构建 Debug APK
./gradlew.bat assembleDebug
# 输出：android_app/app/build/outputs/apk/debug/bolt_0.1.0_universal-debug.apk

# 步骤 4：通过 ADB 安装到真机或模拟器
adb install -r ./app/build/outputs/apk/release/bolt_0.1.0_universal-release.apk
```

---

## 🖥 5. 命令行工具（bolt-cli）编译与联调

`bolt-cli` 是独立于界面的控制台联调工具，适用于服务器、无图形桌面或双机快速验证：

```bash
# 编译 CLI
cargo build -p bolt-cli

# 启动接收端服务（监听指定端口并设置接收目录）
./target/debug/bolt-cli.exe serve --port 8899 --data-dir target/cli_a

# 浏览局域网中广播的 Bolt 在线设备
./target/debug/bolt-cli.exe discover

# 向目标设备发送文件或文件夹
./target/debug/bolt-cli.exe send --data-dir target/cli_b 127.0.0.1:8899 ./path/to/file
```

---

## 🧹 6. 清理工程缓存

当遇到构建缓存不一致、目标平台切换或清理磁盘占用时，可执行全量清理：

```cmd
scripts\clean_all.bat
```
该脚本将安全清理：
- Rust 工作区 `target/` 目录
- Tauri 客户端 `tauri_app/src-tauri/target/` 及前端 `dist/`
- Android `android_app/build/`、`android_app/app/build/` 及 `.gradle/` 缓存
- `dist/` 输出归档目录
