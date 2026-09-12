# QUIC 局域网极致传输优化实施总结

针对项目在局域网环境下基于 QUIC 协议传输大文件时连接不稳定、速率不达预期的问题，
我们完成了两轮迭代优化，最终实现了大文件传输的绝对稳定性与合理吞吐。

---

## 一、问题根因

### 大文件传输断连（致命 Bug）

- **现象**：Android → Windows 传输 50MB+ 文件时，几秒钟内 QUIC 连接断开，
  报错 `INTERNAL_ERROR`（`connection lost`），小文件传输正常。
- **根因**：Quinn 内部 `Assembler`（乱序分片重组器）硬限制 `MAX_CHUNKS = 1024`。
  当 `stream_receive_window` 设为 64MB、分片 1MB 时，Wi-Fi 丢包导致乱序分片
  碎片数瞬间超过 1024，触发 `quinn-proto` 强制关闭连接。
- **关键线索**：日志中 `too many gaps in stream buffer` 指向
  `quinn-proto/src/assembler.rs:139`。

### 吞吐受限（性能瓶颈）

- 200ms 定时器节流 ACK 导致发送端流水线停等。
- 每 DATA 帧经 `protocol::encode` 二次内存拷贝。
- 接收端逐块 `seek` 写入产生冗余系统调用。

---

## 二、最终参数方案（生产验证）

### QUIC 传输层（`crates/transfer/src/quic.rs`）

| 参数 | 最终值 | 说明 |
|------|--------|------|
| `stream_receive_window` | **1 MB** | 核心安全约束：1MB / 256KB分片 ≈ 4 个分片，远低于 MAX_CHUNKS=1024 |
| `receive_window` | `VarInt::MAX` | 连接级不限流，多流并发总量不触顶 |
| `send_window` | 4 MB | 发送缓冲，配合应用层背压 |
| `initial_mtu` | 1200 | RFC 9000 安全基线，兼容 Wi-Fi / VPN |
| MTU Discovery | 开启 | DPLPMTUD (RFC 8899) 自动探测 |
| 拥塞控制 | BBR | 局域网高带宽场景表现最优 |
| `keep_alive_interval` | 10s | 保活周期 |
| `max_idle_timeout` | 90s | 空闲断连阈值 |
| 最大并发流 | 64 | 双向/单向 |

### 应用层常量（`crates/utils/src/constants.rs`）

| 常量 | 值 | 说明 |
|------|----|------|
| `DEFAULT_CHUNK_SIZE` | 256 KB | 文件分片大小 |
| `IN_FLIGHT_WINDOW_BYTES` | 4 MB | 发送端在途字节窗口 |
| `ACK_THRESHOLD_BYTES` | 256 KB | 接收端即时 ACK 阈值 |
| `DEFAULT_CONCURRENCY` | 4 | 并行文件流数 |
| UDP 缓冲 | 8 MB | SO_RCVBUF / SO_SNDBUF |

### 参数设计原则

```
stream_receive_window (1MB) ÷ chunk_size (256KB) = 4 个分片 / 流
即使 100% 乱序，碎片数也远低于 MAX_CHUNKS=1024，绝不触发断连。

in_flight_window (4MB) ÷ chunk_size (256KB) = 16 个分片在途
配合 256KB 阈值 ACK，发送端流水线始终饱满。
```

---

## 三、性能优化点汇总

### 1. 消除 200ms ACK 延迟

- 接收端每累积 256KB 未确认数据，**立即回传 ACK**。
- 200ms 定时器仅作为文件尾部不满阈值的保底机制。
- 效果：发送端流水线不再因等待 ACK 而停等。

### 2. 零拷贝与向量化写入

- `Message::Data.payload` 使用 `bytes::Bytes`，`decode` 阶段 `slice()` 零拷贝解包。
- DATA 帧发送采用头部（25B 栈数组）+ payload 两段向量化写入，绕过 `encode` 二次拷贝。
- 移除每帧同步 `flush()`，由传输层自动批量聚合。

### 3. 磁盘写入优化

- `FileWriter` 内部 `current_offset` 游标跟踪，顺序到达的分片直接追加写入免 `seek`。
- 文件创建阶段即 `set_len(size)` 预分配磁盘空间，减少文件系统元数据更新开销。

### 4. UI 一致性

- 移除 Windows/Android 端非功能性的暂停/恢复按钮。
- 统一任务状态为：`waiting_accept → transferring → done/cancelled/error/rejected`。

---

## 四、关键代码索引

| 模块 | 文件 | 核心改动 |
|------|------|----------|
| `utils` | `constants.rs` | 全局统一常量定义（窗口/分片/超时/状态） |
| `transfer` | `quic.rs` | 1MB 流窗口 + VarInt::MAX 连接窗口 + 1200 MTU + BBR |
| `transfer` | `protocol.rs` | `Message::Data` 改用 `bytes::Bytes` 零拷贝 |
| `transfer` | `conn.rs` | DATA 两段向量化写入，移除帧级 flush |
| `transfer` | `send.rs` | 4MB 在途窗口 + 背压等待机制 |
| `transfer` | `recv.rs` | 256KB 阈值即时 ACK + 200ms 保底 |
| `file` | `writer.rs` | current_offset 免 seek + set_len 预分配 |

---

## 五、实测验证

| 测试场景 | 结果 |
|----------|------|
| 50MB 文件 Wi-Fi 传输（Android → Windows） | ✅ 稳定完成，~22 MB/s |
| 100MB+ 文件传输 | ✅ 无断连，哈希 100% 一致 |
| 小文件批量传输（数十个 < 1MB） | ✅ 全部成功 |
| BLAKE3 全文件校验 | ✅ 100% 一致 |
| 单元测试 | ✅ transfer + task 全部通过 |

---

## 六、已放弃的方案

以下方案在理论分析或实测中被证明不适合本项目：

| 方案 | 放弃原因 |
|------|----------|
| 64MB stream_receive_window | Quinn MAX_CHUNKS=1024 硬限制，Wi-Fi 丢包必触发断连 |
| 32MB/16MB 大窗口 | 同上，只是概率降低但不彻底 |
| 1472 initial_mtu | 部分 Wi-Fi AP MTU < 1400，导致静默丢包黑洞 |
| BBR → CUBIC 切换 | 实测 BBR 在局域网表现更优，CUBIC 慢启动阶段太长 |
| 暂停/恢复功能 | 实现复杂度高，实际使用场景极少，移除以简化代码 |
