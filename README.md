# LocalTransfer

<p align="center">
  <img src="docs/LT.png" width="128" alt="LocalTransfer Logo" />
</p>

<p align="center">
  <b>纯局域网、零服务器的跨平台 P2P 超高速文件传输工具</b><br/>
  多端互传 · 全程加密 · 开箱即用
</p>

---

## 📖 项目简介

LocalTransfer 是一款基于 **Rust** 构建的局域网文件传输工具，支持 Windows 与 Android 双平台互传。
无需任何服务器、账号或互联网连接，设备处于同一局域网即可快速安全地传输文件。

## ✨ 核心优势

- **极速传输**：QUIC 协议深度调优，局域网实测稳定 **22+ MB/s**，支持多文件并发传输
- **点对点直连**：QUIC 优先，自动降级 TCP，无中转服务器，数据不经第三方
- **全程加密**：TLS 1.3 强制加密（rustls + ring），Ed25519 自签证书，传输内容零泄漏
- **零配置发现**：mDNS + UDP 广播双通道自动发现，开箱即用无需手动输入 IP
- **安全配对**：BLAKE3 指纹 + 6 位验证码人工比对 + TOFU 信任库，三重身份校验
- **完整性校验**：BLAKE3 全文件哈希校验，确保传输内容字节级一致
- **隐私至上**：无服务器、无账户、无外网请求、无遥测、日志不含文件内容与密钥

## 🛠 技术栈

| 层面 | 技术 |
|------|------|
| **核心引擎** | Rust（跨平台编译为 `.dll` / `.so`） |
| **传输协议** | QUIC（Quinn 0.11）+ TCP + TLS 1.3（rustls） |
| **加密体系** | Ed25519 签名 · BLAKE3 哈希 · ring 密码库 |
| **设备发现** | mDNS（mdns-sd）+ UDP 广播探测 |
| **Windows 端** | Tauri v2 + Vue 3 + TypeScript |
| **Android 端** | Kotlin + Jetpack Compose + Gradle |
| **异步运行时** | Tokio |
| **FFI 桥接** | C ABI（cbindgen 生成头文件） |

## 🏗 项目架构

```
crates/
  lt-utils        公共设施：错误码、配置、UUID、网络、全局常量
  lt-crypto       设备身份、证书、指纹、配对码、信任库、TLS 配置
  lt-file         文件遍历、读取、写入（BLAKE3 校验）、磁盘预检
  lt-discovery    设备模型、mDNS、UDP 探测、NSD 桥、发现管理器
  lt-transfer     传输引擎：QUIC+TCP 双栈、会话状态机、收发流水线
  lt-task         应用门面：事件驱动、任务管理、配置、编排调度
  lt-ffi          C ABI 导出（Windows lt_ffi.dll / Android liblt_ffi.so）

tools/lt-cli      命令行联调工具（serve / send / discover）
tauri_app/        Windows 桌面客户端（Tauri v2 + Vue 3）
android_app/      Android 客户端（Kotlin + Jetpack Compose）
docs/             设计文档与规范
scripts/          构建与测试脚本
```

### 数据流

```
┌─────────────┐         QUIC / TCP + TLS 1.3         ┌─────────────┐
│  发送端 App  │ ◄──────────────────────────────────► │  接收端 App  │
│  (lt-task)   │    mDNS / UDP 发现  ←→  设备列表     │  (lt-task)   │
└──────┬───────┘                                      └──────┬───────┘
       │ FFI (C ABI)                                         │ FFI
┌──────┴───────┐                                      ┌──────┴───────┐
│ Tauri / 前端  │                                      │ Kotlin / UI  │
│ (Windows)    │                                      │ (Android)    │
└──────────────┘                                      └──────────────┘
```

## 🚀 快速开始

### 环境要求

