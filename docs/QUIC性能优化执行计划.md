# QUIC 局域网极致传输优化实施总结

针对项目在局域网环境下基于 QUIC 协议传输速率低下、无法发挥高吞吐潜力的问题，我们已按照经过充分论证并优化的执行方案，完成了核心传输引擎与文件落盘层的全链路重构与优化。

---

## 一、 核心改造与优化点汇总

### 1. QUIC 传输层参数解禁与拥塞调优 (`crates/lt-transfer/src/quic.rs`)
- **流控窗口全面解禁**：
  - 流接收窗口从 Quinn 默认的小窗口提至 **8MB**（`stream_receive_window`）；
  - 连接接收窗口与发送窗口提至 **32MB**（`receive_window` & `send_window`），彻底解除高带宽低延迟（高 BDP）下的流控刹车。
- **MTU 优化与包头开销削减**：
  - 初始 MTU 直接设为 **1472 字节**（`initial_mtu(1472)`，跑满标准以太网 1500 字节帧）；
  - 开启 `mtu_discovery_config`（DPLPMTUD 动态探测），兼顾特殊网络或 VPN 环境的安全下调回退。
- **拥塞控制切换为 CUBIC**：
  - 移除此前 Wi-Fi 场景下使用的 BBR，改用适合内网高带宽环境的 `CubicConfig`，杜绝 BBR 周期性 ProbeRTT（降至 4 包探测）带来的周期性速度暴跌。
- **UDP 套接字缓冲调优**：
  - 预绑定套接字收发缓冲区提升至 **8MB**（`UDP_BUF_SIZE`），并在 Windows / Linux / Android 兼顾平台最大限制。

### 2. 消除 200ms ACK 延迟，解脱发送端饥饿 (`crates/lt-transfer/src/recv.rs` & `send.rs`)
- **即时累积确认（Immediate Cumulative ACK）**：
  - 在 `recv::on_data` 中，一旦累计接收到 **4MB** 未确认数据且连续前缀推进，**立即原地向发送端回传 ACK 帧**；
  - 调度器的 200ms 定时器仅保留作为文件尾部最后不满 4MB 数据的保底机制。
- **发送端在途窗口扩容**：
  - 发送端 `IN_FLIGHT_WINDOW` 从 16MB 扩至 **32MB**。配合接收端 4MB 即时 ACK，发送流水线保持 100% 饱满不间断滑动，杜绝了此前发完 16MB 必须休眠等待 180ms 的严重空转现象。

### 3. 异步零拷贝与向量化写入 (`crates/lt-transfer/src/conn.rs`, `protocol.rs`, `send.rs`)
- **协议载荷引入 `bytes::Bytes`**：
  - 将 `Message::Data.payload` 由 `Vec<u8>` 改为 `bytes::Bytes`；
  - 接收端 `conn::read_frame` 将网络缓冲直接转为 `Bytes`，在 `protocol::decode` 中利用 `payload.slice(12..)` 实现**单分片载荷零拷贝解包**。
- **DATA 帧两段向量化写入**：
  - 发送端在 `conn::write_frame` 中针对 `Message::Data` 采用头部（25 字节栈数组）与 payload 分离写入，直接绕过 `protocol::encode` 的二次内存合并拷贝，消除每分片 1MB 的频繁堆分配；
  - 移除每帧之后的同步 `w.flush()`，由底层传输层自动批量聚合发送。

### 4. 磁盘写入优化与系统调用精简 (`crates/lt-file/src/writer.rs`)
- **连续写入避让 Seek 系统调用**：
  - `FileWriter` 内部增加 `current_offset` 游标跟踪。对绝大多数顺序到达的分片，直接执行追加写入，不再频繁调用 `SeekFrom::Start(offset)`（减少了 Windows `SetFilePointerEx` 和 Linux `lseek` 系统调用）。
- **文件尺寸预分配（Pre-allocation）**：
  - 文件在 `create` 和 `resume` 阶段即时调用 `set_len(size)` 进行磁盘空间预留，避免写入过程中文件系统频繁分配簇和更新元数据引发的磁盘 IO 抖动。

---

## 二、 关键修改代码索引

| 模块 | 修改文件 | 核心改动 |
|---|---|---|
| `lt-transfer` | [`quic.rs`](file:///e:/AIProjects/local_transfer/crates/lt-transfer/src/quic.rs) | 解禁 8MB/32MB 窗口、配置 1472 MTU 与 MTU 探测、启用 CUBIC、加大 UDP 缓冲至 8MB |
| `lt-transfer` | [`protocol.rs`](file:///e:/AIProjects/local_transfer/crates/lt-transfer/src/protocol.rs) | `Message::Data` 改用 `bytes::Bytes`，`decode` 支持零拷贝分片解包 |
| `lt-transfer` | [`conn.rs`](file:///e:/AIProjects/local_transfer/crates/lt-transfer/src/conn.rs) | `write_frame` 增加 DATA 两段向量化写入，移除帧级强制 flush；`read_frame` 转 `Bytes` 零拷贝切片 |
| `lt-transfer` | [`send.rs`](file:///e:/AIProjects/local_transfer/crates/lt-transfer/src/send.rs) | `IN_FLIGHT_WINDOW` 提至 32MB，使用 `Bytes::copy_from_slice` 构建分片 |
| `lt-transfer` | [`recv.rs`](file:///e:/AIProjects/local_transfer/crates/lt-transfer/src/recv.rs) | `on_data` 增加 4MB 阈值即时 ACK 刷新，消除 200ms 定时器空转 |
| `lt-file` | [`writer.rs`](file:///e:/AIProjects/local_transfer/crates/lt-file/src/writer.rs) | `FileWriter` 增加 `current_offset` 消除冗余 seek，增加 `set_len` 磁盘预分配 |

---

## 三、 预期效果与后续验证

1. **吞吐速率质的飞跃**：
   - 此前瓶颈主要受限于“16MB 窗口 + 200ms ACK 定时器延时”（理论上限仅 80MB/s，抖动时仅 15~30MB/s）；
   - 优化后发送端与接收端形成 32MB 连续滑动流水线，结合 CUBIC 与 1472 MTU，局域网千兆/2.5G/Wi-Fi 6 链路可贴近物理带宽极限（预期千兆达 100~115MB/s，同机环回可达数百 MB/s 至 GB/s）。
2. **CPU 与内存占用明显下降**：
   - 杜绝了每个 1MB 分片的多次堆内存分配与二次拷贝，大幅降低 GC/内存回收与 L3 缓存抖动。
