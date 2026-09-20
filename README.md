# Bolt

<p align="center">
  <img src="docs/bolt.png" width="128" alt="Bolt Logo" />
</p>

<p align="center">
  <b>纯局域网、零服务器的跨平台 P2P 超高速文件传输工具</b><br/>
  多端互传 · 全程加密 · 开箱即用
</p>

<p align="center">
  <a href="LICENSE"><img src="docs/badge_license.svg" alt="License: MIT" /></a>
  <img src="docs/badge_rust.svg" alt="Rust" />
  <img src="docs/badge_platform.svg" alt="Platform" />
</p>

---

## 📖 项目简介

Bolt 是一款基于 **Rust** 构建的局域网文件传输工具，支持 Windows、Linux、Android 与 iOS 全平台互传。
无需任何服务器、账号或互联网连接，设备处于同一局域网即可快速安全地传输文件。

## ✨ 核心优势

- **极速传输**：QUIC 协议深度调优，局域网实测稳定 **22+ MB/s**，支持多文件并发传输
- **点对点直连**：QUIC 优先，自动降级 TCP，无中转服务器，数据不经第三方
- **全程加密**：TLS 1.3 强制加密（rustls + ring），Ed25519 自签证书，传输内容零泄漏
- **零配置发现**：mDNS + Bonjour + UDP 广播多通道自动发现，开箱即用无需手动输入 IP
- **安全配对**：BLAKE3 指纹 + 4 位验证码人工比对 + TOFU 信任库，三重身份校验
- **完整性校验**：BLAKE3 全文件哈希校验，确保传输内容字节级一致
- **隐私至上**：无服务器、无账户、无外网请求、无遥测、日志不含文件内容与密钥

## 🛠 技术栈

| 层面 | 技术 |
|------|------|
| **核心引擎** | Rust（跨平台编译为 `.dll` / `.so` / `.xcframework`） |
| **传输协议** | QUIC（Quinn 0.11）+ TCP + TLS 1.3（rustls） |
| **加密体系** | Ed25519 签名 · BLAKE3 哈希 · ring 密码库 |
| **设备发现** | mDNS（mdns-sd）+ Apple Bonjour（NetService）+ UDP 广播探测 |
| **桌面客户端** | Tauri v2 + Vue 3 + TypeScript（支持 Windows / Linux） |
| **Android 端** | Kotlin + Jetpack Compose + Gradle |
| **iOS 端** | Swift 5.9+ + SwiftUI + Xcode |
| **CLI 命令行端** | Rust 原生二进制（跨平台支持） |
| **异步运行时** | Tokio |
| **FFI 桥接** | C ABI（cbindgen 生成头文件） |

## 🏗 项目架构

```
crates/
  utils           公共设施：错误码、配置、UUID、网络、全局常量
  crypto          设备身份、证书、指纹、配对码、信任库、TLS 配置
  file            文件遍历、读取、写入（BLAKE3 校验）、磁盘预检
  discovery       设备模型、mDNS、UDP 探测、NSD/Bonjour 桥、发现管理器
  transfer        传输引擎：QUIC+TCP 双栈、会话状态机、收发流水线
  task            应用门面：事件驱动、任务管理、配置、编排调度
  ffi             C ABI 导出（Windows bt_ffi.dll / Android libbt_ffi.so / iOS BoltEngine.xcframework）

tools/cli         命令行联调工具（serve / send / discover）
tauri_app/        跨平台桌面客户端（Tauri v2 + Vue 3，支持 Windows / Linux）
android_app/      Android 客户端（Kotlin + Jetpack Compose）
ios_app/          iOS 客户端（Swift + SwiftUI）
docs/             设计文档与规范
scripts/          构建与测试脚本
```

### 数据流

```
┌────────────────┐          QUIC / TCP + TLS 1.3          ┌────────────────┐
│   发送端 App   │ ◄────────────────────────────────────► │   接收端 App   │
│     (task)     │      mDNS / Bonjour / UDP 发现同步     │     (task)     │
└───────┬────────┘                                        └───────┬────────┘
        │ FFI (C ABI)                                             │ FFI (C ABI)
┌───────┴────────┐                                        ┌───────┴────────┐
│  Tauri (前端)  │                                        │  Swift / Kotlin│
│(Windows/Linux) │                                        │ (iOS / Android)│
└────────────────┘                                        └────────────────┘
```

