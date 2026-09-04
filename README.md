# LocalTransfer

纯局域网、无服务器的 P2P 文件传输工具（Windows + Android）。

- **点对点直连**：QUIC 优先，自动降级 TCP + TLS 1.3，全程加密
- **零配置发现**：mDNS（`_lt._udp.local.`）+ UDP 广播探测双通道
- **安全配对**：Ed25519 设备证书 + BLAKE3 指纹 + 6 位验证码人工比对 + TOFU 信任库
- **高速可靠**：多文件并发、累积确认、BLAKE3 全文件校验
- **断点续传**：中断后按已收区间协商续传，绝不重传
- **无服务器、无账户、无外网请求、无遥测**

## 架构

```
crates/
  lt-utils       公共设施（错误码/配置/UUID/网络）
  lt-crypto      身份、证书、指纹、配对码、信任库、TLS
  lt-file        文件遍历/读取/写入、区间集、断点缓存、磁盘预检
  lt-discovery   设备模型、mDNS、UDP 探测、NSD 桥
  lt-transfer    引擎：QUIC+TCP 双栈、会话状态机、收发流水线
  lt-task        应用门面：事件、任务、配置、编排
  lt-ffi         C ABI（Windows lt_ffi.dll / Android liblt_ffi.so）
tools/lt-cli     命令行联调端
tauri_app/       Windows 桌面端（Tauri v2 + Vue3 + TS）
android_app/     Android 端（Kotlin + Gradle）
docs/            方案与规范（protocol_spec.md / ffi_api.md / dev_guide.md）
scripts/         构建与冒烟脚本
```

## 快速开始（lt-cli）

```bash
cargo build -p lt-cli

# 接收端
./target/debug/lt-cli.exe serve --port 8899 --data-dir target/cli_a

# 发送端（同机环回）
./target/debug/lt-cli.exe send --data-dir target/cli_b 127.0.0.1:8899 ./some/file ./some/dir

# 浏览设备
./target/debug/lt-cli.exe discover
```

环回冒烟（CI 同款，覆盖 QUIC / 强制 TCP / 断点续传）：

```bash
bash scripts/smoke_loopback.sh
```

## 桌面端（Tauri）

```bash
cd tauri_app && npm install && npm run tauri dev
```

## Android 端

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
scripts/build_android_lib.bat   # 需要 ANDROID_NDK_HOME
# Android Studio 打开 android_app/ 构建
```

## 质量门禁

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash scripts/smoke_loopback.sh
```

## 文档

- [传输协议规范](docs/protocol_spec.md) — 帧格式、指令码、握手、续传、错误码
- [FFI API](docs/ffi_api.md) — `lt_*` C ABI 与事件
- [开发指南](docs/dev_guide.md) — 架构速览与新增功能检查单
- [实施方案](docs/LocalTransfer-可行性实施方案.txt) — 原始方案（LT-RUST-IMPL-20260828）

## 安全声明

- 局域网内使用，无任何外部网络请求与遥测
- 传输全程 TLS 1.3 加密，设备身份经指纹 + 验证码 + TOFU 三重校验
- 日志不含文件内容与密钥材料
