# Bolt 传输协议规范（Protocol V2）

> 对应实施方案。本文档描述各 crate
> 实际实现的线上格式。所有多字节整数均为**大端**。

## 1. 帧格式（通用）

```
[4B 长度 BE][1B 指令码][8B 会话/任务 ID BE][payload]
```

- 长度 = 1B 指令码 + 8B 会话 ID + payload 字节数；帧总长最小 9 字节，上限 `MAX_FRAME_LEN`（协议层默认 16MB）。
- 指令码未知或帧畸形：回 `ERROR(-13)` 并**保持连接**（4.3-1）。
- 字符串均为 UTF-8，以 2B 长度前缀 + 字节流编码。
- 会话 ID 字段：控制面为 0 或任务 ID；文件面为任务 ID。

## 2. 指令码

### 控制面（0x01–0x0C）

| 码 | 名称 | 方向 | 说明 |
|----|------|------|------|
| 0x01 | HELLO | 双向 | 协议版本、能力位、UUID、设备名、设备类型、BLAKE3 证书指纹 |
| 0x02 | PAIR_REQ | C→S | 16B nonce；触发 4 位验证码展示 |
| 0x03 | PAIR_RESP | S→C | accept(bool) + 16B nonce |
| 0x04 | TRANSFER_REQ | S→R | file_count、total_size、sender_name；入站任务 ID = 本帧会话 ID |
| 0x05 | TRANSFER_RESP | R→S | accept(bool) |
| 0x06 | FILE_META | S→R | file_seq、size、mtime_unix、rel_path、chunk_size、hash_algo |
| 0x07 | FILE_META_ACK | R→S | file_seq、accept(bool) |
| 0x08 | CANCEL | 双向 | reason（`1`=用户取消，`2`=错误中断） |
| 0x09 | ERROR | 双向 | code(i32) + detail(UTF-8) |
| 0x0A | PING | 双向 | seq |
| 0x0B | PONG | 双向 | seq（对 PING 的应答） |
| 0x0C | BYE | 双向 | 正常关闭（无 payload 或带错误码） |

### 数据面（0x20–0x23）

| 码 | 名称 | 方向 | 说明 |
|----|------|------|------|
| 0x20 | DATA | S→R | file_seq、chunk_seq、payload（≤chunk_size，默认 **256KB**） |
| 0x21 | ACK | R→S | file_seq、acked_offset：**累积确认**（256KB 阈值即时回传 / 200ms 保底） |
| 0x22 | FILE_DONE | S→R | file_seq、hash（32B BLAKE3 全文件哈希） |
| 0x23 | FILE_DONE_ACK | R→S | file_seq、ok(bool)；hash 校验 + 落盘完成后发送 |

## 3. 连接建立与握手

1. 拨号方（QUIC 优先，失败 4s 后降级 TCP+TLS1.3）建立传输，双方交换 HELLO。
2. HELLO 校验：
   - 对端 UUID == 本机 UUID → 拒绝（连到自己，`-1`）。
   - 协议版本不兼容 → `ERROR(-13)`。
   - 指纹与信任库不匹配（TOFU Changed）→ `ERROR(-14)`，断开。
3. 配对（双方任一信任状态为 Unknown）：
   - 拨号方发 PAIR_REQ；被叫方在握手期直接读取该帧并回 PAIR_RESP。
   - 双方各自展示 4 位验证码（`verification_code(fp_a, fp_b)`，BLAKE3 指纹派生，两侧一致）。
   - 被叫方用户确认后 PAIR_RESP{accept:true}，双方写入信任库。
   - 拨号方在握手期直接读控制流等待 PAIR_RESP，期间应答 PING（防心跳误杀）。
   - 拒绝 → 拨号方 `-11`。
   - 双向同时配对（双向 Unknown + 迟到 PAIR_REQ）走迟到配对流程，同样展示验证码。
4. 握手收尾：读半部移交控制管道 0，启动调度器 + 心跳。

## 4. 心跳与保活

- 30s 周期发 PING（seq 递增）；收到任何帧刷新 `last_seen`。
- 90s 无任何入站帧 → 判定对端离线，`BYE` 后关闭，上报 `-3`。
- QUIC keep_alive_interval 10s，max_idle_timeout 90s（辅助层）。

## 5. 传输流程（发送 → 接收）

