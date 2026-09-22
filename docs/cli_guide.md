# Bolt CLI 使用指南 (bolt-cli)

`bolt-cli` 是基于 Rust 核心传输内核构建的**原生免 GUI 命令行独立工具**。它不依赖任何图形界面或 Web 引擎，可直接在终端中运行，广泛适用于**无头服务器（Headless Linux / macOS）、自动化测试脚本、网络协议联调及跨平台快速对传**。

---

## 目录

1. [获取与运行方式](#1-获取与运行方式)
2. [语法与全局参数](#2-语法与全局参数)
3. [核心子命令详解](#3-核心子命令详解)
   - [discover：局域网设备发现](#1-discover设备发现)
   - [send：发送文件与文件夹](#2-send文件发送)
   - [serve：后台监听接收服务](#3-serve监听接收)
4. [高频实战场景](#4-高频实战场景)
   - [场景一：跨设备直接 IP 发送](#场景一跨设备直接-ip-发送免广播发现)
   - [场景二：无头 Linux/Mac 作为自动化接收节点](#场景二无头-linuxmac-作为自动化接收节点)
   - [场景三：单机双开冒烟自测（Loopback）](#场景三单机双开冒烟自测loopback)
   - [场景四：CLI 与桌面端 GUI 互传](#场景四cli-与桌面端-gui-互传)

---

## 1. 获取与运行方式

### 方式 A：源码直接运行（开发调试）
在仓库根目录直接通过 Cargo 执行：
```bash
cargo run -p bolt-cli -- <子命令> [参数]
```

### 方式 B：本地编译为独立二进制文件
```bash
# 编译 Release 优化版本
cargo build --release -p bolt-cli

# 产物路径：
# Windows: target/release/bolt-cli.exe
# macOS / Linux: target/release/bolt-cli
```

### 方式 C：直接使用打包产物
在 GitHub Actions 自动化构建产物（Artifacts）或发行包中，可直接获取各平台的预编译二进制：
* Windows: `bolt-windows-cli` (`bolt-windows-<ver>-x64-cli.exe`)
* macOS: `bolt-macos-cli` (`bolt-macos-<ver>-universal-cli`)
* Linux: `bolt-linux-cli` (`bolt-linux-<ver>-amd64-cli`)

---

## 2. 语法与全局参数

```bash
bolt-cli <serve | discover | send> [全局参数] [目标/文件路径...]
```

### 全局可选参数

| 参数 | 默认值 | 说明 |
| :--- | :--- | :--- |
| `--port <端口>` | `8899` | 指定传输引擎监听端口（QUIC / TCP 共享主端口，TCP 备用端口为 `port+1`）。 |
| `--name <名称>` | 本机主机名 | 指定在局域网广播与发现中展示的设备友好名称（如 `--name "Storage-Node-01"`）。 |
| `--data-dir <目录>` | 系统默认数据目录 | 指定独立的数据持久化目录（存储 Ed25519 身份密钥、配置、配对信任库与接收目录）。 |

> 💡 **提示**：通过指定不同的 `--port` 与 `--data-dir`，可以在同一台物理机上并发运行多个互不干扰的 `bolt-cli` 实例。

---

## 3. 核心子命令详解

### 1. `discover`：设备发现

持续扫描并实时打印当前局域网内所有在线的 Bolt 节点（通过 mDNS 与 UDP 探测双通道）。

```bash
# 扫描局域网设备
bolt-cli discover

# 指定独立数据目录浏览
bolt-cli discover --data-dir ./target/cli_debug
```

* **输出示例**：
  ```text
  正在浏览局域网设备…（Ctrl-C 退出）
  ---- 设备列表（2 台）----
    u-550e8400... | geng 电脑 | 192.168.122.1 | QUIC:8899 TCP:8899 | via udp_probe
    u-6ba7b810... | ian's Mac | 192.168.122.128 | QUIC:8899 TCP:8899 | via mdns
  ```
  可按 `Ctrl + C` 退出扫描。

---

### 2. `send`：文件发送

向指定目标设备发起文件/文件夹传输。

```bash
bolt-cli send [--port <端口>] [--data-dir <目录>] <目标设备> <路径1> [路径2...]
```

#### 参数说明：
* **`<目标设备>`** 支持以下两种形式：
  1. **IP 地址或 IP:端口（推荐直连）**：例如 `192.168.122.128:8899` 或 `192.168.3.69`（若目标监听默认端口 8899 可省略端口）。
  2. **设备 UUID**：通过 `discover` 命令扫描到的目标 UUID（例如 `u-6ba7b810...`）。
* **`<路径...>`**：支持一个或多个文件、文件夹相对路径或绝对路径（文件夹会自动进行递归遍历）。

#### 运行示例：
```bash
# 发送单个文件到指定 IP
bolt-cli send 192.168.122.128:8899 ./archive.tar.gz

# 一次性批量发送多个文件及整个目录
bolt-cli send 192.168.122.128:8899 ./document.pdf ./photos/ ./video.mp4
```

#### 控制台实时进度条：
传输过程中，终端提供单行原地刷新的动态传输状态指示：
```text
[发送 video.mp4] 45.2% 452.0MB/1.0GB 速率 68.4MB/s 剩余 8s
```
* 传输结束后打印汇总报告（成功数与失败数），传输完成自动以退出码 `0` 退出；若失败则返回非 `0`。

---

### 3. `serve`：监听接收

启动无头服务节点，常驻前台监听连接、配对与文件传输请求。

```bash
# 使用默认端口 8899 启动监听
bolt-cli serve

# 自定义监听端口与设备名称
bolt-cli serve --port 8900 --name "Linux-Backup-Server" --data-dir /data/bolt
```

#### 交互行为说明：
* **服务就绪提示**：打印当前节点的设备 UUID、引擎绑定端口与本地身份公钥指纹。
* **配对请求交互**：当遇到未配对设备请求连接时，终端弹出配对验证码确认：
  ```text
  [配对] Windows-PC(u-550e...) 请求配对，验证码：【7492】
    屏幕验证码一致，接受配对？ [y/N] 
  ```
  输入 `y` 确认信任，后续该设备再次传输将进入免确认信任通道。
* **文件接收确认**：当接收到传输意向时提示：
  ```text
  [传输请求] Windows-PC(u-550e...) 要发送 3 个文件，共 150.8MB 字节
    接受？ [y/N] 
  ```
  输入 `y` 后开始流式接收并实时落盘至该实例的接收目录，落盘前自动执行 BLAKE3 分块与全文件哈希校验。

---

## 4. 高频实战场景

### 场景一：跨设备直接 IP 发送（免广播发现）
在跨网段、跨 VLAN 或开启了 AP 隔离导致局域网广播受限的网络环境下，直接使用目标 IP 发送：

```bash
# 接收端（在 192.168.1.100 上启动）
bolt-cli serve --port 8899

# 发送端（在任意网络可达的设备上发送）
bolt-cli send 192.168.1.100:8899 ./release_v1.zip
```

---

### 场景二：无头 Linux/Mac 作为自动化接收节点
适合部署在 NAS、家庭服务器或虚拟机后台作为中继与存储节点：

```bash
# 启动后台服务（指定专用存储目录）
bolt-cli serve --name "Home-NAS" --data-dir /mnt/storage/bolt
```

---

### 场景三：单机双开冒烟自测（Loopback）
在无需第二台物理设备的情况下，在一台机器上通过不同端口和隔离数据目录验证双向传输：

* **终端 A（监听端）**：
  ```bash
  cargo run -p bolt-cli -- serve --port 8901 --data-dir target/data_node_a
  ```
* **终端 B（发送端）**：
  ```bash
  cargo run -p bolt-cli -- send --port 8902 --data-dir target/data_node_b 127.0.0.1:8901 ./testfile.bin
  ```

---

### 场景四：CLI 与桌面端 GUI 互传
`bolt-cli` 与 Windows、macOS 桌面端（Tauri）及移动端（Android / iOS）完全基于同一套传输协议规范：
* 终端启动 `bolt-cli serve` 后，桌面 GUI 客户端的“可用设备”列表会自动发现该命令行节点。
* 桌面端选中该节点，即可像操作普通电脑一样直接向命令行节点拖拽投送文件。
