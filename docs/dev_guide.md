# LocalTransfer 开发指南

## 仓库布局

```
crates/
  lt-utils      公共设施：错误码、配置、UUID、网络、全局常量（constants.rs）
  lt-crypto     设备身份（Ed25519 自签证书）、指纹、配对码、信任库、TLS 配置
  lt-file       遍历/读取/写入（BLAKE3 校验+落盘）、磁盘预检
  lt-discovery  设备模型、mDNS（mdns-sd 0.21）、UDP 探测、NSD 桥、发现管理器
  lt-transfer   传输引擎：QUIC(quinn 0.11)+TCP 双栈、会话状态机、收发流水线、协议编解码
  lt-task       应用门面 App：事件、任务记录、配置、发现/引擎编排
  lt-ffi        C ABI（lt_* 前缀，cbindgen 生成 include/lt_api.h）
tools/lt-cli    命令行联调端（serve/discover/send）
tauri_app/      Windows 桌面端（Tauri v2 + Vue3 + TS）
android_app/    Android 端（Kotlin + Gradle，jniLibs 装载 liblt_ffi.so）
scripts/        构建/冒烟脚本
docs/           方案与规范文档
```

## 环境要求

- Rust ≥ 1.85（开发环境 1.98），MSVC 工具链（Windows）
- Windows：`cargo` + MSVC；Android：`cargo-ndk` + NDK；Tauri：Node 18+
- Git Bash（运行 bash 脚本）

## 常用命令

```bash
cargo build --workspace          # 全量编译
cargo test --workspace           # 全量测试
cargo clippy --workspace --all-targets -- -D warnings   # 门禁级 lint
cargo fmt --all                  # 格式化

cargo run -p lt-cli -- serve --port 8899 --data-dir target/cli_a   # 接收端
cargo run -p lt-cli -- send --data-dir target/cli_b 127.0.0.1:8899 ./some/file
cargo run -p lt-cli -- discover
```

环境变量：

- `LT_FORCE_TCP=1`：拨号侧跳过 QUIC 直走 TCP（联调/排障）。
- `RUST_LOG=info,lt_transfer=debug`：模块级日志（tracing）。

## 架构速览

1. **App（lt-task）** 是唯一门面：持有 tokio 运行时、配置、身份、信任库、
   设备表、任务表、会话表；`SessionHandler` 经 `Weak<App>` 回指避免循环引用。
2. **会话（lt-transfer::session）**：握手 → 配对/信任 → 控制管道 0 +
   调度器（单消费点路由所有入站帧）+ 心跳（30s/90s）。文件管道：QUIC 每文件
   独立双向流；TCP 复用控制管道。
3. **接收端状态** 全部活在调度器协程内（`recv_tasks`/`seq_index`），
   不跨协程共享，故无锁；对上层只发事件。
4. **事件流**：引擎 → `EngineEvent` → App 记账 → `LtEvent` → 事件槽
   （CLI 用 std mpsc / FFI 用 crossbeam_channel 队列+专用线程）。
5. **发现**：mDNS（非 Android 平台）+ UDP 广播探测双通道，`DeviceList`
   去重聚合，10s 过期；Android 由 Kotlin NSD 桥注入。

## 全局常量管理

所有传输相关的硬编码常量统一定义在 `crates/lt-utils/src/constants.rs`，
各模块通过 `use lt_utils::constants::*` 引用，修改一处全局生效：

| 常量 | 值 | 用途 |
|------|----|------|
| `DEFAULT_CHUNK_SIZE` | 256KB | 文件分片大小 |
| `IN_FLIGHT_WINDOW_BYTES` | 4MB | 发送端在途窗口 |
| `ACK_THRESHOLD_BYTES` | 256KB | 接收端累积确认阈值 |
| `QUIC_STREAM_FLOW_CONTROL_WINDOW` | 1MB | QUIC 单流流控窗口 |
| `DEFAULT_CONCURRENCY` | 4 | 默认并行文件传输流数 |