- **Rust** ≥ 1.85（推荐 1.98+），MSVC 工具链（Windows）
- **Node.js** ≥ 18（Tauri 前端构建）
- **Android NDK**（Android 端编译）
- **cargo-ndk**（`cargo install cargo-ndk`）

### 编译 Windows 桌面端

- **直接打包独立可执行文件（推荐，免 Node 环境）**：
  静态资产已内嵌于 `tauri_app/dist`，直接通过 Rust 工具链构建：
  ```bash
  cd tauri_app/src-tauri
  cargo build --release
  # 产物输出于：tauri_app/src-tauri/target/release/lt-tauri.exe
  ```

- **编译 Windows FFI 动态库与 CLI 工具**：
  ```cmd
  scripts\build_rust_lib.bat
  # 产物输出于：lib/win64/lt_ffi.dll 及 target/release/lt-cli.exe
  ```

- **完整前端热重载开发 / NSIS 打包（需 Node.js）**：
  ```bash
  cd tauri_app && npm install && npm run tauri dev
  cd tauri_app && npm run tauri build
  ```

### 编译 Android 端

```bash
# 1. 添加 Android 编译目标（首次需要）
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

# 2. 编译 Rust 动态库（.so）（自动支持三大架构 aarch64, armv7, x86_64）
scripts\build_android_lib.bat    # 或 powershell.exe -ExecutionPolicy Bypass -File scripts\build_android_lib.ps1

# 3. 构建 APK
cd android_app
./gradlew.bat :app:assembleDebug

# 4. 安装到设备
adb install -r ./app/build/outputs/apk/debug/app-debug.apk
```

### 命令行联调（lt-cli）

```bash
cargo build -p lt-cli

# 接收端
./target/debug/lt-cli.exe serve --port 8899 --data-dir target/cli_a

# 发送端
./target/debug/lt-cli.exe send --data-dir target/cli_b 127.0.0.1:8899 ./file.zip

# 浏览局域网设备
./target/debug/lt-cli.exe discover
```

## 📱 使用说明

1. **确保设备在同一局域网**（Wi-Fi 或有线均可）
2. **启动双端应用**：Windows 端运行桌面客户端，Android 端打开 App
3. **自动发现**：设备列表自动显示对方设备
4. **首次配对**：点击设备名连接，双方屏幕显示 6 位验证码，确认一致后信任
5. **选择文件发送**：选择文件或文件夹，点击发送，对方确认接收即开始传输
6. **传输完成**：自动 BLAKE3 校验，确保文件完整无损

## 🔒 安全机制

| 层面 | 实现 |
|------|------|
| **传输加密** | TLS 1.3 强制，无明文模式 |
| **设备身份** | Ed25519 自签证书（10 年有效期） |
| **身份验证** | BLAKE3 指纹 + 6 位验证码 + TOFU 信任库 |
| **文件校验** | BLAKE3 全文件哈希，字节级一致性保证 |
| **隐私保护** | 零外网请求、零遥测、日志不含文件内容与密钥 |

## 📋 质量门禁

```bash
cargo fmt --all -- --check           # 代码格式检查
cargo clippy --workspace --all-targets -- -D warnings   # 静态分析
cargo test --workspace               # 全量单元测试
bash scripts/smoke_loopback.sh       # 环回冒烟测试（QUIC + TCP + 校验）
```

## 📖 文档

| 文档 | 说明 |
|------|------|
| [传输协议规范](docs/protocol_spec.md) | 帧格式、指令码、握手流程、错误码 |
| [FFI API 参考](docs/ffi_api.md) | `lt_*` C ABI 接口与事件定义 |
| [开发指南](docs/dev_guide.md) | 架构速览、环境配置、构建步骤、测试约定 |
| [QUIC 性能优化](docs/QUIC性能优化执行计划.md) | 传输层调优方案与实测验证 |
| [可行性实施方案](docs/LocalTransfer-可行性实施方案.txt) | 原始设计方案 |

## 📜 许可

本项目仅供学习与个人使用。
