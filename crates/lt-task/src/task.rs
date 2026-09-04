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
    /// 暂停（可续传）
    Paused,
    /// 全部完成
    Done,
    /// 用户取消
    Cancelled,
    /// 出错终止
    Error,
}

impl TaskState {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskState::WaitingAccept => "waiting_accept",
            TaskState::Transferring => "transferring",
            TaskState::Paused => "paused",
            TaskState::Done => "done",
            TaskState::Cancelled => "cancelled",
            TaskState::Error => "error",
        }
    }

    /// 从引擎字符串状态解析。
    pub fn from_engine(s: &str) -> TaskState {
        match s {
            "waiting_accept" => TaskState::WaitingAccept,
            "transferring" => TaskState::Transferring,
            "paused" => TaskState::Paused,
            "done" => TaskState::Done,
            "cancelled" => TaskState::Cancelled,
            _ => TaskState::Error,
        }
    }
}

/// 任务记录（发送/接收入同一队列，实施方案 9.2）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub task_id: u64,
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
}

impl TaskRecord {
    pub fn new_send(
        task_id: u64,
        peer_uuid: String,
        peer_name: String,
        file_count: u32,
        total_size: u64,
        source_paths: Vec<String>,
    ) -> TaskRecord {
        TaskRecord {
            task_id,
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
        }
    }

    pub fn new_recv(
        task_id: u64,
        peer_uuid: String,
        peer_name: String,
        file_count: u32,
        total_size: u64,
    ) -> TaskRecord {
        TaskRecord {
            task_id,
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
        assert_eq!(TaskState::Paused.as_str(), "paused");
    }

    #[test]
    fn record_json() {
        let r = TaskRecord::new_send(1, "u".into(), "pc".into(), 2, 100, vec!["a".into()]);
        let j = serde_json::to_string(&r).unwrap();
        let back: TaskRecord = serde_json::from_str(&j).unwrap();
        assert_eq!(back.task_id, 1);
        assert!(matches!(back.direction, Direction::Send));
    }
}
