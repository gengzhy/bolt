# Bolt 编译与构建指南

> 本指南汇总 Bolt 项目的全平台（Windows / macOS / Linux / Android / iOS）编译构建环境准备、代码质量门禁、桌面端与移动端各形态打包流程及常见问题排障。

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

### 苹果平台（macOS / iOS）专用工具链
- **开发设备**：macOS（推荐 macOS 14+，支持 Apple Silicon 与 Intel）
- **Node.js**：≥ 18.0.0（macOS 桌面端 Tauri 前端构建）
- **Xcode**：≥ 15.0（内置 Clang、macOS SDK 与 iOS SDK）
- **Rust Apple 跨编译目标**：
  ```bash
  # macOS 桌面端 Universal 双架构支持
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  # iOS 移动端架构支持
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
  ```

> 💡 **无需本地配置复杂的各平台跨编译环境？**
> Bolt 具备完整的 GitHub Actions 云端自动化构建体系。任何推送到主分支或发布标签的操作均会自动编译出 Windows、macOS、Linux、Android 与 iOS 最新安装包，您也可以随时在 GitHub 网页上一键手动触发构建并直接下载成品。详细使用方法请参阅 📖 [CI/CD 打包指南](github_actions_guide.md)。

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

### 产物结构规范 (`dist/windows/[release|debug]/`)
Windows 端打包脚本将生成四类标准形态，归档于对应模式目录下：
```text
dist/windows/[release|debug]/
├── portable/ -> bolt-windows-0.1.0-x64-portable.exe   # 单文件绿色便携版（免安装，即开即用）
├── cli/      -> bolt-windows-0.1.0-x64-cli.exe        # 命令行联调工具（终端交互与自动化测试）
├── nsis/     -> bolt-windows-0.1.0-x64-setup.exe      # NSIS 安装引导程序（含创建快捷方式与卸载）
└── msi/      -> bolt-windows-0.1.0-x64-zh-CN.msi      # WiX MSI 企业级静默部署包
```

### 一键全量打包（推荐）
Bolt 提供了全自动化打包脚本，自动构建上述四类形态并归档至 `dist/windows/[release|debug]/`：

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
npm run build:portable    # 仅打包绿色便携版 -> bundle/portable/bolt-windows-0.1.0-x64-portable.exe
npm run build:cli         # 仅编译命令行工具 -> bundle/cli/bolt-windows-0.1.0-x64-cli.exe
npm run build:nsis        # 仅打包 NSIS 安装包 -> bundle/nsis/bolt-windows-0.1.0-x64-setup.exe
npm run build:msi         # 仅打包 MSI 安装包 -> bundle/msi/bolt-windows-0.1.0-x64-zh-CN.msi
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

### 产物结构规范 (`dist/android/[release|debug]/`)
Android 端产物归档于对应模式目录下，遵循跨端统一规范 `bolt-android-<版本号>-<架构/变体>.<扩展名>`：
```text
dist/android/
├── release/
│   └── bolt-android-0.1.0-universal.apk           # Release 发布包（R8 深度优化代码与资源，体积约 16 MB）
└── debug/
    └── bolt-android-0.1.0-universal-debug.apk     # Debug 调试包（含调试日志与符号表，体积约 27 MB）
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
# 输出：android_app/app/build/outputs/apk/release/bolt-android-0.1.0-universal.apk

# 步骤 3：（可选）构建 Debug APK
./gradlew.bat assembleDebug
# 输出：android_app/app/build/outputs/apk/debug/bolt-android-0.1.0-universal-debug.apk

# 步骤 4：通过 ADB 安装到真机或模拟器
adb install -r ./app/build/outputs/apk/release/bolt-android-0.1.0-universal.apk
```

---

## 🐧 5. Linux 桌面端与控制台编译打包

### 产物结构规范 (`dist/linux/[release|debug]/`)
Linux 端打包脚本将生成四类标准形态，归档于对应模式目录下：
```text
dist/linux/[release|debug]/
├── appimage/ -> bolt-linux-0.1.0-amd64.AppImage     # 单文件免安装通用绿色版（主流发行版双击即跑）
├── cli/      -> bolt-linux-0.1.0-amd64-cli          # 命令行独立控制台工具（适用于无界面服务器与 NAS）
├── deb/      -> bolt-linux-0.1.0-amd64.deb          # Debian / Ubuntu / Deepin / UOS 安装包
└── rpm/      -> bolt-linux-0.1.0-1.x86_64.rpm       # Fedora / RHEL / openSUSE 安装包
```

### Linux 编译环境准备（Ubuntu / Debian 示例）
在 Linux 开发机或 CI 环境中执行：
```bash
sudo apt-get update && sudo apt-get install -y \
  build-essential \
  pkg-config \
  libglib2.0-dev \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf \
  rpm \
  fonts-wqy-microhei
```

### 一键全量打包（推荐）
Bolt 提供了全自动化打包脚本，自动构建上述四类形态并归档至 `dist/linux/[release|debug]/`：

```bash
# 1. 构建 Release 正式发布包（带符号剥离与尺寸优化）
bash scripts/build_linux_dist.sh -m release

# 2. 构建 Debug 调试包（保留调试符号）
bash scripts/build_linux_dist.sh -m debug
```

