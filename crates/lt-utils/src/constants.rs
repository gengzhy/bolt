//! LocalTransfer 全局统一常量定义。
//!
//! 集中管理所有协议魔数、窗口大小、超时阈值、取消原因码与状态常量，
//! 消除散落在各模块的硬编码，确保修改一处全局生效。

use std::time::Duration;

// ---------------- 任务取消原因码（Wire Protocol Cancel Frame） ----------------
/// 用户主动取消任务：彻底废弃临时文件与断点记录
pub const CANCEL_REASON_USER: u32 = 1;
/// 任务发生错误中断
pub const CANCEL_REASON_ERROR: u32 = 2;

// ---------------- 传输窗口与流控尺寸 ----------------
/// 在途（已发未确认）字节窗口：4MB，平滑流水线
pub const IN_FLIGHT_WINDOW_BYTES: u64 = 4 * 1024 * 1024;
/// 接收端累积 ACK 触发阈值：256KB，高频回传，消除发送端流水线停等
pub const ACK_THRESHOLD_BYTES: u64 = 256 * 1024;
/// 默认文件分片大小：256KB
pub const DEFAULT_CHUNK_SIZE: usize = 256 * 1024;
/// 默认并行文件传输流数
pub const DEFAULT_CONCURRENCY: usize = 4;
/// QUIC 单流流控窗口：1MB（严格约束在 Quinn 1024 乱序包上限内，杜绝 gaps 超限）
pub const QUIC_STREAM_FLOW_CONTROL_WINDOW: u64 = 1024 * 1024;
/// QUIC 连接级流控窗口：64MB
pub const QUIC_CONN_FLOW_CONTROL_WINDOW: u64 = 64 * 1024 * 1024;

// ---------------- 超时与间隔时间定义 ----------------
/// 文件接收校验与落盘完成等待超时（大文件全量 Blake3 校验时间）
pub const TIMEOUT_FILE_DONE_ACK: Duration = Duration::from_secs(300);
/// 文件元数据确认等待超时
pub const TIMEOUT_FILE_META_ACK: Duration = Duration::from_secs(30);
/// 传输请求等待对端用户应答超时
pub const TIMEOUT_TRANSFER_REQ: Duration = Duration::from_secs(60);
/// 配对请求等待对端用户确认超时
pub const TIMEOUT_PAIR_REQ: Duration = Duration::from_secs(60);
/// 网络连接握手超时
pub const TIMEOUT_CONNECT: Duration = Duration::from_secs(10);
/// 背压达到上限时单次等待 ACK 到达超时
pub const TIMEOUT_BACKPRESSURE_ACK: Duration = Duration::from_millis(200);
/// UI 进度更新事件的平滑节流间隔
pub const INTERVAL_PROGRESS_EMIT: Duration = Duration::from_millis(250);

// ---------------- 任务状态字符串常量 ----------------
pub const TASK_STATE_WAITING_ACCEPT: &str = "waiting_accept";
pub const TASK_STATE_TRANSFERRING: &str = "transferring";
pub const TASK_STATE_PAUSED: &str = "paused";
pub const TASK_STATE_CANCELLED: &str = "cancelled";
pub const TASK_STATE_DONE: &str = "done";
pub const TASK_STATE_ERROR: &str = "error";
pub const TASK_STATE_REJECTED: &str = "rejected";

// ---------------- 同名文件冲突处理策略 ----------------
pub const COLLISION_RENAME: &str = "rename";
pub const COLLISION_OVERWRITE: &str = "overwrite";
