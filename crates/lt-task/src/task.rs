//! 任务记录与状态机（实施方案 9.2）。

use serde::{Deserialize, Serialize};

/// 任务方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Send,
    Recv,
}

/// 任务状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// 等待接收方用户确认
    WaitingAccept,
    /// 传输中
    Transferring,
    /// 全部完成
    Done,
    /// 用户取消
    Cancelled,
    /// 出错终止
    Error,
}

use lt_utils::constants::*;

impl TaskState {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskState::WaitingAccept => TASK_STATE_WAITING_ACCEPT,
            TaskState::Transferring => TASK_STATE_TRANSFERRING,
            TaskState::Done => TASK_STATE_DONE,
            TaskState::Cancelled => TASK_STATE_CANCELLED,
            TaskState::Error => TASK_STATE_ERROR,
        }
    }

    /// 从引擎字符串状态解析。
    pub fn from_engine(s: &str) -> TaskState {
        match s {
            TASK_STATE_WAITING_ACCEPT => TaskState::WaitingAccept,
            TASK_STATE_TRANSFERRING => TaskState::Transferring,
            TASK_STATE_DONE => TaskState::Done,
            TASK_STATE_CANCELLED | TASK_STATE_PAUSED => TaskState::Cancelled,
            _ => TaskState::Error,
        }
    }
}

/// 任务记录（发送/接收入同一队列，实施方案 9.2）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub task_id: u64,
    /// 全局唯一任务 ID（UUIDv4 32位十六进制字符串），贯穿跨网络、跨断点全生命周期
    #[serde(default)]
    pub task_uid: String,
    pub direction: Direction,
    pub peer_uuid: String,
    pub peer_name: String,
    pub file_count: u32,
    pub total_size: u64,
    pub done_bytes: u64,
    pub ok_files: u32,
    pub failed_files: u32,
    pub state: TaskState,
    /// 当前文件相对路径
    #[serde(default)]
    pub current_file: String,
    pub rate_bps: u64,
    pub eta_secs: u64,
    /// 发送任务源路径（续传用）
    #[serde(default)]
    pub source_paths: Vec<String>,
    /// 任务实际使用的传输通道（quic/tcp；会话建立后填入）
    #[serde(default)]
    pub transport: String,
    /// 创建时间（unix 秒）
    pub created_unix: u64,
    /// 生命周期世代版本号（续传自增，防止过期协程退出时交叉背刺篡改状态）
    #[serde(default)]
    pub generation: u64,
}

impl TaskRecord {
    pub fn new_send(
        task_id: u64,
        task_uid: String,
        peer_uuid: String,
        peer_name: String,
        file_count: u32,
        total_size: u64,
        source_paths: Vec<String>,
    ) -> TaskRecord {
        TaskRecord {
            task_id,
            task_uid,
            direction: Direction::Send,
            peer_uuid,
            peer_name,
            file_count,
            total_size,
            done_bytes: 0,
            ok_files: 0,
            failed_files: 0,
            state: TaskState::WaitingAccept,
            current_file: String::new(),
            rate_bps: 0,
            eta_secs: 0,
            source_paths,
            transport: String::new(),
            created_unix: now_unix(),
            generation: 1,
        }
    }

    pub fn new_recv(
        task_id: u64,
        task_uid: String,
        peer_uuid: String,
        peer_name: String,
        file_count: u32,
        total_size: u64,
    ) -> TaskRecord {
        TaskRecord {
            task_id,
            task_uid,
            direction: Direction::Recv,
            peer_uuid,
            peer_name,
            file_count,
            total_size,
            done_bytes: 0,
            ok_files: 0,
            failed_files: 0,
            state: TaskState::WaitingAccept,
            current_file: String::new(),
            rate_bps: 0,
            eta_secs: 0,
            source_paths: vec![],
            transport: String::new(),
            created_unix: now_unix(),
            generation: 1,
        }
    }
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_roundtrip() {
        assert_eq!(
            TaskState::from_engine("transferring"),
            TaskState::Transferring
        );
        assert_eq!(TaskState::from_engine("done"), TaskState::Done);
        assert_eq!(TaskState::from_engine("paused"), TaskState::Cancelled);
        assert_eq!(TaskState::Cancelled.as_str(), "cancelled");
    }

    #[test]
    fn record_json() {
        let r = TaskRecord::new_send(
            1,
            "0123456789abcdef0123456789abcdef".into(),
            "u".into(),
            "pc".into(),
            2,
            100,
            vec!["a".into()],
        );
        let j = serde_json::to_string(&r).unwrap();
        let back: TaskRecord = serde_json::from_str(&j).unwrap();
        assert_eq!(back.task_id, 1);
        assert_eq!(back.task_uid, "0123456789abcdef0123456789abcdef");
        assert!(matches!(back.direction, Direction::Send));
    }
}