### CI/CD 自动化构建
项目已内置 GitHub Actions 工作流（`.github/workflows/build-linux.yml`），每次推送到 main/master 分支或发布标签时，均会在 `ubuntu-latest` 容器上全自动编译出这 4 大形态发布包并供直接下载。

---

## 🍏 6. macOS 桌面端与控制台编译打包

### 产物结构规范 (`dist/macos/[release|debug]/`)
macOS 端打包脚本将生成三大标准形态，归档于对应模式目录下：
```text
dist/macos/[release|debug]/
├── dmg/ -> bolt-macos-0.1.0-universal.dmg               # 免安装拖拽磁盘镜像（标准 DMG，双击直接拖拽至 Applications）
├── app/ -> Bolt.app & bolt-macos-0.1.0-universal.app.zip # 独立应用程序 Bundle 与便于分发的 ZIP 压缩包
└── cli/ -> bolt-macos-0.1.0-universal-cli              # 命令行独立终端工具（原生支持 M 系列与 Intel 芯片）
```

### 一键全量打包（推荐）
Bolt 提供了全自动化打包脚本，自动构建上述三大形态并归档至 `dist/macos/[release|debug]/`：

```bash
# 1. 构建 Release 正式发布包（Universal 双架构，通用支持 Apple Silicon 与 Intel Mac）
bash scripts/build_macos_dist.sh -m release -t universal-apple-darwin

# 2. 快速构建当前本机芯片原生 Release 包
bash scripts/build_macos_dist.sh -m release

# 3. 构建 Debug 调试包（保留调试符号）
bash scripts/build_macos_dist.sh -m debug
```

### CI/CD 自动化构建
项目内置 GitHub Actions 工作流（`.github/workflows/build-macos.yml`），运行于原生 `macos-14`（Apple Silicon）虚拟环境：
- 触发机制：推送代码至 main/master、发布 `v*` Release 标签、或手动触发 `workflow_dispatch`；
- 产出构建：自动跨编译 Universal 双架构二进制并组装 `.dmg`、`.app.zip` 与 `-cli` 文件；
- 无 Mac 设备的开发者可直接在 GitHub Actions Artifacts 页面点击一键下载完整的 macOS 发行包。

---

## 📱 7. iOS 移动端编译与打包

### 架构设计说明
- **核心引擎**：基于 Rust FFI（`crates/ffi`）编译为标准 C ABI 静态库，并通过 `lipo` 与 `xcodebuild -create-xcframework` 封装为通用的 `BoltEngine.xcframework`。
- **上层原生应用**：位于 `ios_app/`，使用 Swift 5.9+ 与 SwiftUI 原生编写，通过系统原生 Bonjour 协议进行局域网 `_bolt._udp.` 服务发现与广播。
- **文件沙盒集成**：开启 `UIFileSharingEnabled`，接收文件直通 iOS 系统自带的「文件」(Files) 应用，支持相册 `PhotosPicker` 高速原图原视频发送。

### 产物结构规范 (`dist/ios/[release|debug]/`)
iOS 端打包脚本将生成标准侧载 IPA 安装包与 Xcode 归档工程，归档于对应模式目录下：
```text
dist/ios/[release|debug]/
├── bolt-ios-0.1.0.ipa      # 免越狱侧载安装包 (通过 Sideloadly / AltStore / TrollStore 直接安装)
├── Bolt.ipa                # 兼容性短文件名别名
└── Bolt.xcarchive          # Xcode 标准归档目录 (可导入 Xcode Organizer 或分发 App Store)
```

### 步骤一：编译底层静态库与 XCFramework
在 macOS 环境的终端中执行：

```bash
# 1. 编译全架构静态库并生成 BoltEngine.xcframework (Debug 模式)
bash scripts/build_ios_lib.sh debug

# 2. 编译 Release 正式版 XCFramework（含尺寸与性能优化）
bash scripts/build_ios_lib.sh release
```
脚本将自动跨编译 `aarch64-apple-ios`（真机）与 `aarch64-apple-ios-sim` / `x86_64-apple-ios`（模拟器），并在 `ios_app/Frameworks/BoltEngine.xcframework` 下生成开箱即用的多架构 XCFramework。

### 步骤二：Xcode 本地调试或归档打包

#### 方式 A：Xcode 图形化调试
1. 双击打开 `ios_app/Bolt.xcodeproj`；
2. 顶部选择真机设备或 iOS 模拟器（如 iPhone 15 Pro）；
3. 点击 **Run (⌘R)** 即可就地编译运行，享受完整的 SwiftUI 界面与局域网传输能力。

#### 方式 B：自动化构建脚本（推荐）
```bash
# 1. 一键生成 Release 正式发布包 (包含 bolt-ios-0.1.0.ipa 与 Bolt.xcarchive)
bash scripts/build_ios_dist.sh -m release

# 2. 一键生成 Debug 调试安装包
bash scripts/build_ios_dist.sh -m debug
```

---

## 🖥 8. 命令行工具（bolt-cli）编译与联调

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

## 🧹 9. 清理工程缓存

当遇到构建缓存不一致、目标平台切换或清理磁盘占用时，可执行全量清理：

```cmd
scripts\clean_all.bat
```
该脚本将安全清理：
- Rust 工作区 `target/` 目录
- Tauri 客户端 `tauri_app/src-tauri/target/` 及前端 `dist/`
- Android `android_app/build/`、`android_app/app/build/` 及 `.gradle/` 缓存
- `dist/` 输出归档目录
