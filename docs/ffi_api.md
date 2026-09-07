# LocalTransfer FFI API（`lt_` 前缀）

> 头文件由 cbindgen 生成：`crates/lt-ffi/include/lt_api.h`。
> 库产物：Windows `lt_ffi.dll`；Android `liblt_ffi.so`。
> 线程模型：任何线程可调用；事件由专用分发线程回调（不在调用线程上执行）。

## 约定

- 整型返回 `0` 成功 / 负错误码（见 protocol_spec.md §7）。
- 返回 `char*` 的接口由调用方 `lt_free_string` 释放；`lt_version` 为静态串不释放。
- 回调签名：`void cb(int32_t event_id, const char *payload_json)`，payload 为 UTF-8 JSON，
  回调返回后即失效（如需保留请拷贝）。
- 事件回调可随时重设；传 NULL 清除。

## 生命周期

| 函数 | 说明 |
|------|------|
| `int lt_init(const char *data_dir)` | 初始化引擎+发现；data_dir=NULL 用系统默认目录；幂等 |
| `void lt_shutdown(void)` | 停发现、停引擎、释放内部状态；幂等 |
| `const char *lt_version(void)` | 版本串（静态） |

## 事件（`lt_set_event_callback`）

| 常量 | 值 | payload JSON 关键字段 |
|------|----|----------------------|
| EVT_DEVICE_LIST | 1 | `devices:[{uuid,name,ip,quic_port,tcp_port,device_type,source,…}]` |
| EVT_CONN_STATE | 2 | `uuid,name,state:connected\|disconnected,conn_id,transport,err` |
| EVT_PAIR_REQUEST | 3 | `pair_id,uuid,name,code`（6 位验证码，展示并让用户比对） |
| EVT_TRANSFER_REQUEST | 4 | `req_id,uuid,name,file_count,total_size` |
| EVT_TASK_STATE | 5 | `task_id,incoming,state:waiting_accept\|transferring\|done\|cancelled\|error\|rejected` |
| EVT_TASK_PROGRESS | 6 | `task_id,incoming,rel_path,done,total,rate_bps,eta_secs` |
| EVT_TASK_SUMMARY | 7 | `task_id,incoming,ok,failed` |
| EVT_ERROR | 8 | `task_id,code,message` |

> **注意**：`EVT_TASK_STATE` 的 `state` 字段不再包含 `paused`（暂停/恢复功能已移除）。

## 配置

| 函数 | 说明 |
|------|------|
| `int lt_set_config(const char *json)` | 合并 JSON 片段（如 `{"device_name":"我的电脑"}`）；未知 key 报 -1 |
| `char *lt_get_config(void)` | 全量配置 JSON |

配置 key：`device_name`、`save_dir`、`stealth_mode`、`auto_accept_trusted`、
`concurrency`、`listen_port`、`chunk_size`、`data_dir`。

## 发现

| 函数 | 说明 |
|------|------|
| `int lt_start_discovery(void)` | 启动 mDNS 广播+浏览、UDP 探测、过期清理 |
| `int lt_stop_discovery(void)` | 停止全部发现活动 |
| `int lt_probe_network(void)` | 立即发起一轮 UDP 广播探测 |
| `char *lt_get_devices(void)` | 当前设备列表 JSON 数组 |
| `int lt_add_manual_device(const char *ip, uint16_t port)` | 直连场景手动登记 |
| `int lt_nsd_inject_device(const char *json)` | Android NSD 桥：注入发现结果（`{uuid,ip,name,dt,qport,tport}`） |
| `int lt_nsd_remove_device(const char *uuid)` | Android NSD 桥：服务丢失 |

## 连接 / 配对 / 传输

| 函数 | 说明 |
|------|------|
| `int lt_connect(const char *uuid)` | 连接设备（异步，结果经事件） |
| `int lt_connect_addr(const char *ip, uint16_t port)` | 直连任意地址 |
| `int lt_disconnect(const char *uuid)` | 断开会话 |
| `int lt_respond_pair(uint64_t pair_id, int accept)` | 配对应答（pair_id 取自 EVT_PAIR_REQUEST） |
| `int lt_respond_transfer(uint64_t req_id, int accept)` | 传输请求应答（req_id 取自 EVT_TRANSFER_REQUEST） |
| `int lt_send_files(const char *uuid, const char *paths_json, uint64_t *out_task_id)` | 发送文件/目录；`paths_json=["a.txt","dir"]`；未连接自动先连 |
| `int lt_cancel_task(uint64_t task_id)` | 取消（通知对端 CANCEL，清理任务状态） |
| `char *lt_get_tasks(void)` | 任务列表 JSON |
| `int lt_clear_records(void)` | 清除已结束任务记录 |
| `int lt_clear_temp_cache(void)` | 清理临时文件缓存 |
| `char *lt_get_local_fingerprint(void)` | 本机指纹（冒号分隔 BLAKE3，屏幕比对用） |
| `void lt_free_string(char *ptr)` | 释放字符串 |

> **v2 变更**：已移除 `lt_pause_task` 和 `lt_resume_task`。当前版本的传输一旦开始只能取消或完成，不支持暂停/恢复。

## 集成要点

- **Windows/Tauri**：静态或动态链接 `lt_ffi`，事件回调通过 `crossbeam_channel`
  桥接到 Tauri 主线程，再经 `Emitter` 转发给 Vue 前端。
- **Android**：`System.loadLibrary("lt_ffi")`；发现走 NSD 桥（Kotlin 调
  `lt_nsd_inject_device`），Rust 侧不初始化 mDNS；发送用 SAF 路径或共享
  存储绝对路径；长时间任务置于前台服务。
- **回调内禁止**：阻塞、再次调用可能持锁的 lt_* 接口（事件线程与内部
  状态耦合，重入会死锁）。