> **⚠️ 重要**：`QUIC_STREAM_FLOW_CONTROL_WINDOW` 不可随意增大。Quinn 内部
> `Assembler` 硬限制 `MAX_CHUNKS=1024`，大窗口 + 丢包场景会导致乱序碎片数
> 超限，触发 `INTERNAL_ERROR` 强制断连。当前 1MB 窗口 + 256KB 分片是经过
> 生产验证的安全组合。

## 新增功能检查单

- [ ] 协议帧：`lt-transfer/src/protocol.rs` 编解码 + 单元测试往返
- [ ] 会话路由：`session.rs handle_frame` 增分支
- [ ] 应用层：`lt-task/src/app.rs` 事件/记账
- [ ] FFI：`lt-ffi/src/lib.rs`（cbindgen 自动更新 `include/lt_api.h`）
- [ ] CLI 联调：`tools/lt-cli/src/main.rs`
- [ ] `cargo fmt` + `cargo clippy -D warnings` + `cargo test --workspace`

## Windows 桌面端（tauri_app）

```bash
cd tauri_app
npm install
npm run tauri dev      # 开发模式
npm run tauri build    # 安装包（NSIS）
```

Rust 侧在 `src-tauri` 内以 `#[tauri::command]` 封装 `lt-ffi`，
经 `Emitter` 把 `lt_set_event_callback` 的事件转发给前端。

**UI 说明**：当前版本传输任务卡片只有「取消」按钮，
不提供暂停/恢复功能（已移除）。

## Android 端（android_app）

1. `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`
2. `cargo install cargo-ndk`；安装 NDK（Android Studio SDK Manager 勾选
   "NDK (Side by side)"，或手动解压到 `%ANDROID_HOME%\ndk\<版本>`）
3. `scripts/build_android_lib.bat` 产出 `jniLibs/*/liblt_ffi.so`
   （自动探测 NDK：`ANDROID_NDK_HOME` → `%ANDROID_HOME%\ndk\*` → 默认 AS 路径）
4. 构建 APK（Android Studio 或 **VS Code 均可**）：

```bash
cd android_app
./gradlew.bat :app:assembleDebug            # 产物 app/build/outputs/apk/debug/app-debug.apk
# 真机安装：
"$ANDROID_HOME/platform-tools/adb.exe" install -r app/build/outputs/apk/debug/app-debug.apk
```

- Gradle wrapper 锁定 **8.7**（AGP 8.5 与 Gradle 9 不兼容；wrapper 已随仓库提交，
  无 wrapper 时可用系统 Gradle 在空目录 `gradle wrapper --gradle-version 8.7` 生成）。
- `android_app/local.properties` 指向本机 SDK（已 gitignore）。
- 首次构建需联网下载 Gradle 发行版与 AGP 依赖，之后全离线。

注意：Android 上 Rust 侧不启用 mDNS（`use_mdns=false`），Kotlin 用
`NsdManager` 发现并调 `lt_nsd_inject_device` 桥接；文件访问走 SAF 授权路径。

## 测试约定

- 单元测试自隔离：临时目录名含 `std::process::id()` **且按用例区分**
  （曾发生 fixture 并行互删导致 flaky）。
- 环回冒烟是验收门禁：QUIC 通道、强制 TCP 通道、TOFU 免二次配对、
  字节级内容一致，全部断言通过才算过。
- 安全红线：无外部网络请求、无遥测；日志不得含文件内容与密钥材料。

## 已知限制

- 暂停/恢复功能已移除，传输一旦开始只能取消或等待完成。
- Wi-Fi 环境下丢包率较高时，吞吐会因 QUIC 重传而下降，但不会断连。
- 单流窗口限制为 1MB，在极低延迟（<0.1ms）的有线局域网中可能无法完全跑满带宽。
