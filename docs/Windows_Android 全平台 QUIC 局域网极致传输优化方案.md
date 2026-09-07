# Windows/Android 全平台 QUIC 局域网极致传输优化方案

> 本文档为初期设计方案记录。最终实施参数见 [QUIC性能优化执行计划.md](QUIC性能优化执行计划.md)。

## 设计目标

解决 QUIC 协议在局域网高带宽、低延迟环境下因默认参数保守导致的吞吐瓶颈与稳定性问题。
通过统一跨平台（Windows/Android）的底层网络参数、优化 IO 模型，使 QUIC 传输性能稳定可靠。

## 核心参数设计（最终生产版本）

### QUIC 传输层 (TransportConfig)

- **流窗口约束**：stream_receive_window = 1MB（Quinn MAX_CHUNKS=1024 安全限制）
- **连接窗口不限**：receive_window = VarInt::MAX
- **MTU 安全探测**：initial_mtu = 1200，开启 DPLPMTUD 自动探测
- **拥塞控制**：BBR（局域网高带宽场景最优）
- **并发控制**：双向/单向流上限 64

### UDP 内核缓冲

- 强制 8MB 缓冲区（SO_RCVBUF/SO_SNDBUF），防内核静默丢包

### 应用层流控

- 发送端在途窗口：4MB（IN_FLIGHT_WINDOW_BYTES）
- 分片大小：256KB（DEFAULT_CHUNK_SIZE）
- ACK 阈值：256KB 即时回传 + 200ms 保底

## 关键教训

1. **不可忽视 Quinn 内部限制**：MAX_CHUNKS=1024 是硬编码，大窗口在丢包时必定触发断连
2. **MTU 不宜激进**：1472 在部分 Wi-Fi 环境会导致黑洞丢包，1200 是安全基线
3. **稳定性优先于极致吞吐**：22MB/s 的稳定传输远优于偶发断连的 100MB/s