1. 发送端：`TRANSFER_REQ` → 等待 `TRANSFER_RESP`（60s 超时）。
2. 每文件独立管道（QUIC：新双向流；TCP：复用控制连接，pipe 0）。
3. `FILE_META` → 接收端校验后回 `FILE_META_ACK{accept:true}`。
4. 发送端逐分片发 DATA（256KB/片），边读边算 BLAKE3。
   - 在途窗口 4MB（`IN_FLIGHT_WINDOW_BYTES`），超限时等待 ACK 回传后继续。
   - 背压等待单次超时 200ms（`TIMEOUT_BACKPRESSURE_ACK`）。
5. 接收端按偏移写入，每累积 256KB（`ACK_THRESHOLD_BYTES`）或 200ms 保底，
   刷 `ACK{acked_offset}`（连续前缀）。
6. 文件发完 → `FILE_DONE{hash}`。接收端：
   - 不完整 → `FILE_DONE_ACK{ok:false}`。
   - 完整 → 落盘校验 BLAKE3，一致 → `FILE_DONE_ACK{ok:true}`；
     不一致 → `FILE_DONE_ACK{ok:false}`，临时文件删除，`-6`。
7. 全部文件结束 → 双方各发 Summary（ok/failed 计数），清理任务临时目录。

## 6. 取消

- CANCEL{reason=1}（用户取消）：双方停止传输，清理临时文件。
- CANCEL{reason=2}（错误中断）：记录错误，清理状态。
- 重复 DATA 分片幂等（按偏移覆写）。

## 7. 错误码

| 码 | 含义 | 码 | 含义 |
|----|------|----|------|
| 0 | 成功 | -9 | 已取消 |
| -1 | 参数无效 | -10 | 权限不足 |
| -2 | 端口不可用 | -11 | 配对失败 |
| -3 | 超时/对端离线 | -12 | 传输被拒绝 |
| -4 | 文件不可访问 | -13 | 协议不兼容 |
| -5 | 磁盘空间不足 | -14 | 指纹变更（TOFU） |
| -6 | 校验和不匹配 | -15 | 内部错误 |
| -7 | 发现服务不可用 | | |
| -8 | mmap 失败 | | |

## 8. 全局常量（`utils/src/constants.rs`）

| 常量 | 值 | 说明 |
|------|----|------|
| `IN_FLIGHT_WINDOW_BYTES` | 4 MB | 发送端在途（已发未确认）字节窗口 |
| `ACK_THRESHOLD_BYTES` | 256 KB | 接收端累积 ACK 触发阈值 |
| `DEFAULT_CHUNK_SIZE` | 256 KB | 文件分片大小 |
| `DEFAULT_CONCURRENCY` | 4 | 默认并行文件传输流数 |
| `QUIC_STREAM_FLOW_CONTROL_WINDOW` | 2 MB | 单流流控窗口（Quinn MAX_CHUNKS 安全限制：2MB/1200B ≈ 1740包，最坏碎片数 870 < 1024） |
| `QUIC_CONN_FLOW_CONTROL_WINDOW` | 64 MB | 连接级流控窗口 |
| `TIMEOUT_FILE_DONE_ACK` | 300s | 文件校验等待超时 |
| `TIMEOUT_TRANSFER_REQ` | 60s | 传输请求应答超时 |
| `TIMEOUT_CONNECT` | 10s | 网络连接握手超时 |
| `INTERVAL_PROGRESS_EMIT` | 250ms | UI 进度更新节流间隔 |

## 9. 发现（mDNS + UDP 探测）

- 服务类型 `_bolt._udp.local.`，实例 `bolt-{uuid}`，TXT 属性：
  `uuid/name/dt/qport/tport/ver/stealth`。
- UDP 广播探测 `255.255.255.255:8951`（独立端口，避开 QUIC 端口池）：魔数 `BTQ1`（查询）/ `BTP1`（应答+设备 JSON），
  3s 周期广播，10s 过期，UUID 去重；回复单播，不回自身。
- 端口池 8899 → 8950 自动避让；Android 侧用 NSD API 桥接（`bt_nsd_inject_device`）。
- 存在活跃连接的设备在列表中被钉住，TTL 清扫不移除；连接 HELLO 以真实 uuid 校正手动连接占位条目。
- 隐身模式：停广播与应答，仅被动发现他人。

## 10. 安全模型

- TLS 1.3 强制（rustls+ring），Ed25519 自签证书（10 年），无明文模式。
- 设备身份 = 证书 BLAKE3 指纹（冒号分隔）；HELLO 交换指纹。
- 信任链：信道加密（TLS）+ 4 位验证码人工比对 + TOFU 信任库（指纹变更 → `-14` 硬拒绝）。
- 接收方必须确认传输请求（自动接收仅限已信任设备且显式开启）。
- 日志不含文件内容与密钥材料；无任何外部网络请求。