## 🚀 快速开始

### 运行预编译版
Bolt 提供了开箱即用的多端安装包与免安装便携版，可直接从 [dist/](dist/) 目录或 Releases 下载使用：
- **Windows 端**：直接运行 `dist/windows/release/portable/bolt_0.1.0_x64-portable.exe` 或使用 NSIS/MSI 安装向导。
- **Linux 端**：赋予可执行权限后直接运行 `dist/linux/release/appimage/bolt_0.1.0_amd64.AppImage`，或安装 `.deb` / `.rpm` 软件包。
- **Android 端**：在手机上安装 `dist/android/release/bolt_0.1.0_universal.apk`。
- **iOS 端**：在 Xcode 中打开 `ios_app/Bolt.xcodeproj` 并连接真机运行，或使用 `scripts/build_ios_dist.sh` 编译导出。

### 源码一键构建
如需从源码编译全平台发行包，可使用一键自动化打包脚本：

```powershell
# Windows 端全量打包（便携版 + CLI + NSIS + MSI 四大形态）
powershell.exe -ExecutionPolicy Bypass -File scripts\build_windows_dist.ps1 -Mode release

# Android 端全量打包（跨编译 SO 动态库 + R8 优化压缩 APK）
powershell.exe -ExecutionPolicy Bypass -File scripts\build_android_dist.ps1 -Mode release
```

```bash
# Linux 端全量打包（AppImage + DEB + RPM + CLI 四大形态）
bash scripts/build_linux_dist.sh -m release

# iOS 端全量打包（跨编译真机+模拟器静态库与 XCFramework，并导出 Xcode Archive）
bash scripts/build_ios_lib.sh release
bash scripts/build_ios_dist.sh Release
```

> 💡 完整的环境搭建、分步打包、单项形态编译、质量门禁与排障细节，请参阅 📖 [编译指南](docs/build_guide.md)。

### 命令行联调（bolt-cli）

```bash
# 启动接收端（监听 8899 端口）
cargo run -p bolt-cli -- serve --port 8899 --data-dir target/cli_a

# 发送文件至目标设备
cargo run -p bolt-cli -- send --data-dir target/cli_b 127.0.0.1:8899 ./file.zip

# 浏览局域网中的在线设备
cargo run -p bolt-cli -- discover
```

## 📱 使用说明

1. **确保设备在同一局域网**（Wi-Fi 或有线均可）
2. **启动应用**：Windows / Linux 桌面端打开应用，Android 端打开 App
3. **自动发现**：设备列表自动显示对方设备
4. **首次配对**：点击设备名连接，双方屏幕显示 4 位验证码，确认一致后信任
5. **选择文件发送**：选择文件或文件夹，点击发送，对方确认接收即开始传输
6. **传输完成**：自动 BLAKE3 校验，确保文件完整无损

## 🔒 安全机制

| 层面 | 实现 |
|------|------|
| **传输加密** | TLS 1.3 强制，无明文模式 |
| **设备身份** | Ed25519 自签证书（10 年有效期） |
| **身份验证** | BLAKE3 指纹 + 4 位验证码 + TOFU 信任库 |
| **文件校验** | BLAKE3 全文件哈希，字节级一致性保证 |
| **隐私保护** | 零外网请求、零遥测、日志不含文件内容与密钥 |

## 📖 文档

| 文档 | 说明 |
|------|------|
| [协议规范](docs/protocol_spec.md) | 帧格式、指令码、握手流程、错误码 |
| [FFI API](docs/ffi_api.md) | `bt_*` C ABI 接口与事件定义 |
| [开发指南](docs/dev_guide.md) | 架构速览、全局常量、新增功能检查单 |
| [编译指南](docs/build_guide.md) | 环境配置、质量门禁、Windows/Linux/Android 全形态打包流程 |
| [项目结构](docs/project_structure.md) | 仓库完整文件树、分层约束与打包产物规范 |
| [隐私政策](docs/privacy_policy.md) | 纯局域网数据安全、操作系统权限与合规声明 |

## 📜 许可

本项目基于 [MIT License](LICENSE) 开源协议分发与使用。
